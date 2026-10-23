//! Builds a bootable AWA disk image:
//!
//! ```text
//! LBA 0        MBR (one bootable FAT partition; Limine's BIOS stage 1 is
//!              installed into it afterwards by `limine bios-install`)
//! LBA 1..2047  gap used by Limine's BIOS stage 2
//! LBA 2048..   FAT partition:
//!                /EFI/BOOT/BOOTX64.EFI        Limine for UEFI
//!                /boot/limine/limine-bios.sys Limine for BIOS
//!                /boot/limine/limine.conf
//!                /boot/awa-kernel
//! ```
//!
//! An MBR disk boots both ways: legacy BIOS (the dev laptop's current mode)
//! and UEFI, whose firmware loads `\EFI\BOOT\BOOTX64.EFI` from removable media.
//!
//! Usage: awa-mkimage <out.img> <size-MiB> <dest-path>=<source-file>...

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::ExitCode;

const SECTOR: u64 = 512;
const PARTITION_START_LBA: u64 = 2048;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("awa-mkimage: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        return Err("usage: awa-mkimage <out.img> <size-MiB> <dest>=<source>...".into());
    }
    let out = Path::new(&args[0]);
    let size_mib: u64 = args[1].parse().map_err(|_| "size must be a number of MiB")?;
    if size_mib < 33 {
        return Err("image must be at least 33 MiB".into());
    }
    let mut files = Vec::new();
    for spec in &args[2..] {
        let (dest, src) = spec.split_once('=').ok_or_else(|| format!("bad file spec: {spec}"))?;
        files.push((dest.trim_start_matches('/').to_string(), src.to_string()));
    }

    let total_sectors = size_mib * 1024 * 1024 / SECTOR;
    let part_sectors = total_sectors - PARTITION_START_LBA;

    let mut disk = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(out)
        .map_err(|e| format!("create {}: {e}", out.display()))?;
    disk.set_len(total_sectors * SECTOR).map_err(|e| format!("size image: {e}"))?;

    let mut part = Slice::new(&mut disk, PARTITION_START_LBA * SECTOR, part_sectors * SECTOR);
    fatfs::format_volume(&mut part, fatfs::FormatVolumeOptions::new().volume_label(*b"AWA BOOT   "))
        .map_err(|e| format!("format FAT: {e}"))?;

    let fat_type = {
        let fs = fatfs::FileSystem::new(&mut part, fatfs::FsOptions::new())
            .map_err(|e| format!("open FAT: {e}"))?;
        for (dest, src) in &files {
            copy_into(&fs, dest, Path::new(src)).map_err(|e| format!("copy {src} -> /{dest}: {e}"))?;
        }
        let fat_type = fs.fat_type();
        fs.unmount().map_err(|e| format!("unmount FAT: {e}"))?;
        fat_type
    };

    // FAT16/FAT32 with LBA addressing.
    let type_byte = match fat_type {
        fatfs::FatType::Fat32 => 0x0C,
        _ => 0x0E,
    };
    write_mbr(&mut disk, type_byte, part_sectors).map_err(|e| format!("write MBR: {e}"))?;
    disk.sync_all().map_err(|e| format!("sync: {e}"))?;
    println!(
        "awa-mkimage: wrote {} ({size_mib} MiB, {:?}, {} files)",
        out.display(),
        fat_type,
        files.len()
    );
    Ok(())
}

fn copy_into<T: fatfs::ReadWriteSeek>(fs: &fatfs::FileSystem<T>, dest: &str, src: &Path) -> io::Result<()> {
    let data = std::fs::read(src)?;
    let root = fs.root_dir();
    let mut dir = root.clone();
    let mut parts: Vec<&str> = dest.split('/').filter(|p| !p.is_empty()).collect();
    let name = parts.pop().ok_or_else(|| io::Error::other("empty destination"))?;
    for part in parts {
        dir = match dir.open_dir(part) {
            Ok(d) => d,
            Err(_) => dir.create_dir(part)?,
        };
    }
    let mut file = dir.create_file(name)?;
    file.truncate()?;
    file.write_all(&data)?;
    file.flush()
}

/// Writes a classic MBR with one active partition. Bytes 0..440 (boot code)
/// are left zero for `limine bios-install` to fill in.
fn write_mbr(disk: &mut File, type_byte: u8, part_sectors: u64) -> io::Result<()> {
    let start = u32::try_from(PARTITION_START_LBA).map_err(io::Error::other)?;
    let count = u32::try_from(part_sectors).map_err(|_| io::Error::other("image too large for MBR"))?;
    let mut mbr = [0u8; 512];
    mbr[440..444].copy_from_slice(&0x4157_4131u32.to_le_bytes()); // disk signature "1AWA"
    let entry = &mut mbr[446..462];
    entry[0] = 0x80; // active (bootable)
    entry[1..4].copy_from_slice(&[0xFE, 0xFF, 0xFF]); // CHS start: use LBA
    entry[4] = type_byte;
    entry[5..8].copy_from_slice(&[0xFE, 0xFF, 0xFF]); // CHS end: use LBA
    entry[8..12].copy_from_slice(&start.to_le_bytes());
    entry[12..16].copy_from_slice(&count.to_le_bytes());
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    disk.seek(SeekFrom::Start(0))?;
    disk.write_all(&mbr)
}

/// A window `[offset, offset + len)` of a file, so the FAT library sees only
/// the partition.
struct Slice<'a> {
    file: &'a mut File,
    offset: u64,
    len: u64,
    pos: u64,
}

impl<'a> Slice<'a> {
    fn new(file: &'a mut File, offset: u64, len: u64) -> Self {
        Self { file, offset, len, pos: 0 }
    }

    fn remaining(&self) -> usize {
        usize::try_from(self.len.saturating_sub(self.pos)).unwrap_or(usize::MAX)
    }
}

impl Read for Slice<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = buf.len().min(self.remaining());
        self.file.seek(SeekFrom::Start(self.offset + self.pos))?;
        let n = self.file.read(&mut buf[..n])?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Write for Slice<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = buf.len().min(self.remaining());
        if n == 0 && !buf.is_empty() {
            return Err(io::Error::new(io::ErrorKind::WriteZero, "write past end of partition"));
        }
        self.file.seek(SeekFrom::Start(self.offset + self.pos))?;
        let n = self.file.write(&buf[..n])?;
        self.pos += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl Seek for Slice<'_> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new = match pos {
            SeekFrom::Start(p) => Some(p),
            SeekFrom::End(d) => self.len.checked_add_signed(d),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
        };
        match new {
            Some(p) if p <= self.len => {
                self.pos = p;
                Ok(p)
            }
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "seek outside partition")),
        }
    }
}

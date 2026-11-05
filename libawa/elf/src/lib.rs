//! Minimal, defensive ELF64 reader for AWA executables.
//!
//! AWA programs are static position-independent executables (x86_64,
//! little-endian). Loading one means: map each PT_LOAD segment at
//! `base + vaddr` with its own permissions, then add `base` to every
//! R_X86_64_RELATIVE slot. Everything else is rejected.
//!
//! Every offset and size is checked against the file, so a malformed file
//! produces an error, never an out-of-bounds read.

#![no_std]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    TooSmall,
    NotElf64LittleEndian,
    NotX86_64,
    NotPositionIndependent,
    BadProgramHeaders,
    SegmentOutOfFile,
    WritableAndExecutable,
    BadRelocations,
    UnsupportedRelocation(u32),
}

#[derive(Debug, Clone, Copy)]
pub struct Segment {
    /// Offset from the load base where the segment starts in memory.
    pub vaddr: u64,
    /// Bytes in memory (the part past `file_size` is zero-filled).
    pub mem_size: u64,
    pub file_offset: u64,
    pub file_size: u64,
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

pub struct Elf<'a> {
    data: &'a [u8],
    entry: u64,
    phoff: u64,
    phnum: u16,
}

const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const DT_NULL: u64 = 0;
const DT_RELA: u64 = 7;
const DT_RELASZ: u64 = 8;
const DT_RELAENT: u64 = 9;
const R_X86_64_RELATIVE: u32 = 8;

fn u16_at(d: &[u8], o: u64) -> Option<u16> {
    let o = usize::try_from(o).ok()?;
    Some(u16::from_le_bytes(d.get(o..o.checked_add(2)?)?.try_into().ok()?))
}
fn u32_at(d: &[u8], o: u64) -> Option<u32> {
    let o = usize::try_from(o).ok()?;
    Some(u32::from_le_bytes(d.get(o..o.checked_add(4)?)?.try_into().ok()?))
}
fn u64_at(d: &[u8], o: u64) -> Option<u64> {
    let o = usize::try_from(o).ok()?;
    Some(u64::from_le_bytes(d.get(o..o.checked_add(8)?)?.try_into().ok()?))
}

impl<'a> Elf<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, Error> {
        if data.len() < 64 {
            return Err(Error::TooSmall);
        }
        if &data[0..4] != b"\x7fELF" || data[4] != 2 || data[5] != 1 {
            return Err(Error::NotElf64LittleEndian);
        }
        if u16_at(data, 18) != Some(62) {
            return Err(Error::NotX86_64);
        }
        if u16_at(data, 16) != Some(3) {
            return Err(Error::NotPositionIndependent); // ET_DYN: required for ASLR
        }
        let entry = u64_at(data, 24).ok_or(Error::TooSmall)?;
        let phoff = u64_at(data, 32).ok_or(Error::TooSmall)?;
        let phentsize = u16_at(data, 54).ok_or(Error::TooSmall)?;
        let phnum = u16_at(data, 56).ok_or(Error::TooSmall)?;
        let end = phoff.checked_add(u64::from(phnum) * 56).ok_or(Error::BadProgramHeaders)?;
        if phentsize != 56 || end > data.len() as u64 {
            return Err(Error::BadProgramHeaders);
        }
        Ok(Self { data, entry, phoff, phnum })
    }

    /// Entry point, as an offset from the load base.
    pub fn entry(&self) -> u64 {
        self.entry
    }

    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    fn header(&self, i: u16) -> (u32, u32, u64, u64, u64, u64) {
        let h = self.phoff + u64::from(i) * 56;
        let d = self.data;
        // In bounds: checked in `parse`.
        (
            u32_at(d, h).unwrap_or(0),
            u32_at(d, h + 4).unwrap_or(0),
            u64_at(d, h + 8).unwrap_or(0),
            u64_at(d, h + 16).unwrap_or(0),
            u64_at(d, h + 32).unwrap_or(0),
            u64_at(d, h + 40).unwrap_or(0),
        )
    }

    /// Calls `f` for every loadable segment, after validating it.
    pub fn segments(&self, mut f: impl FnMut(Segment) -> Result<(), Error>) -> Result<(), Error> {
        for i in 0..self.phnum {
            let (kind, flags, offset, vaddr, file_size, mem_size) = self.header(i);
            if kind != PT_LOAD {
                continue;
            }
            let end = offset.checked_add(file_size).ok_or(Error::SegmentOutOfFile)?;
            if end > self.data.len() as u64 || file_size > mem_size || vaddr.checked_add(mem_size).is_none() {
                return Err(Error::SegmentOutOfFile);
            }
            let s = Segment { vaddr, mem_size, file_offset: offset, file_size, read: flags & 4 != 0, write: flags & 2 != 0, execute: flags & 1 != 0 };
            if s.write && s.execute {
                return Err(Error::WritableAndExecutable); // W^X
            }
            f(s)?;
        }
        Ok(())
    }

    /// Highest address (offset from base) any segment reaches.
    pub fn image_size(&self) -> Result<u64, Error> {
        let mut top = 0;
        self.segments(|s| {
            top = top.max(s.vaddr + s.mem_size);
            Ok(())
        })?;
        Ok(top)
    }

    /// Calls `f(offset)` for every R_X86_64_RELATIVE slot: the loader must
    /// add the load base to the 8-byte value at `base + offset`, whose
    /// original value (the addend) is passed as the second argument.
    pub fn relocations(&self, mut f: impl FnMut(u64, u64) -> Result<(), Error>) -> Result<(), Error> {
        for i in 0..self.phnum {
            let (kind, _, offset, _, file_size, _) = self.header(i);
            if kind != PT_DYNAMIC {
                continue;
            }
            let (mut rela, mut relasz, mut relaent) = (0, 0, 24);
            let mut o = offset;
            while o + 16 <= offset + file_size {
                let tag = u64_at(self.data, o).ok_or(Error::BadRelocations)?;
                let val = u64_at(self.data, o + 8).ok_or(Error::BadRelocations)?;
                match tag {
                    DT_NULL => break,
                    DT_RELA => rela = val,
                    DT_RELASZ => relasz = val,
                    DT_RELAENT => relaent = val,
                    _ => {}
                }
                o += 16;
            }
            if relasz == 0 {
                return Ok(());
            }
            if relaent != 24 {
                return Err(Error::BadRelocations);
            }
            // In a static PIE the file offset of .rela.dyn equals its vaddr for
            // the first (read-only) segment; translate through the segments to be safe.
            let file_off = self.vaddr_to_offset(rela).ok_or(Error::BadRelocations)?;
            let mut r = 0;
            while r < relasz {
                let at = file_off + r;
                let r_offset = u64_at(self.data, at).ok_or(Error::BadRelocations)?;
                let info = u64_at(self.data, at + 8).ok_or(Error::BadRelocations)?;
                let addend = u64_at(self.data, at + 16).ok_or(Error::BadRelocations)?;
                let kind = info as u32;
                if kind != R_X86_64_RELATIVE {
                    return Err(Error::UnsupportedRelocation(kind));
                }
                f(r_offset, addend)?;
                r += 24;
            }
        }
        Ok(())
    }

    fn vaddr_to_offset(&self, va: u64) -> Option<u64> {
        for i in 0..self.phnum {
            let (kind, _, offset, vaddr, file_size, _) = self.header(i);
            if kind == PT_LOAD && va >= vaddr && va < vaddr + file_size {
                return Some(offset + (va - vaddr));
            }
        }
        None
    }
}

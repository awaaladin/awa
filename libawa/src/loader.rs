//! Starting a program from an ELF file held in a VMO: the user-space launcher.
//!
//! Reads the file, creates a process, copies each segment into one image VMO
//! at a random base, applies relocations, maps segments with their own
//! permissions (never writable and executable), maps a stack with an unmapped
//! guard page below it, and starts the main thread.

use crate::abi::{map, Status};
use crate::elf::Elf;
use crate::{process, random, vmo, Result};

const PAGE: u64 = 4096;
const STACK_SIZE: u64 = 128 * 1024;
const MAX_FILE: usize = 256 * 1024;

/// Scratch space for reading a program file (no heap in early programs).
static mut FILE: [u8; MAX_FILE] = [0; MAX_FILE];

fn random_page(lo: u64, hi: u64) -> u64 {
    lo + (random() % ((hi - lo) / PAGE)) * PAGE
}

/// Starts the program in `file_vmo`. `bootstrap` (0 for none) is moved into
/// the new process. Returns the process handle.
pub fn spawn(file_vmo: u32, bootstrap: u32) -> Result<u32> {
    let size = vmo::size(file_vmo)? as usize;
    if size > MAX_FILE {
        return Err(Status::OutOfRange);
    }
    // SAFETY: programs using the loader are single-threaded (AWA processes
    // have one thread for now), so nothing else touches FILE concurrently.
    let file: &mut [u8] = unsafe { &mut (&mut *core::ptr::addr_of_mut!(FILE))[..size] };
    vmo::read(file_vmo, 0, file)?;
    let elf = Elf::parse(file).map_err(|_| Status::InvalidArgs)?;
    let image_size = elf.image_size().map_err(|_| Status::InvalidArgs)?.next_multiple_of(PAGE);

    let image = vmo::create(image_size)?;
    let mut err = Ok(());
    elf.segments(|s| {
        let bytes = &elf.data()[s.file_offset as usize..(s.file_offset + s.file_size) as usize];
        if let Err(e) = vmo::write(image, s.vaddr, bytes) {
            err = Err(e);
        }
        Ok(())
    })
    .map_err(|_| Status::InvalidArgs)?;
    err?;

    let base = random_page(0x1_0000_0000, 0x4000_0000_0000);
    elf.relocations(|offset, addend| {
        if let Err(e) = vmo::write(image, offset, &(base + addend).to_le_bytes()) {
            err = Err(e);
        }
        Ok(())
    })
    .map_err(|_| Status::InvalidArgs)?;
    err?;

    let p = process::create()?;
    // Segments arrive in address order. If one starts inside the last page of
    // the previous one, that page is already mapped: it is shared only when
    // both want the same permissions, otherwise the file is refused.
    let mut mapped_to = 0u64;
    let mut last_flags = 0u64;
    elf.segments(|s| {
        let mut start = s.vaddr / PAGE * PAGE;
        let end = (s.vaddr + s.mem_size).next_multiple_of(PAGE);
        let flags = map::READ | if s.write { map::WRITE } else { 0 } | if s.execute { map::EXECUTE } else { 0 };
        if start < mapped_to {
            if flags != last_flags {
                err = Err(Status::InvalidArgs);
                return Ok(());
            }
            start = mapped_to;
        }
        if start < end {
            if let Err(e) = process::map(p, image, start, base + start, end - start, flags) {
                err = Err(e);
            }
        }
        mapped_to = mapped_to.max(end);
        last_flags = flags;
        Ok(())
    })
    .map_err(|_| Status::InvalidArgs)?;
    err?;

    let stack = vmo::create(STACK_SIZE)?;
    let stack_base = random_page(0x5000_0000_0000, 0x6000_0000_0000);
    process::map(p, stack, 0, stack_base, STACK_SIZE, map::READ | map::WRITE)?;

    process::start(p, base + elf.entry(), stack_base + STACK_SIZE, bootstrap)?;
    crate::close(image)?;
    crate::close(stack)?;
    Ok(p)
}

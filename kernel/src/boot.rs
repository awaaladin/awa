//! Limine boot protocol: the requests this kernel makes and safe access to
//! the bootloader's responses.
//!
//! The bootloader scans the kernel image for these structures before jumping
//! to `kmain` and fills in the response pointers. Reference: the Limine
//! protocol specification (PROTOCOL.md in the Limine repository).

use crate::console::Framebuffer;
use core::cell::UnsafeCell;
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};

/// Limine base revision this kernel is written against.
pub const BASE_REVISION: u64 = 3;

const COMMON_MAGIC: [u64; 2] = [0xc7b1_dd30_df4c_8b88, 0x0a82_e883_a194_f07b];

/// A request whose response pointer the bootloader fills in.
#[repr(C)]
struct Request<T> {
    id: [u64; 4],
    revision: u64,
    response: UnsafeCell<*const T>,
}

// SAFETY: the bootloader writes `response` before any kernel code runs. The
// kernel only reads it afterwards, so there are no concurrent writes.
unsafe impl<T> Sync for Request<T> {}

impl<T> Request<T> {
    const fn new(id: [u64; 2], revision: u64) -> Self {
        Self {
            id: [COMMON_MAGIC[0], COMMON_MAGIC[1], id[0], id[1]],
            revision,
            response: UnsafeCell::new(ptr::null()),
        }
    }

    fn response(&self) -> Option<&'static T> {
        // SAFETY: `self.response` is a valid, aligned pointer to a static.
        // Volatile, because the bootloader wrote it outside Rust's view.
        let raw = unsafe { ptr::read_volatile(self.response.get()) };
        // SAFETY: when non-null, Limine guarantees the response is valid,
        // correctly typed for this request id, mapped, and never freed
        // (it lives in bootloader-reclaimable memory, which the kernel does
        // not reuse in Phase 1).
        unsafe { raw.as_ref() }
    }
}

#[repr(transparent)]
struct BaseRevisionTag(UnsafeCell<[u64; 3]>);

// SAFETY: same reasoning as for `Request`: written once by the bootloader
// before the kernel starts, then only read.
unsafe impl Sync for BaseRevisionTag {}

#[used]
#[unsafe(link_section = ".requests_start_marker")]
static REQUESTS_START: [u64; 4] = [
    0xf6b8_f4b3_9de7_d1ae,
    0xfab9_1a69_40fc_b9cf,
    0x785c_6ed0_15d3_e316,
    0x181e_920a_7852_b9d9,
];

#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION_TAG: BaseRevisionTag = BaseRevisionTag(UnsafeCell::new([
    0xf956_2b2d_5c95_a6c8,
    0x6a7b_3849_4453_6bdc,
    BASE_REVISION,
]));

#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: Request<FramebufferResponse> =
    Request::new([0x9d58_27dc_d881_dd75, 0xa314_8604_f6fa_b11b], 0);

#[used]
#[unsafe(link_section = ".requests")]
static MEMMAP_REQUEST: Request<MemmapResponse> = Request::new([0x67cf_3d9d_378a_806f, 0xe304_acdf_c50c_3c62], 0);

#[used]
#[unsafe(link_section = ".requests")]
static HHDM_REQUEST: Request<HhdmResponse> = Request::new([0x48dc_f1cb_8ad2_b852, 0x6398_4e95_9a98_244b], 0);

#[used]
#[unsafe(link_section = ".requests")]
static EXECUTABLE_ADDRESS_REQUEST: Request<ExecutableAddressResponse> =
    Request::new([0x71ba_7686_3cc5_5f63, 0xb264_4a48_c516_a487], 0);

/// The multiprocessor request has an extra `flags` field after the response.
#[repr(C)]
struct MpRequest {
    id: [u64; 4],
    revision: u64,
    response: UnsafeCell<*const MpResponse>,
    flags: u64,
}

// SAFETY: as for `Request`: written once by the bootloader before the kernel runs.
unsafe impl Sync for MpRequest {}

#[used]
#[unsafe(link_section = ".requests")]
static MP_REQUEST: MpRequest = MpRequest {
    id: [COMMON_MAGIC[0], COMMON_MAGIC[1], 0x95a6_7b81_9a1b_857e, 0xa0b6_1b72_3b6a_73e0],
    revision: 0,
    response: UnsafeCell::new(ptr::null()),
    flags: 1, // ask Limine to enable x2APIC on every CPU when the hardware has it
};

#[repr(C)]
struct MpResponse {
    revision: u64,
    flags: u32,
    bsp_lapic_id: u32,
    cpu_count: u64,
    cpus: *const *mut MpInfo,
}

/// One CPU as described by Limine. Writing `goto_address` starts it.
#[repr(C)]
pub struct MpInfo {
    pub processor_id: u32,
    pub lapic_id: u32,
    _reserved: u64,
    goto_address: core::sync::atomic::AtomicU64,
    pub extra_argument: u64,
}

impl MpInfo {
    /// Starts this (parked) CPU at `entry`, which receives a pointer to this record.
    pub fn start(&self, entry: extern "C" fn(&'static MpInfo) -> !) {
        self.goto_address.store(entry as usize as u64, core::sync::atomic::Ordering::SeqCst);
    }
}

pub struct Cpus {
    pub bsp_lapic_id: u32,
    pub list: &'static [&'static MpInfo],
}

/// Every CPU Limine found (including the boot CPU), or None on a bootloader
/// that did not answer the request.
pub fn cpus() -> Option<Cpus> {
    // SAFETY: as in `Request::response`.
    let r = unsafe { ptr::read_volatile(MP_REQUEST.response.get()).as_ref() }?;
    if r.cpus.is_null() {
        return None;
    }
    // SAFETY: Limine guarantees `cpus` points to `cpu_count` valid MpInfo
    // pointers that live as long as the kernel (bootloader-reclaimable memory
    // is never reused). The records are only modified through atomics.
    let list = unsafe { core::slice::from_raw_parts(r.cpus as *const &'static MpInfo, r.cpu_count as usize) };
    Some(Cpus { bsp_lapic_id: r.bsp_lapic_id, list })
}

#[used]
#[unsafe(link_section = ".requests")]
static MODULE_REQUEST: Request<ModuleResponse> = Request::new([0x3e7e_2797_02be_32af, 0xca1c_4f3b_d128_0cee], 0);

#[repr(C)]
struct ModuleResponse {
    revision: u64,
    module_count: u64,
    modules: *const *const LimineFile,
}

#[repr(C)]
struct LimineFile {
    revision: u64,
    address: *const u8,
    size: u64,
    path: *const u8,
    // more fields follow that AWA does not use
}

/// A file Limine loaded next to the kernel (from `module_path:` in limine.conf).
pub struct Module {
    /// File name without directories, e.g. "init".
    pub name: &'static str,
    pub bytes: &'static [u8],
}

pub fn modules() -> alloc::vec::Vec<Module> {
    let mut out = alloc::vec::Vec::new();
    let Some(r) = MODULE_REQUEST.response() else { return out };
    if r.modules.is_null() {
        return out;
    }
    for i in 0..r.module_count as usize {
        // SAFETY: Limine guarantees `modules` holds `module_count` valid file
        // pointers whose data and NUL-terminated path stay mapped (bootloader-
        // reclaimable memory is never reused).
        let f = unsafe { &**r.modules.add(i) };
        if f.address.is_null() || f.path.is_null() {
            continue;
        }
        // SAFETY: as above; `size` bytes at `address`.
        let bytes = unsafe { core::slice::from_raw_parts(f.address, f.size as usize) };
        // SAFETY: as above; the path is a NUL-terminated string.
        let path = unsafe { core::ffi::CStr::from_ptr(f.path.cast()) }.to_str().unwrap_or("?");
        out.push(Module { name: path.rsplit('/').next().unwrap_or(path), bytes });
    }
    out
}

#[used]
#[unsafe(link_section = ".requests_end_marker")]
static REQUESTS_END: [u64; 2] = [0xadc0_e053_1bb1_0d03, 0x9572_709f_3176_4c62];

// ---------------------------------------------------------------- memory map

#[repr(C)]
struct MemmapResponse {
    revision: u64,
    entry_count: u64,
    entries: *const *const RawMemmapEntry,
}

#[repr(C)]
struct RawMemmapEntry {
    base: u64,
    length: u64,
    kind: u64,
}

/// Physical memory region kinds reported by Limine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemKind {
    Usable,
    Reserved,
    AcpiReclaimable,
    AcpiNvs,
    BadMemory,
    BootloaderReclaimable,
    KernelAndModules,
    Framebuffer,
    Other(u64),
}

impl MemKind {
    fn from_raw(v: u64) -> Self {
        match v {
            0 => Self::Usable,
            1 => Self::Reserved,
            2 => Self::AcpiReclaimable,
            3 => Self::AcpiNvs,
            4 => Self::BadMemory,
            5 => Self::BootloaderReclaimable,
            6 => Self::KernelAndModules,
            7 => Self::Framebuffer,
            other => Self::Other(other),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MemRegion {
    pub base: u64,
    pub length: u64,
    pub kind: MemKind,
}

/// The physical memory map, as copied at boot. Bootloader-reclaimable memory
/// is treated as in use for now (TODO(awa): reclaim it once nothing from
/// Limine is needed any more).
pub struct MemoryMap {
    entries: &'static [*const RawMemmapEntry],
}

impl MemoryMap {
    pub fn iter(&self) -> impl Iterator<Item = MemRegion> + '_ {
        self.entries.iter().map(|&p| {
            // SAFETY: Limine guarantees every entry pointer is valid and
            // points to a memory map entry that lives as long as the kernel
            // (bootloader-reclaimable memory is never reused, see above).
            let e = unsafe { &*p };
            MemRegion { base: e.base, length: e.length, kind: MemKind::from_raw(e.kind) }
        })
    }
}

pub fn memory_map() -> Option<MemoryMap> {
    let r = MEMMAP_REQUEST.response()?;
    if r.entries.is_null() || r.entry_count == 0 {
        return None;
    }
    // SAFETY: Limine guarantees `entries` points to `entry_count` entry
    // pointers, valid for the kernel's lifetime.
    let entries = unsafe { core::slice::from_raw_parts(r.entries, r.entry_count as usize) };
    Some(MemoryMap { entries })
}

// ---------------------------------------------------------------- HHDM and kernel address

#[repr(C)]
struct HhdmResponse {
    revision: u64,
    offset: u64,
}

/// Offset of the higher-half direct map: physical address P is at virtual P + offset.
pub fn hhdm_offset() -> Option<u64> {
    HHDM_REQUEST.response().map(|r| r.offset)
}

#[repr(C)]
struct ExecutableAddressResponse {
    revision: u64,
    physical_base: u64,
    virtual_base: u64,
}

/// Where the kernel image was loaded: (physical base, virtual base).
pub fn kernel_address() -> Option<(u64, u64)> {
    EXECUTABLE_ADDRESS_REQUEST.response().map(|r| (r.physical_base, r.virtual_base))
}

/// True if the bootloader accepted `BASE_REVISION` (it then zeroes the last
/// element of the tag).
pub fn base_revision_supported() -> bool {
    // SAFETY: the pointer comes from a static `[u64; 3]`, so it is valid and
    // aligned, and index 2 is in bounds.
    let last = unsafe { ptr::read_volatile(BASE_REVISION_TAG.0.get().cast::<u64>().add(2)) };
    last == 0
}

#[repr(C)]
struct FramebufferResponse {
    revision: u64,
    framebuffer_count: u64,
    framebuffers: *const *const RawFramebuffer,
}

#[repr(C)]
struct RawFramebuffer {
    address: *mut u8,
    width: u64,
    height: u64,
    pitch: u64,
    bpp: u16,
    memory_model: u8,
    red_mask_size: u8,
    red_mask_shift: u8,
    green_mask_size: u8,
    green_mask_shift: u8,
    blue_mask_size: u8,
    blue_mask_shift: u8,
    _unused: [u8; 7],
    edid_size: u64,
    edid: *mut u8,
}

const MEMORY_MODEL_RGB: u8 = 1;

static FRAMEBUFFER_TAKEN: AtomicBool = AtomicBool::new(false);

/// Hands out the first framebuffer, at most once.
pub fn take_framebuffer() -> Result<Framebuffer, &'static str> {
    if FRAMEBUFFER_TAKEN.swap(true, Ordering::AcqRel) {
        return Err("framebuffer already taken");
    }
    let response = FRAMEBUFFER_REQUEST
        .response()
        .ok_or("bootloader gave no framebuffer response")?;
    if response.framebuffer_count == 0 || response.framebuffers.is_null() {
        return Err("bootloader reported zero framebuffers");
    }
    // SAFETY: Limine guarantees `framebuffers` points to `framebuffer_count`
    // (checked > 0 above) valid pointers to framebuffer descriptions.
    let raw = unsafe { &**response.framebuffers };

    if raw.memory_model != MEMORY_MODEL_RGB || raw.bpp != 32 {
        return Err("unsupported pixel format (need 32-bit RGB)");
    }
    let fits = |size: u8, shift: u8| size == 8 && shift <= 24;
    if !fits(raw.red_mask_size, raw.red_mask_shift)
        || !fits(raw.green_mask_size, raw.green_mask_shift)
        || !fits(raw.blue_mask_size, raw.blue_mask_shift)
    {
        return Err("unsupported colour masks");
    }
    let (width, height, pitch) = (raw.width as usize, raw.height as usize, raw.pitch as usize);
    if pitch < width * 4 {
        return Err("framebuffer pitch smaller than a row");
    }

    // SAFETY: Limine maps the framebuffer (at least `pitch * height` bytes)
    // in the higher-half direct map for the kernel's lifetime. The
    // `FRAMEBUFFER_TAKEN` flag above guarantees this is the only owner.
    let fb = unsafe {
        Framebuffer::new(
            raw.address,
            width,
            height,
            pitch,
            [raw.red_mask_shift, raw.green_mask_shift, raw.blue_mask_shift],
        )
    };
    fb.ok_or("framebuffer address is null")
}

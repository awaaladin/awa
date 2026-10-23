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
#[unsafe(link_section = ".requests_end_marker")]
static REQUESTS_END: [u64; 2] = [0xadc0_e053_1bb1_0d03, 0x9572_709f_3176_4c62];

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

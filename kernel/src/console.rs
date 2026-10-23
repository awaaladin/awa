//! Framebuffer drawing: a raw pixel surface and a minimal text console.

use core::ptr::NonNull;
use noto_sans_mono_bitmap::{FontWeight, RasterHeight, get_raster, get_raster_width};

/// A linear 32-bit-per-pixel framebuffer owned exclusively by its holder.
pub struct Framebuffer {
    base: NonNull<u8>,
    width: usize,
    height: usize,
    pitch: usize,
    /// Bit shifts of the red, green and blue channels inside a pixel.
    shifts: [u8; 3],
}

impl Framebuffer {
    /// # Safety
    /// `base` must point to at least `pitch * height` bytes of mapped,
    /// writable memory that stays valid for the program's lifetime and that
    /// nothing else accesses. `pitch >= width * 4`.
    pub unsafe fn new(
        base: *mut u8,
        width: usize,
        height: usize,
        pitch: usize,
        shifts: [u8; 3],
    ) -> Option<Self> {
        Some(Self { base: NonNull::new(base)?, width, height, pitch, shifts })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn pitch(&self) -> usize {
        self.pitch
    }

    /// Writes one pixel; coordinates outside the screen are ignored.
    pub fn put_pixel(&mut self, x: usize, y: usize, color: Rgb) {
        if x >= self.width || y >= self.height {
            return;
        }
        let [rs, gs, bs] = self.shifts;
        let value = (u32::from(color.0) << rs) | (u32::from(color.1) << gs) | (u32::from(color.2) << bs);
        let offset = y * self.pitch + x * 4;
        // SAFETY: x < width and y < height, and pitch >= width * 4, so
        // `offset + 4 <= pitch * height`, inside the region `new` requires
        // to be valid and exclusively ours. Pixel rows are 4-byte aligned
        // because the base is page aligned and the pitch is a multiple of 4
        // for 32-bpp modes. Volatile because this is device memory.
        unsafe { self.base.as_ptr().add(offset).cast::<u32>().write_volatile(value) };
    }

    pub fn fill(&mut self, color: Rgb) {
        for y in 0..self.height {
            for x in 0..self.width {
                self.put_pixel(x, y, color);
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Blends `self` over `background` by `alpha` (0 = background, 255 = self).
    fn over(self, background: Rgb, alpha: u8) -> Rgb {
        let mix = |fg: u8, bg: u8| {
            ((u16::from(fg) * u16::from(alpha) + u16::from(bg) * (255 - u16::from(alpha))) / 255) as u8
        };
        Rgb(mix(self.0, background.0), mix(self.1, background.1), mix(self.2, background.2))
    }
}

/// Colours are part of the boot test's contract (`tools/awa.py`).
pub const BACKGROUND: Rgb = Rgb(0x10, 0x18, 0x20);
pub const FOREGROUND: Rgb = Rgb(0xE8, 0xF0, 0xFF);

/// Banner position and scale, also part of the boot test's contract.
pub const BANNER_X: usize = 32;
pub const BANNER_Y: usize = 32;
pub const BANNER_SCALE: usize = 2;

const FONT_WEIGHT: FontWeight = FontWeight::Regular;
const FONT_HEIGHT: RasterHeight = RasterHeight::Size16;

pub struct Console {
    fb: Framebuffer,
}

impl Console {
    pub fn new(fb: Framebuffer) -> Self {
        Self { fb }
    }

    pub fn clear(&mut self) {
        self.fb.fill(BACKGROUND);
    }

    /// Draws `text` at the banner position.
    pub fn draw_banner(&mut self, text: &str) {
        self.draw_text(BANNER_X, BANNER_Y, text, BANNER_SCALE);
    }

    fn draw_text(&mut self, x: usize, y: usize, text: &str, scale: usize) {
        let advance = get_raster_width(FONT_WEIGHT, FONT_HEIGHT) * scale;
        for (i, ch) in text.chars().enumerate() {
            let glyph = get_raster(ch, FONT_WEIGHT, FONT_HEIGHT)
                .or_else(|| get_raster('?', FONT_WEIGHT, FONT_HEIGHT));
            let Some(glyph) = glyph else { continue };
            let left = x + i * advance;
            for (row, line) in glyph.raster().iter().enumerate() {
                for (col, &alpha) in line.iter().enumerate() {
                    if alpha == 0 {
                        continue;
                    }
                    let color = FOREGROUND.over(BACKGROUND, alpha);
                    for dy in 0..scale {
                        for dx in 0..scale {
                            self.fb.put_pixel(left + col * scale + dx, y + row * scale + dy, color);
                        }
                    }
                }
            }
        }
    }
}

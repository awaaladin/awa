//! COM1 serial output (16550-compatible UART at I/O port 0x3F8).
//!
//! Used for logs and for the automated boot test. Many laptops (including
//! the Portégé Z30-A) have no UART; `init` detects that and output is then
//! silently dropped.

use crate::arch::port::{inb, outb};
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, Ordering};

const COM1: u16 = 0x3F8;
const DATA: u16 = COM1;
const INT_ENABLE: u16 = COM1 + 1;
const FIFO_CTRL: u16 = COM1 + 2;
const LINE_CTRL: u16 = COM1 + 3;
const MODEM_CTRL: u16 = COM1 + 4;
const LINE_STATUS: u16 = COM1 + 5;
const LINE_STATUS_TX_EMPTY: u8 = 1 << 5;

static PRESENT: AtomicBool = AtomicBool::new(false);

/// Programs COM1 for 115200 baud, 8N1, and checks that a UART is present.
pub fn init() {
    // SAFETY: 0x3F8..=0x3FF is the architecturally fixed COM1 range on PCs.
    // These registers only configure the UART; none of them start DMA. On
    // machines without a UART, the writes go nowhere.
    let present = unsafe {
        outb(INT_ENABLE, 0x00); // no interrupts
        outb(LINE_CTRL, 0x80); // DLAB on: next two writes set the divisor
        outb(DATA, 0x01); // divisor low byte: 115200 baud
        outb(INT_ENABLE, 0x00); // divisor high byte
        outb(LINE_CTRL, 0x03); // DLAB off, 8 data bits, no parity, 1 stop bit
        outb(FIFO_CTRL, 0xC7); // enable and clear FIFOs, 14-byte threshold
        outb(MODEM_CTRL, 0x1E); // loopback mode for the presence test
        outb(DATA, 0xAE);
        let echoed = inb(DATA) == 0xAE;
        outb(MODEM_CTRL, 0x0F); // normal operation: DTR, RTS, OUT1, OUT2
        echoed
    };
    PRESENT.store(present, Ordering::Relaxed);
}

fn write_byte(byte: u8) {
    // SAFETY: same COM1 register range as `init`; reading the line status
    // and writing the data register have no side effects beyond the UART.
    unsafe {
        while inb(LINE_STATUS) & LINE_STATUS_TX_EMPTY == 0 {}
        outb(DATA, byte);
    }
}

struct SerialWriter;

impl Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            if byte == b'\n' {
                write_byte(b'\r');
            }
            write_byte(byte);
        }
        Ok(())
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    if PRESENT.load(Ordering::Relaxed) {
        // Phase 1 runs on one CPU with interrupts off, so lines cannot
        // interleave. TODO(awa): take a lock once SMP is up (Phase 3).
        let _ = SerialWriter.write_fmt(args);
    }
}

#[macro_export]
macro_rules! serial_println {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!("{}\n", format_args!($($arg)*)))
    };
}

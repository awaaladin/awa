//! Architecture-specific code. Only x86_64 exists today.

mod x86_64;

pub use x86_64::{breakpoint, cpuid, halt_forever, idt, init, port, probe};

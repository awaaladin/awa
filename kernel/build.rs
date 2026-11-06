use std::fmt::Write;

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
    let script = std::path::Path::new(&dir).join("linker.ld");
    println!("cargo:rustc-link-arg-bins=-T{}", script.display());
    println!("cargo:rerun-if-changed={}", script.display());
    println!("cargo:rerun-if-changed=build.rs");

    // Interrupt entry stubs for all 256 vectors. Each pushes a dummy error
    // code (if the CPU pushes none), its vector number, and jumps to the
    // common entry. The stubs live in `.entry_text`, the page that stays
    // mapped while user programs run (kernel page-table isolation).
    const WITH_ERROR_CODE: [u32; 10] = [8, 10, 11, 12, 13, 14, 17, 21, 29, 30];
    let mut s = String::new();
    s.push_str(".section .entry_text, \"ax\"\n");
    for v in 0..256u32 {
        writeln!(s, ".balign 16\nawa_vector_{v}:").unwrap();
        if !WITH_ERROR_CODE.contains(&v) {
            s.push_str("  push 0\n");
        }
        writeln!(s, "  push {v}\n  jmp awa_trap_common").unwrap();
    }
    s.push_str(".section .rodata\n.balign 8\n.global awa_vector_table\nawa_vector_table:\n");
    for v in 0..256u32 {
        writeln!(s, "  .quad awa_vector_{v}").unwrap();
    }
    let out = std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo")).join("vectors.s");
    std::fs::write(out, s).expect("write vectors.s");
}

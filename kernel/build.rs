fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
    let script = std::path::Path::new(&dir).join("linker.ld");
    println!("cargo:rustc-link-arg-bins=-T{}", script.display());
    println!("cargo:rerun-if-changed={}", script.display());
}

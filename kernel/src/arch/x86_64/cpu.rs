//! Control registers, MSRs and CPU feature detection.

use core::arch::asm;

pub const MSR_EFER: u32 = 0xC000_0080;
pub const MSR_PAT: u32 = 0x277;
const EFER_NXE: u64 = 1 << 11;
const CR0_WP: u64 = 1 << 16;
const CR4_PGE: u64 = 1 << 7;
const CR4_UMIP: u64 = 1 << 11;
const CR4_SMEP: u64 = 1 << 20;
const CR4_SMAP: u64 = 1 << 21;

pub fn read_msr(msr: u32) -> u64 {
    let (lo, hi): (u32, u32);
    // SAFETY: only called with architectural MSRs that exist on every x86_64
    // CPU (EFER, PAT); rdmsr has no side effects.
    unsafe { asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags)) };
    (u64::from(hi) << 32) | u64::from(lo)
}

/// # Safety
/// The caller must know the effect of the write is sound (e.g. enabling NX
/// or programming PAT entries that match how memory is mapped).
pub unsafe fn write_msr(msr: u32, value: u64) {
    // SAFETY: the caller's contract above.
    unsafe { asm!("wrmsr", in("ecx") msr, in("eax") value as u32, in("edx") (value >> 32) as u32, options(nostack, preserves_flags)) };
}

const MSR_TSC_AUX: u32 = 0xC000_0103;

/// True if the CPU has RDTSCP (CPUID 0x8000_0001 EDX bit 27).
pub fn has_rdtscp() -> bool {
    super::cpuid(0x8000_0000, 0).0 >= 0x8000_0001 && super::cpuid(0x8000_0001, 0).3 & (1 << 27) != 0
}

/// Stores this CPU's scheduler index where RDTSCP returns it (ring 0 only can set it).
pub fn set_cpu_index(index: u32) {
    // SAFETY: IA32_TSC_AUX only holds a value RDTSCP reports; callers check `has_rdtscp`.
    unsafe { write_msr(MSR_TSC_AUX, u64::from(index)) };
}

/// This CPU's index, as set by `set_cpu_index`. Only call when `has_rdtscp`.
pub fn cpu_index_fast() -> u32 {
    let aux: u32;
    // SAFETY: RDTSCP reads the TSC and IA32_TSC_AUX; no memory access.
    unsafe { asm!("rdtscp", out("ecx") aux, out("eax") _, out("edx") _, options(nomem, nostack, preserves_flags)) };
    aux
}

/// True unless the CPU reports it is not affected by Meltdown (rogue data
/// cache load): IA32_ARCH_CAPABILITIES.RDCL_NO. CPUs that cannot tell us
/// (Haswell, for example) are treated as affected.
pub fn meltdown_affected() -> bool {
    let has_arch_caps = super::cpuid(7, 0).3 & (1 << 29) != 0;
    !(has_arch_caps && read_msr(0x10A) & 1 != 0)
}

pub fn read_cr3() -> u64 {
    let v: u64;
    // SAFETY: reading CR3 has no side effects.
    unsafe { asm!("mov {}, cr3", out(reg) v, options(nomem, nostack, preserves_flags)) };
    v
}

/// One RDRAND attempt. Only call when `features().rdrand` is true.
pub fn rdrand64() -> Option<u64> {
    let v: u64;
    let ok: u8;
    // SAFETY: RDRAND writes a register and the carry flag; callers check CPUID first.
    unsafe { asm!("rdrand {v}", "setc {ok}", v = out(reg) v, ok = out(reg_byte) ok, options(nomem, nostack)) };
    (ok == 1).then_some(v)
}

/// # Safety
/// `pml4_phys` must be a complete page-table hierarchy that maps the code,
/// stack and data the CPU is using at the moment of the switch.
pub unsafe fn write_cr3(pml4_phys: u64) {
    // SAFETY: the caller's contract above.
    unsafe { asm!("mov cr3, {}", in(reg) pml4_phys, options(nostack, preserves_flags)) };
}

/// Flushes one page's translation from this CPU's TLB.
pub fn invlpg(virt: u64) {
    // SAFETY: invlpg only drops a cached translation; it never breaks memory safety.
    unsafe { asm!("invlpg [{}]", in(reg) virt, options(nostack, preserves_flags)) };
}

/// CPUID.1:ECX bit 31: set by hypervisors, never by real hardware.
pub fn running_under_hypervisor() -> bool {
    super::cpuid(1, 0).2 & (1 << 31) != 0
}

pub struct Features {
    pub smep: bool,
    pub smap: bool,
    pub umip: bool,
    pub rdrand: bool,
    pub x2apic: bool,
    pub invariant_tsc: bool,
}

pub fn features() -> Features {
    let (_, _, ecx1, _) = super::cpuid(1, 0);
    let (_, ebx7, ecx7, _) = super::cpuid(7, 0);
    let (max_ext, _, _, _) = super::cpuid(0x8000_0000, 0);
    let invariant_tsc = max_ext >= 0x8000_0007 && super::cpuid(0x8000_0007, 0).3 & (1 << 8) != 0;
    Features {
        smep: ebx7 & (1 << 7) != 0,
        smap: ebx7 & (1 << 20) != 0,
        umip: ecx7 & (1 << 2) != 0,
        rdrand: ecx1 & (1 << 30) != 0,
        x2apic: ecx1 & (1 << 21) != 0,
        invariant_tsc,
    }
}

/// Which protections ended up enabled.
pub struct Protections {
    pub nx: bool,
    pub write_protect: bool,
    pub smep: bool,
    pub smap: bool,
    pub umip: bool,
}

/// Turns on NX, CR0.WP, global pages, and SMEP/SMAP/UMIP where the CPU has them.
/// Must run before the kernel's own page tables (which use NX and global bits) are loaded.
pub fn enable_protections() -> Protections {
    let f = features();
    // SAFETY: enabling NX only makes the NX bit in page tables meaningful;
    // CR0.WP makes the kernel honour read-only pages; PGE allows global
    // pages; SMEP/SMAP/UMIP only restrict ring 3 access paths, and no user
    // mappings exist yet. All bits are checked against CPUID first.
    unsafe {
        write_msr(MSR_EFER, read_msr(MSR_EFER) | EFER_NXE);
        let mut cr0: u64;
        asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
        cr0 |= CR0_WP;
        asm!("mov cr0, {}", in(reg) cr0, options(nostack, preserves_flags));
        let mut cr4: u64;
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
        cr4 |= CR4_PGE;
        if f.smep { cr4 |= CR4_SMEP; }
        if f.smap { cr4 |= CR4_SMAP; }
        if f.umip { cr4 |= CR4_UMIP; }
        asm!("mov cr4, {}", in(reg) cr4, options(nostack, preserves_flags));
    }
    super::syscall::set_smap(f.smap);
    Protections { nx: true, write_protect: true, smep: f.smep, smap: f.smap, umip: f.umip }
}

/// PAT layout used by AWA's page tables: index 0 write-back (default),
/// index 1 write-combining (framebuffer), the rest uncached variants.
pub fn program_pat() {
    // WB, WC, UC-, UC, WB, WC, UC-, UC (types: WB=6, WC=1, UC-=7, UC=0).
    let pat: u64 = 0x0007_0106_0007_0106;
    // SAFETY: entry 0 stays write-back, which is what every existing mapping
    // uses (Limine maps with PAT index 0). Entry 1 becomes write-combining,
    // used only for the framebuffer after AWA's tables are loaded.
    unsafe { write_msr(MSR_PAT, pat) };
}

/// Switches to `stack_top` and calls `next`, which never returns.
///
/// # Safety
/// `stack_top` must be the 16-byte aligned top of a mapped, writable stack
/// that nothing else uses.
pub unsafe fn switch_stack(stack_top: u64, next: extern "C" fn() -> !) -> ! {
    // SAFETY: the caller's contract above; the old stack is abandoned.
    unsafe { asm!("mov rsp, {top}", "xor ebp, ebp", "call {next}", "ud2", top = in(reg) stack_top, next = in(reg) next, options(noreturn)) }
}

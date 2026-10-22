# 01 · Kernel architecture

Status: draft · Not implemented

## 1. Shape of the kernel

AWA uses a **small core kernel** (microkernel-style) written in Rust. The kernel provides
mechanisms; policies live in user-space services wherever the hot path allows it.

**In the kernel (ring 0):**

| Component | Why it must be in the kernel |
|-----------|------------------------------|
| Capability tables and kernel-object lifetime | Root of all authority |
| Address spaces, page tables, VMOs, page-fault handling | Privileged hardware state; hot path |
| Memory classes, accounting, reclaim, compression | Run inside the page-fault and reclaim paths; a user-space round trip per fault would be too slow |
| Threads and the scheduler (all three classes) | Context switches are privileged and latency-critical |
| IPC (channels, events, ports) | Hot path for everything |
| Interrupt delivery (IRQ → event object) | Privileged |
| IOMMU (VT-d) programming | Only the kernel may decide which DMA a driver can do |
| Timers (APIC TSC-deadline, HPET fallback) | Privileged |
| Trace ring buffers and counters | Must be always on and cheap |
| VMX core: VMCS, VM entry/exit, EPT | VMX root operation is ring 0 only |

**Not in the kernel (user space):** device drivers including GPU, PCI enumeration, the
ACPI AML interpreter, file systems and VFS, the network stack, the window server, the
memory-*policy* service (what quotas to set; the kernel only enforces them), the
hypervisor's device models (one VMM process per VM box), the download broker, the
update agent and the shell.

## 2. Kernel objects

Every kernel object is reached only through a handle (see [04](04-capabilities.md)).

| Object | Purpose |
|--------|---------|
| `Process` | Address space + handle table + memory account + trace identity |
| `Thread` | Execution context; belongs to one process; has a scheduler class |
| `Vmo` | Range of pages with a memory class (Required / Recomputable / Streamable) |
| `Vmar` | Region of an address space into which VMOs are mapped (enables ASLR and sub-delegation) |
| `Channel` | Bidirectional message pipe carrying bytes + handles |
| `Event` / `EventPair` | Signal bits for waiting |
| `Port` | Waits on many objects at once; receives async notifications (pressure, IRQs, exits) |
| `Interrupt` | A hardware interrupt line or MSI vector delivered to a driver |
| `Mmio` / `IoPort` | Rights to a physical MMIO range or an I/O-port range |
| `DmaRegion` | Pinned pages with an IOMMU mapping for one device |
| `Account` | Memory account node (system → box → process) with quota |
| `SchedReservation` | Admitted frame-deadline budget (runtime / period) |
| `Gang` | Set of threads to be co-scheduled |
| `Box` | Sandbox: a set of accounts, capability policy, and (for VM boxes) a VM |
| `Vcpu` / `GuestMemory` | Hypervisor objects used by a VMM process |
| `Pager` | Supplies pages for Streamable VMOs (implemented by a storage-backed service) |
| `TraceSession` | Read access to trace buffers |
| `Resource` | Root capabilities handed to init (physical memory, IRQs, I/O, hypervisor) |

## 3. Boot sequence (kernel view)

1. Limine loads the kernel (a relocatable ELF, randomized base) and the **boot bundle**,
   and passes the memory map, framebuffer, RSDP, SMP info and higher-half direct map.
2. The kernel sets up per-CPU data, the GDT and IDT, serial and framebuffer logging, and
   physical memory. It parses static ACPI tables: MADT (CPUs and APICs), HPET, MCFG
   (PCIe ECAM) and DMAR (IOMMU).
3. It brings up the APs (other CPU cores), the scheduler and timers.
4. It creates the first process, **init**, from the boot bundle, giving it the `Resource`
   root capabilities and a read-only VMO of the bundle.
5. Init starts the early drivers from the bundle (PCI, storage, USB), verifies and mounts
   the system image, then hands over to the service manager.

The kernel never parses file systems, AML or device-specific formats.

## 4. Address-space layout (x86_64, 4-level paging)

```
User   0x0000_0000_0000_1000 .. 0x0000_7FFF_FFFF_F000   (ASLR'd: PIE image, stacks, heap, vDSO, mmap)
       0x0000_0000_0000_0000 page is never mapped
Kernel 0xFFFF_8000_0000_0000 ..                         higher-half direct map (from Limine; NX, no user)
       0xFFFF_C000_0000_0000 ..                         kernel heap, per-CPU, stacks w/ guard pages
       0xFFFF_FFFF_8000_0000 ..                         kernel image (randomized by Limine; W^X per section)
```

- 5-level paging is not needed (this hardware doesn't support it). The layout code is
  parameterized so it can be added.
- **Kernel page-table isolation** (separate user page tables with only the entry
  trampolines mapped) is **on by default** on CPUs that report Meltdown vulnerability,
  which includes Haswell. PCID is used to keep TLB flushes cheap.
- Kernel stacks have unmapped guard pages. Kernel heap allocations use a slab allocator
  for fixed-size kernel objects and a linked-list or buddy allocator for the rest.

## 5. SMP and concurrency inside the kernel

- Per-CPU run queues and per-CPU allocator caches.
- Fine-grained spinlocks with interrupts disabled. Lock ordering is documented per
  module and checked in debug builds by a lock-ordering validator.
- Kernel code does not sleep while holding a spinlock. Blocking waits use wait queues.
- TLB shootdowns use IPIs, batched per address-space operation.

## 6. Panics and faults

- A kernel panic prints a backtrace to serial and the framebuffer, writes a crash record
  to a reserved RAM region (read on next boot by the crash service), then halts. Reboot
  on panic is configurable.
- A user-space fault (page fault, #GP, #UD) produces an exception message on the
  process's exception channel. The supervisor (usually the service manager) decides
  whether to restart. **A driver crash does not crash the kernel.**

## 7. Rust and `unsafe` rules for the kernel

- Workspace layout: `kernel/` is a `no_std` binary crate. Hardware abstraction modules
  (`arch/x86_64/*`) are the only place inline assembly may appear.
- `#![deny(unsafe_op_in_unsafe_fn)]` and `#![deny(clippy::undocumented_unsafe_blocks)]`.
- Every module containing `unsafe` is listed in `kernel/UNSAFE.md` with an owner and a
  one-line justification. Adding a new one requires a design-log entry.
- Panic strategy: `panic = "abort"`. Allocation failure in the kernel returns errors
  (`try_*` APIs); it does not panic.
- Dependencies are permissively licensed, `no_std` and pinned. Each one is reviewed and
  listed in `docs/DEPENDENCIES.md` (created in Phase 1).

## 8. Open items

- TODO(awa): decide between a buddy allocator and a bitmap-plus-free-list physical frame
  allocator. This will be done in Phase 2 with a microbenchmark.
- TODO(awa): aarch64 port plan (not before the x86_64 kernel reaches Phase 5).

# 04 · Capabilities

Status: draft · Not implemented

## 1. Principle

**No ambient authority.** A process can only affect what it holds a handle to. There is
no global file namespace, no "root" user, and no syscall that takes a path, a PID or a
global name. A newly created process has exactly the handles its creator gave it.

## 2. Handles and rights

- A **handle** is a 32-bit per-process index into the process's handle table. Each entry
  holds a pointer to the kernel object, a **rights** bitmask and a generation counter.
  The value given to user space mixes the index with a per-process random tweak, so
  handle values cannot be guessed across processes and stale values are detected.
- Rights (common set; objects add their own):
  `READ, WRITE, EXECUTE, MAP, DUPLICATE, TRANSFER, SIGNAL, WAIT, INSPECT, MANAGE`.
  Examples of object-specific rights: `Vmo: RESIZE, SET_CLASS_HINTS`,
  `Account: SET_LIMIT, CREATE_CHILD`, `Process: SPAWN_THREAD, KILL`.
- **Attenuation:** `handle_duplicate(h, rights)` only ever creates a handle with the same
  or fewer rights.
- **Transfer:** handles move between processes only through channel messages, and only
  if the handle has `TRANSFER`.

## 3. Revocation

AWA uses **revocable forwarders** rather than a global derivation tree (simpler, with
bounded kernel memory):

- `revocable_create(h) -> (forwarder_handle, revoker_handle)`. The forwarder behaves
  exactly like the original. Calling `revoke(revoker)` makes every copy of the
  forwarder permanently invalid. Any in-progress operation completes; later
  operations fail with `REVOKED`.
- The powerbox, the box manager and the service manager hand out forwarders, so they
  can withdraw access later (for example when a box is reset).

## 4. Root capabilities

At boot, the kernel creates `Resource` capabilities (physical memory ranges, IRQ ranges,
I/O-port ranges, the hypervisor, the trace system, the root memory account) and hands
them **only** to init. Init passes narrowed pieces to drivers. For example, the AHCI
driver gets its controller's MMIO range, its MSI interrupt and a DMA capability limited
by the IOMMU to its own device. Nothing else.

## 5. Files and the powerbox

- File and directory access is a capability to a VFS node, with rights.
- Each process has a **namespace**: a table mapping path prefixes to directory handles.
  It is assembled by whoever launches the process. This is how POSIX paths work on top
  of capabilities. A process cannot name anything outside its namespace.
- **Powerbox:** when an app needs a file the user chooses (Open/Save), it asks the
  trusted file-picker service. The picker runs outside the app and grants a handle to
  exactly the file the user chose. Apps never need broad file access.

## 6. Devices and DMA

- A driver can only start DMA into `DmaRegion` objects. These are pages pinned and
  mapped in the device's IOMMU domain.
- **Without an IOMMU, DMA isolation is impossible.** A driver with DMA could overwrite
  any memory. On systems with no usable VT-d (including QEMU without `intel-iommu`, and
  possibly your laptop if firmware does not publish a DMAR table), drivers with DMA are
  marked **trusted** in the boot log and in `awa status`. This is reported openly, not
  hidden.

## 7. Boxes and capabilities

A box is defined by the capabilities its processes start with. A box gets:

- its own memory account (with quota)
- a namespace of only its own writable layer plus a read-only base
- no network capability unless granted (default deny)
- no clipboard, no window-server privileges beyond its own windows (window titles are
  tagged with the box name and a colored border drawn by the window server, which
  apps cannot spoof)

## 8. Open items

- TODO(awa): decide whether handle rights are checked in a single generic path or per
  syscall. Per syscall gives better error messages; generic gives fewer bugs.
  Prototype in Phase 4.

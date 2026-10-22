# 07 · Sandboxes, hypervisor and download broker

Status: draft · Not implemented

## 1. Two box tiers (proposal A7)

| | **Light box** | **VM box** |
|-|---------------|------------|
| Isolation | Capabilities: a process group with its own account, namespace and no ambient authority | Hardware virtualization (VT-x + EPT): a separate AWA guest kernel |
| Defends against | Malicious or buggy *apps* | Malicious apps **and** kernel exploits in AWA itself |
| Cost on 8 GB | Near zero (processes only) | Guest kernel + services, target ≤ 64 MiB idle per box (unmeasured) |
| Start time target | < 100 ms | < 1 s |
| Use for | Your own tools, trusted apps you want limits on | **Downloads, untrusted code, browsers** |

`box new <name>` creates a VM box by default. `box new <name> --light` creates a light box.
Downloads (`get`) always go into VM boxes.

Why two tiers: the brief asks for microVMs, and they are the right defense for untrusted
downloads. On an 8 GB / 2-core machine, putting *every* constrained workload in a VM
wastes the memory AWA is trying to save. Quotas, scheduling classes and capability
confinement work the same in both tiers.

## 2. Hypervisor architecture

- **Kernel (VMX core):** enables VMX, manages VMCS per vCPU, builds and maintains **EPT**
  from `GuestMemory` objects, handles fast exits in the kernel (CPUID, MSR filtering,
  APIC timer, EPT violations on paged guest memory) and forwards everything else.
- **VMM process (one per VM box, user space, unprivileged except for its own `Guest`
  capability):** device models (virtio-blk, virtio-net, virtio-console, virtio-vsock,
  virtio-gpu 2D, virtio-balloon) and the box lifecycle. A VMM compromise gives the
  attacker only that VMM's handles, never the host kernel or other boxes.
- **No device passthrough** in the first version.
- **Guest:** a minimal AWA build ("guest profile") with no hardware drivers, only virtio.
  It shares the same code base, so testing the OS tests the guest.

## 3. Storage layout of a box

- **Read-only base image**: a verified AWA guest system image, identical for all boxes.
  Its pages are shared across boxes by content identity ([02 §8](02-memory.md)), so ten
  boxes pay for the base image roughly once.
- **Writable layer**: a per-box copy-on-write block device stored as a file on the
  encrypted data partition.
- **Disposable** boxes discard the writable layer when stopped. **Persistent** boxes
  keep it. `box reset <name>` replaces the writable layer with an empty one (one action,
  a few ms, no reinstall).

## 4. Default policy for every box

- **Network: denied** unless the owner grants it (`box net <name> allow <host>`, or a
  Settings toggle). Granted network goes through the host network stack with per-box
  filtering, not raw device access.
- **Files: none** of the host's files. Files cross into a box only through the
  download broker or an explicit "send to box" action, and out only through an explicit
  "export from box" action.
- **Clipboard: not shared.**
- **Display:** box windows are drawn with a colored border and the box name by the host
  window server. Apps in boxes cannot draw outside their windows or read the screen.
- **Memory and CPU:** each box has its own account (quota) and defaults to the Batch or
  Interactive class. VMs cannot obtain frame-deadline reservations unless granted.

## 5. Download broker

`get <url> --into <box>`:

1. The broker validates the URL. If `--into` is missing, it asks the owner which box
   should receive it, offering "new disposable box".
2. The fetch runs **inside the target box**, which receives a **one-shot network grant**
   limited to that host and port for the duration of the download. Untrusted bytes (TLS,
   HTTP, the file itself) are never parsed by the host.
3. The file lands in the box's `~/Downloads`. The host records URL, size, hash and time
   in the box's log.
4. Exporting the file to the host is a separate, explicit owner action. It shows the
   file type detected inside the box and the file's hash.

Downloads started by apps (browsers inside a box) stay in that box.

## 6. Memory for VM boxes

- Guest RAM is a `GuestMemory` VMO charged to the box account. The guest kernel reports
  freed pages to the host with virtio-balloon **free page reporting**, so idle boxes
  give memory back.
- Guest memory classes: the guest's Recomputable and Streamable memory map to host-side
  hints over a paravirtual channel (`virtio-awa-mem`), so host pressure can reach inside
  guests. TODO(awa): protocol design at the hypervisor phase.

## 7. Security notes

- On Haswell, **L1TF** and **MDS** affect VM isolation. The VMX core flushes L1D on VM
  entry (where microcode supports it) and clears CPU buffers. With SMT on, a malicious
  guest on one hyperthread can still attack the sibling. **VM boxes running untrusted
  code use core scheduling**: a vCPU never shares a physical core with host or other-box
  threads. This costs throughput and is the documented price of isolation on this CPU.
- See [SECURITY.md](../SECURITY.md) for the VMM and broker threat models.

## 8. AMD-V

The VMX core sits behind an `arch::virt` interface. AMD-V (SVM + NPT) support comes
later. Your laptop is Intel, so VT-x is first.

## 9. Testing limits (proposal A9)

- QEMU without KVM (TCG) **does not emulate VT-x**. On Windows 10, WSL2 does not provide
  nested KVM. So **VM-box features cannot be tested automatically on this PC.**
- Plan: unit and integration tests for everything except actual VM entry run in QEMU.
  VM entry is tested (a) on the real laptop booted from USB, with results captured over a
  USB-serial adapter or written to the data partition, and (b) optionally on a Linux
  machine with nested KVM, where it can be automated.
- Windows reports "virtualization firmware enabled: False" on your laptop. Check that
  **VT-x is enabled in the BIOS setup** before the hypervisor phase (Hyper-V running in
  Windows can also cause this report).

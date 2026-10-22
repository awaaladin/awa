# 05 · Syscalls and IPC

Status: draft · Not implemented. These interfaces are hard to change later; review closely.

## 1. Syscall mechanism

- Entry via the `syscall` instruction; return via `sysretq` (with a canonical-address
  check on the return RIP, to avoid the known `sysret` #GP-in-ring-0 problem).
- Registers: `rax` = syscall number; arguments in `rdi, rsi, rdx, r10, r8, r9`. The
  return value is `rax`: `0` or a positive value on success, a negative `Status` on error.
- Every syscall takes handles, never paths or global IDs.
- User pointers are validated (range below the user limit) and accessed only through
  `copy_from_user` / `copy_to_user` helpers. These recover from faults through an
  exception table. Haswell has no SMAP, so these helpers are the only defense against
  the kernel accidentally reading user memory. With SMAP present, `stac`/`clac` bracket them.

## 2. The vDSO is the ABI (proposal A10)

- Every process gets the read-only **AWA vDSO** mapped at a random address. It exports
  functions such as `awa_channel_write`. **Syscall numbers are private** and may change
  between kernel versions; programs link against the vDSO.
- The kernel records the vDSO's address in each process and **rejects `syscall`
  instructions executed from anywhere else**. This blocks injected syscall gadgets.
- The vDSO also gives fast, syscall-free access to time (TSC + calibration page).

## 3. Initial syscall set (grouped)

| Group | Calls |
|-------|-------|
| Handles | `handle_close`, `handle_duplicate`, `handle_replace`, `object_get_info`, `object_wait_one`, `object_wait_many` |
| Revocation | `revocable_create`, `revoke` |
| Process/thread | `process_create`, `process_start`, `process_exit`, `thread_create`, `thread_start`, `thread_exit` |
| Memory | `vmo_create(class, size)`, `vmo_create_paged(pager)`, `vmo_read/write`, `vmo_clone(cow)`, `vmo_lock_chunk`, `vmo_unlock_chunk`, `vmo_advise`, `vmar_map`, `vmar_unmap`, `vmar_protect`, `vmar_allocate` |
| Accounts | `account_create`, `account_set_limits`, `account_get_stats`, `account_pressure_port` |
| IPC | `channel_create`, `channel_write`, `channel_read`, `channel_call` (write+wait+read, fast path), `event_create`, `object_signal`, `port_create`, `port_wait`, `port_queue` |
| Scheduling | `sched_set_class`, `sched_reserve`, `gang_create`, `gang_add`, `thread_set_affinity` |
| Time | `clock_get` (vDSO fast path), `timer_create`, `timer_set`, `nanosleep` |
| Devices | `interrupt_create`, `interrupt_ack`, `mmio_map`, `ioport_enable`, `dma_region_create` |
| Hypervisor | `guest_create`, `guest_memory_map`, `vcpu_create`, `vcpu_resume`, `vcpu_read_state`, `vcpu_write_state`, `vcpu_interrupt` |
| Trace | `trace_session_create`, `trace_read`, `trace_emit` (user events) |
| Debug (dev builds) | `debug_write` (serial), `debug_read` |

## 4. Channels

- A channel is a pair of endpoints. A message is **up to 64 KiB of bytes plus up to 64
  handles**, delivered whole (no partial reads).
- Messages queue on the receiving endpoint. Each endpoint has a bounded queue
  charged to the **sender's** memory account, so a flood fills the attacker's own quota.
- `channel_call` is the **synchronous fast path**: write a request, block, and receive the
  reply directly. The kernel switches straight to the server thread waiting in
  `channel_read`, passing small messages (≤ 128 bytes) in registers and donating the
  caller's scheduling context (see [03 §6](03-scheduler.md)).
- Bulk data uses **shared VMOs plus event signalling** (ring buffers set up once over a
  channel). This is how drivers, the GPU service and the window server move large buffers.

## 5. Message format

```
offset  size  field
0       4     txid      (0 = one-way; otherwise matched by the reply)
4       4     flags     (bit 0: is_reply, bit 1: is_epitaph/error)
8       8     ordinal   (method id = first 64 bits of BLAKE3("Protocol.Method"))
16      ...   payload   (little-endian, 8-byte aligned, AWA wire encoding)
handles carried out-of-band, referenced from payload by index
```

## 6. Interface definitions (IDL)

- Protocols are defined in Rust using a small declarative macro or attribute
  (`#[awa::protocol]`), which generates client and server stubs and a **schema hash**.
  At connection time each side checks the schema hash and protocol version.
- The wire encoding is defined by the schema. No reflection, no allocation needed to
  decode fixed-size messages. A C header generator comes in the POSIX phase.
- Every protocol states its **threat model**: who may hold the client end, and what the
  server must validate.

## 7. Errors

`Status` codes include `OK, BAD_HANDLE, WRONG_TYPE, ACCESS_DENIED, INVALID_ARGS,
OUT_OF_RANGE, NO_MEMORY, SHOULD_WAIT, PEER_CLOSED, TIMED_OUT, REVOKED, NOT_SUPPORTED,
BAD_STATE, CANCELED, INTERNAL`. The list is extensible; the numeric values are part of the vDSO ABI.

## 8. Executable format and process start

- **ELF64**, statically or dynamically linked. All user programs must be **position
  independent** (PIE) to allow ASLR. Non-PIE ELF is rejected.
- The kernel loads only init. Every other process is built by the **launcher** library
  in user space: it creates the process, maps segments with exact W/X permissions, maps
  the vDSO and stack at random addresses, then hands over a **startup message** on a
  bootstrap channel (arguments, environment, namespace, handles).
- No `fork()` in the native API. The POSIX layer emulates it with `vmo_clone(cow)` for
  programs that need it.

# 02 · Memory: classes, accounting, quotas, pressure

Status: draft · Not implemented

This is AWA's main differentiator, so it is specified in the most detail.

## 1. Memory classes

Every VMO has exactly one class, fixed at creation.

| Class | Meaning | What the kernel may do under pressure |
|-------|---------|----------------------------------------|
| **Required** | Program state that cannot be rebuilt (heap, stacks, unsaved edits) | Compress it; write it to swap as a last resort; never discard it |
| **Recomputable** | Caches the app can rebuild (decoded frames, thumbnails, BVH caches, tokenized data, preview renders) | **Discard** unlocked ranges, then notify the app |
| **Streamable** | Disk-backed, read-mostly data (model weights, image sequences, texture libraries, mapped files) | Drop clean pages at any time and re-read them on fault through the VMO's pager |

Default classes for code that does not use libawa: anonymous memory (`mmap`
anonymous, heap) is Required, and file mappings are Streamable. Unmodified programs
work correctly. They just don't get the Recomputable benefit.

## 2. Recomputable memory: the lock protocol

The kernel cannot discard memory that an app is reading right now, so recomputable
memory uses an explicit lock protocol. This follows a known pattern (purgeable or
volatile memory), with AWA-specific accounting added.

- A recomputable VMO is divided into **chunks** chosen by the app (minimum one page).
- A chunk is either **locked** (in use, never discarded) or **unlocked** (may be discarded).
- `lock(chunk)` returns `Intact` or `Discarded`. If it returns `Discarded`, the pages are
  now zero-filled and locked, and the app must rebuild the chunk.
- Touching an unlocked chunk is allowed: it reads either the old content or zeros.
  Safe Rust APIs in libawa only expose chunk contents through a lock guard, so that
  race cannot appear in safe code.
- Locked recomputable bytes count against the quota exactly like Required memory.
  Unlocked bytes count as "reclaimable" in reports.

```rust
// libawa (sketch — API is a proposal)
let cache = awa::mem::Recomputable::new(256 * MIB, ChunkSize(2 * MIB), Priority::Low)?;
cache.set_notifier(&port, key)?;            // optional: async "chunks were discarded" event
{
    let guard = cache.lock(chunk_idx)?;     // -> Lock::Intact(g) | Lock::Discarded(g)
    match guard {
        Lock::Intact(g) => use_frame(&g),
        Lock::Discarded(mut g) => { decode_frame_into(&mut g); use_frame(&g) }
    }
}                                           // guard drop = unlock; chunk is reclaimable again
```

`Priority` (Low / Normal / High) orders discards within one account. The **rebuild cost
hint** (an estimate of how many microseconds a chunk costs to rebuild) is optional. The
reclaimer uses it to prefer cheap-to-rebuild memory, and the tradeoff report uses it.

## 3. Streamable memory

- A streamable VMO is created against a **pager**: a capability to a service that
  supplies page contents. Usually this is the VFS or a storage driver. Initially the kernel
  sends the pager an IPC request for each fault batch.
- **Read-ahead** is driven by access-pattern detection (sequential / strided / random)
  plus explicit hints: `advise(range, Sequential | WillNeed(range) | DontNeed(range))`.
- Clean pages are dropped freely under pressure. Dirty pages (writable mappings) are
  written back through the pager before reuse.
- **Model weights and image sequences** are the main use. A 13 GB weight file can be
  mapped on an 8 GB machine; only the pages that are actually used stay resident.
- Streamable mappings of the same file object share pages. This is the basis of dedup
  (§8).

## 4. Accounting

- Memory is charged to an **account tree**: `system → {box…} → {process…}`. A process may
  create child accounts for subsystems (for example "texture cache") if it holds the
  right capability.
- Every resident physical page has exactly one **owner account**. Private pages are
  owned by the process that faulted them in. Shared pages are owned by the **shared
  object** (a file VMO or a deduplicated asset), which belongs to the account that
  created it, usually a box or the system.
- Reports show both **charged** bytes (what counts against the quota) and **proportional**
  bytes (shared pages divided among their mappers). This means `ram show` totals add
  up and still show who benefits from sharing.
- Counters kept per account (always on, see [10](10-tracing.md)): resident by class,
  compressed bytes (and their compressed size), swapped bytes, discards, refaults
  (with refault distance), page-ins from pagers, and peak resident.
- **GPU memory is accounted in the same tree** with a separate counter per memory domain
  (system RAM vs. device VRAM). See [06](06-gpu.md).

## 5. Quotas and "what the limit costs"

- Each account has `limit` (hard) and `target` (soft) values. Exceeding `target`
  triggers local reclaim in that account. Reaching `limit` makes new allocations wait
  for reclaim. If reclaim cannot free memory, the allocation fails with `NO_MEMORY`
  and the account's **pressure port** is signalled at `Critical`. The OOM policy (which
  process to kill, if any) lives in the user-space memory-policy service. The kernel
  never kills anything by itself.
- The owner can change limits at runtime with `ram set` or the Settings app. Lowering a
  limit is never instantaneous: reclaim runs within the new limit over a bounded time
  window, and the result is reported.

**Tradeoff report.** Lowering a limit produces a report built from two sources:

1. **Kernel-measured, available for every app:**
   - working-set estimate from refault distances: how much RAM the app actively uses
   - predicted extra refaults per second at the new limit (from the refault-distance
     histogram)
   - recomputable chunks that would be discarded, multiplied by the declared rebuild cost
   - how much would be compressed vs. swapped, and the expected CPU cost of compression
2. **App-declared, only for apps using libawa:** an app registers **quality levels**:
   ```rust
   awa::budget::declare(&[
       Level { min_bytes: 6 * GIB, describe: "batch 32, full-res preview" },
       Level { min_bytes: 3 * GIB, describe: "batch 16, half-res preview" },
       Level { min_bytes: 1 * GIB, describe: "batch 4, preview off" },
   ])?;
   ```
   When the account crosses a level boundary, the app gets a `BudgetChanged(level)`
   event and is expected to adapt. The report quotes the app's own text.

The report never invents app-level consequences for apps that did not declare them.
For those, it shows only the kernel-measured cost (proposal A4).

## 6. Pressure handling order

Global and per-account reclaim use the same ladder. They stop as soon as enough memory
is free:

1. **Discard unlocked Recomputable chunks**, lowest priority and cheapest rebuild first,
   oldest first. Notify owners.
2. **Drop clean Streamable pages** (least-recently-used, scaled by refault distance).
   *(This step is an addition to the brief, proposal A1. Clean disk-backed pages cost no
   writes to drop, and re-reading them is often cheaper than decompressing.)*
3. **Compress** cold Required pages (and dirty Streamable pages awaiting writeback) into
   the in-RAM compressed store.
4. **Page out** to the swap area: compressed pages, coldest first, written in batches.
   Swap lives on the encrypted data partition. On USB flash media, swap is **off by
   default** to protect the drive's lifespan. Swap is on by default on SSD.
5. **Signal Critical** and let the memory-policy service decide (shrink limits, ask the
   user, kill an idle background app).

Pressure levels (Normal / Moderate / High / Critical) are sent to each account's
pressure port, both for global pressure and for that account's own quota. Apps using
libawa get a callback before step 1, so they can drop caches themselves.

## 7. Compressed memory

- In the kernel, because it runs on the fault path.
- Algorithm: LZ4-class (fast). The crate choice depends on a Phase 5 benchmark;
  `lz4_flex` (MIT, `no_std`) is the first candidate. Zero-filled and same-byte pages are
  stored as a marker instead of being compressed.
- Compressed pages live in a zsmalloc-style pool of size classes. The pool's own memory is
  charged to the owning account at its compressed size.
- Pages that compress worse than 75 % of a page are skipped and marked incompressible
  for a cool-down period.
- Compression and decompression CPU time is charged to the owning account in the
  trace counters.

## 8. Shared read-only memory and deduplication

**Primary mechanism: content-addressed sharing.**

- The VFS gives every immutable file object (system image files, read-only assets,
  model files opened read-only) a content identity (a BLAKE3 hash of the file). Two
  mappings of the same identity share one set of physical pages, even if the paths
  differ or they come from different boxes.
- libawa offers `awa::share::publish(name, vmo)`, which makes a read-only, sealed VMO
  others can map by capability. Ten tools mapping the same weights through it pay for
  one copy, charged once to the account that published it.
- VM boxes share their read-only base image pages with each other the same way. The
  VMM maps the same host frames read-only through EPT.

**Not done by default: blind scanning dedup** (periodically hashing anonymous pages and
merging identical ones). It is a proven side channel: an attacker can time
copy-on-write faults to learn whether another tenant has a given page. AWA may later
offer opt-in scanning **within one account or box**, never across trust boundaries
(proposal A2).

## 9. Page sizes

- 4 KiB base pages. **2 MiB huge pages** are used for large Required or Streamable
  regions when alignment allows (model weights, frame buffers). Compression and
  discard work at 4 KiB granularity, so a huge page is split first.
- 1 GiB pages are only used for the kernel's direct map.

## 10. Kernel memory

Kernel heap and kernel objects are charged to the account of the process that caused
their creation (handle tables, channel message buffers, page tables). That way a hostile
process cannot exhaust kernel memory beyond its own quota.

## 11. Open items

- TODO(awa): pager protocol batching and fault-around size. This will be benchmarked in
  Phase 5 against a 4 GiB weight file.
- TODO(awa): whether recomputable chunks can be backed by huge pages without a high
  split cost.
- TODO(awa): exact rules for working-set estimation. The plan is refault-distance
  shadow entries; parameters to be validated in Phase 5.

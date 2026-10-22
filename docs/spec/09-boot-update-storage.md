# 09 · Boot, update chain and disk layout

Status: draft · Not implemented

## 1. Disk layout on the USB drive / external SSD

The owner asked for "whatever works and is easiest for people". The design is an
**immutable, A/B-updated system plus an encrypted data partition**. The user never sees
the partitions: they flash one image file, boot, and set a passphrase.

| # | Partition | Size (8 GB+ media) | Contents |
|---|-----------|--------------------|----------|
| 1 | EFI System Partition (FAT32) | 256 MiB | Limine (signed), configs, kernels + boot bundles for slots A and B |
| 2 | System A (read-only) | 2 GiB | AWA system image with hash tree |
| 3 | System B (read-only) | 2 GiB | Previous or next version |
| 4 | Data (encrypted) | rest | Owner files, settings, box writable layers, swap file (SSD only) |

- First boot **grows the data partition** to fill the drive and runs setup: passphrase,
  keyboard layout, time zone.
- Minimum media size: 8 GiB. Recommended: a USB 3 SSD (flash drives are slow at random
  writes and wear out faster).

## 2. Boot chain

**Interim chain (Phases 1–N, with Limine):**

1. UEFI firmware with **Secure Boot** using an AWA key the owner enrolls once (documented
   procedure; Secure Boot can also be off during development, which is reported).
2. **Limine**, signed with the AWA key. Its config file's hash is **enrolled into the
   Limine binary** (`limine enroll-config`), and the config pins the BLAKE2B hash of each
   kernel and boot bundle. Limine refuses modified files.
3. **Kernel + boot bundle** for the active slot. Init's early storage drivers find the
   system partition and **verify the system image's signed root hash** (Ed25519) against
   the key compiled into the kernel. Every block read is then checked against the
   hash tree.
4. Services start from the verified image. The data partition is unlocked with the
   owner's passphrase (Argon2id → AES-256-XTS key; Haswell has AES-NI).

Weakness (proposal A8): Limine does not natively implement A/B slot selection with
boot counting. Updating the kernel requires re-enrolling Limine's config, which means
re-signing Limine. The interim updater does this with the AWA signing key **on the
build machine**, not on the device. Fully atomic, rollback-safe boot needs our own loader.

**Final chain (own UEFI loader, later milestone):** a small Rust UEFI application, signed
for Secure Boot, which reads a boot-control block (active slot, tries remaining, last
good), verifies the slot's kernel + bundle signature, and falls back automatically if a
slot fails to boot three times.

## 3. Updates

- An update is a **signed bundle** (Ed25519 signature over a manifest listing hashes of
  the kernel, boot bundle and system image) with a **monotonic version number**.
- The update agent writes the new image into the **inactive slot**, verifies it, and marks
  it "try next boot". After a successful boot (services healthy for 60 s) it is marked
  good. Otherwise the loader returns to the old slot.
- **Anti-rollback**: the device refuses bundles with a version lower than the stored
  minimum. The minimum is stored on the data partition (and later in TPM NV storage if a
  TPM is available; your laptop may have TPM 1.2, unverified).
- Signing keys live offline. TODO(awa): key-management procedure document before the
  first public release.

## 4. System image format

- A read-only, content-addressed image: a file tree serialized with each file's BLAKE3
  hash, followed by a Merkle hash tree over 4 KiB blocks. The signed root hash is in the
  update manifest.
- Content hashes give the VFS file identities for free, which powers page sharing
  ([02 §8](02-memory.md)).

## 5. Data file system

- Needs crash safety (USB drives get unplugged). Initial plan: an **AWA copy-on-write
  file system** with checksums, written in Rust, as a user-space service. This is a large
  piece of work, scheduled in the Storage phase.
- Interim option: **FAT32/exFAT-style** support is required anyway (for the ESP and for
  reading other USB sticks), but is not crash-safe enough for the data partition.
- TODO(awa): file-system design document at the Storage phase.

## 6. Drivers needed to boot from USB on real hardware

After firmware hands over, UEFI boot services are gone. The kernel needs its own drivers
to read the system partition: **xHCI → USB mass storage (BOT, later UAS)**. These live
in the **boot bundle**. For the internal mSATA drive, AHCI is needed. In QEMU, virtio-blk
and AHCI are used first because they are simpler.

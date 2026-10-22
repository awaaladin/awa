# 11 · User-facing commands and Settings

Status: draft · Not implemented

## 1. Commands

All commands follow `noun verb [args]`. Sizes accept `512M`, `4G`, `1.5G`. Every
command supports `--json` for scripting.

```
ram show                       per-app and per-box RAM: used, by class, shared, compressed, limit
ram set <box|app> <size>       change a limit at runtime; prints the tradeoff report
ram set <box|app> none         remove a limit

gpu show                       per-app/box GPU memory (RAM/VRAM), busy %, preemptions, resets

box new <name> [--light] [--persistent]   create a box (VM box, disposable, by default)
box run <name> <app> [args]    run an app inside a box
box reset <name>               discard the box's writable layer
box list                       boxes, state, RAM, network grants
box net <name> allow|deny <host>          (addition) manage a box's network grants
box rm <name>                  delete a box

get <url> --into <box>         download into a box via the download broker
```

`box list`, `box net` and `box rm` are additions to the brief. A box is otherwise
impossible to see, open to the network or delete.

Example (target output, not real data):

```
$ ram set renders 3G
renders: limit 6.0 GiB -> 3.0 GiB
  measured:  working set ~3.4 GiB; expect ~120 extra page refaults/s (~2% slower)
  blender:   "batch 16, half-res preview" (was "batch 32, full-res preview")
  cache:     410 MiB of thumbnails will be dropped and rebuilt on demand
Applied. Reclaiming... done in 1.8 s.
```

## 2. Settings app

- **Memory** page: a live list of apps and boxes with stacked bars (Required /
  Recomputable / Streamable / compressed / shared), each with a limit slider. Moving a
  slider shows the tradeoff report in plain language *before* applying, then an Apply
  button.
- **GPU** page: the same for GPU memory and time.
- **Boxes** page: create, reset, delete, network grants, persistent/disposable toggle.
- **Security** page: Secure Boot state, IOMMU state ("drivers isolated" / "drivers
  trusted"), mitigations active, update status.
- Text is plain language; technical detail is behind "Details".

## 3. Shell

A native AWA shell (`userland/sh`) with these commands built in comes before the POSIX
layer. Phase 5 delivers a minimal serial shell with `ram show` only.

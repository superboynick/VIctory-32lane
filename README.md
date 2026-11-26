# VIctory, 32-lane branch

A fork of [fxceless/victory-fork](https://github.com/fxceless/victory-fork) carrying the changes I
needed on 32-lane NVIDIA hardware, plus the measurements behind them. Same layer, same goals;
everything here is either a defect fix or an observation switch, and everything is off by default
unless it is a fix.

Built and measured on an RTX 5070 Ti Laptop (12 GB, driver 617.14) and an RTX 3080 Ti (12 GB),
Windows 11, Series 1.0.2.1. No game files are hosted or linked here.

## What this branch changes on top of upstream

Picked from the nine upstream pull requests I opened, now on one branch:

| Area | Change | Evidence |
|---|---|---|
| Shader validation | Signature elements of differing interpolation modes no longer share a row; container signature parts rebuilt in name order; unused LOD slot written as `undef` | `--validate` errors over the full pack: 158 → 72, all 14,199 emitted vertex and pixel programs validate |
| Wave width | A shader that cannot observe its wave width is narrowed to 32 after optimisation | Refused compute pipelines 5 → 4 on a 32-lane host; `369cbe906ea72764` created and bound at frame 1408 |
| Frame capture | Buffer size computed with checked arithmetic and a per-frame ceiling; capture can be scheduled by wall clock and emit a depth image beside the colour frame | Old build died in the allocator at 4K (`7763201452000 bytes`); new one captures; wall-clock schedule makes two runs comparable |
| Diagnostics | Opt-in traces for HLE exports, DirectStorage reads, PSO creation failures, audio submissions/completions, present intervals with per-window summaries and per-stall attribution | See below |
| Preflight | `victory check` verifies every required archive is present *and openable*, and warns before startup if not | The startup `int3` I chased for a day was a missing/locked archive, not a mount defect |
| Docs | `docs/diagnosing.md` records what each measurement answers and what it cannot | — |

## Switches this branch adds

All inert unless set.

```
VICTORY_TRACE_HLE=1            export names at dispatch; paths for file API calls; read lengths
VICTORY_TRACE_READS=1          per-read offset/length/returned bytes, 4-byte header check at offset 0
VICTORY_TRACE_PSO=1            on failure only: kind, hash, root params, descriptor counts, flags, blob length, HRESULT
VICTORY_TRACE_AUDIO=1          per submission: bytes, pending, queue, positions, FNV-1a hash of first 64 bytes
VICTORY_TRACE_SIGNATURE=1      per program: component counts and estimated signature bytes
VICTORY_LOG_PRESENT=1          swap chain configuration, and present arguments when they change
VICTORY_PRESENT_SYNC=n         explicit sync interval passed to Present
VICTORY_PRESENT_TIMES=<path>   per-present intervals, window rows, stall attribution (VICTORY_STALL_LOG=<ms>)
VICTORY_CAPTURE_AT=<secs>      capture by wall clock instead of frame number
VICTORY_CAPTURE_DEPTH=1        write <frame>-depth.png beside the colour frame
```

## Two measurements worth knowing

**Frame rate hides hitching.** The prologue reads 30 fps and still freezes: 1500 presents over
72.8 s gave p50 33.4 ms, p95 105.4 ms, p99 370.8 ms, worst 3527 ms, with 77 presents over 100 ms,
against p99 34.1 ms and three such presents for the same build sitting in a menu. 81 of 91 stalls
carried a texture upload count; shader translation appeared in a handful.

**A startup breakpoint can mean "an archive is missing".** Tracing the last file operations before
the `int3` showed one archive returning `0xFFFFFFFF`. The same breakpoint also appears when that
archive exists but another process holds it open (`os error 32`). A directory listing cannot tell
those apart; `victory check` can, because it tries to open each file.

## What is not fixed here

72 mesh programs still exceed the combined vertex and primitive output limit of 32768 components.
Their overshoot is 128 to 896 components with no primitive outputs at all, and neither tighter row
packing nor dropping unread outputs helps: the limit counts components, and the emitter has no
consumer information at signature build time. This is a budget question, not a code defect.

## Building

```
cargo build --release -p victory
```

A 32-lane host wants `VICTORY_HOST_WAVE=32` for a comparable translation census. Validating a
census needs `dxil.dll`, which most machines do not have; `VICTORY_DXIL` points at one taken from
the DXC releases. `docs/diagnosing.md` has the details.

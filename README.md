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

## Reproducing the numbers in this repository

Everything quoted above came from the retail title on a 32-lane NVIDIA host. The commands are
short; the two things that trip people up are the validator and the wave width.

```
cargo build --release -p victory
```

Binding, without starting the title - expect `88 hle, 427 host, 72 title, 0 missing`:

```
victory run <path to the title executable>
```

A translation census that is comparable to the numbers in the first table:

```
# dxil.dll is not present by default; take dxc_<date>.zip from the DXC releases
set VICTORY_DXIL=<path>\bin\x64\dxil.dll
set VICTORY_HOST_WAVE=32
victory translate <shaders.rpf> --validate --pipelines --list
```

To look at a handful of programs instead of twenty-six thousand, put the sixteen hex digits a log
names a shader by into `VICTORY_TRANSLATE_ONLY`.

Hitching, which a frame rate of 30 never shows:

```
set VICTORY_PRESENT_TIMES=out.csv
set VICTORY_STALL_LOG=100
victory run <title> --execute
```

Before starting, check that the archive set is complete and openable - the startup `int3` I chased
for a day turned out to be one missing archive, and a locked one produces the same breakpoint:

```
victory check
```

## State of this branch

Verified on the machine it was built for: binds `88 hle, 427 host, 72 title, 0 missing`, runs the
title at a steady 30 fps, no crash over repeated runs, and the shader tests pass (227 in the shader
crate, 106 in HLE, 5 in the app).

Not verified here: anything about AMD hardware, anything about the 72 mesh programs over the output
component limit, and audio, which I can only describe from measurements rather than by listening.

## The flicker this fork had, and how it was measured

Worth writing down because the title was blamed for it first, and because the measurement method
matters more than the fix.

Symptom: the opening animation flickered, with a black frame between every visible frame. Back
buffer captures of consecutive frames were steady - 0.04 to 0.25 average difference between
neighbours - so the rendered content was never the problem, and every capture-based check said the
picture was fine.

What found it was capturing the **composed desktop**, not the swap chain: `ddagrab` (Desktop
Duplication) at 60 fps over the window region, six seconds, then the mean luminance of every frame.

```
two Present calls per frame   343 of 360 frames black, content one frame in six
one Present call per frame      0 of 360 frames black, steady content
```

The cause was a second `Present` in the frame path, left over from merging my sync-interval change
with upstream's error-reporting call. The second call flipped to the buffer that had not been drawn
into, so every other visible frame was an empty surface. Upstream has one call, and so did every
branch I opened there; the duplicate existed only here.

Two switches stay, for machines that still show something and need to try the other way without a
rebuild - neither is set by default:

```
VICTORY_SWAP_BUFFERS=3        three buffers instead of two
VICTORY_SWAP_EFFECT=discard   the bitblt model instead of flip
```

For anyone chasing something similar: a capture of the swap chain answers "what did the title
draw", and a capture of the composed desktop answers "what did the screen show". They disagreed
here by a wide margin, and only the second one reproduces what a person watching the monitor sees.

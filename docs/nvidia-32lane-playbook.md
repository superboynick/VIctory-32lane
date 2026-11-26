# Running the Series 1.0.2.1 build on a 32-lane NVIDIA laptop GPU

What was measured on one machine, what was disproved, and what is still wrong.
Every number below carries the round of `F:\gta6-victory\ops\LEDGER.md` or the log
file it came from. Where a thing was not measured it says `not measured`.

---

## 1. Scope

| | |
|---|---|
| GPU | RTX 5070 Ti Laptop, 12 GB, **32-lane waves** (LEDGER R93; `issue-draft.md` env table) |
| System RAM | 31.4 GB (LEDGER R93) |
| OS | Windows 11 (LEDGER R93) |
| Title | Xbox Series profile **1.0.2.1** (`issue-draft.md` env table) |
| Title data | 22 rpf, 1.45 GiB, `GTA6.exe` SHA256 `e95020b2…` (LEDGER R5) |
| Old line | our fork: upstream `main` (`ffe310e`, then `0b0074e`) + our patches (LEDGER R5, R65) |
| Working base | `trustno1-coder/VIctory-NVIDIA` @ `0ef2b99` + mouse-device patch `12b1d16` (LEDGER R93, R99) |
| Build used for play | `--profile diag --features diagnostics` — `VICTORY_SKIP_PIPELINES` exists only in the diagnostics build (`GTA6-进度报告.md`, section "how you play now") |
| Display | window 1920x1080, internal render scale 0.4, the title's own upscaler |

**"Playable" here means all five of these at once**, not a frame number:

1. the world is entered and the story advances past the cell (`F:\shots\scale04\frame-2355.png`);
2. the camera follows the mouse (ported GameInput device, `device=6 kind=32`, LEDGER R90);
3. the picture is judged correct by a human — no white haze, no black blobs, no orange
   fog (`F:\shots\trust2\frame-2400.png`, `F:\shots\room70\frame-1514.png`, LEDGER R93, R97);
4. no crash, no device loss, no frame-rate decay over 30 minutes (LEDGER R95);
5. about 5,000 draws per frame run at 7–10 fps (LEDGER R95, R97, R100).

Anything below 6 fps at that draw count, or any of the three picture faults, fails (3) or
(5) and is not playable regardless of what else is true.

---

## 2. Why the old line was abandoned

Two hypotheses were tested to destruction and both were wrong. The line was dropped on a
third finding: the fix we shipped for the second hypothesis was itself a wrong answer.

### 2.1 Hypothesis: the root signature is what the driver refuses

`VICTORY_PSO_LADDER=1` built six root-signature variants for the four compute pipelines
the driver kept rejecting: flag `DIRECTLY_INDEXED` removed, zero parameters, a root CBV
at `b0`, a root UAV at `u0`, and two 16-entry descriptor tables. **v1–v6 all returned
`0x80070057` (E_INVALIDARG)** (LEDGER R79). A seventh data point: our DXIL container wrote
a 20-byte all-zero HASH part (spec wants code digest + 4-byte flags); fixing it dropped
refused pipelines from 41 to 24 in a 60 s run, and the four targets stayed refused
(LEDGER R80). So neither the root signature nor the container hash was the cause. Refusal
counts move for reasons unrelated to the four programs of interest — a lesson that cost a
round.

### 2.2 Hypothesis: the wave size is what the driver refuses

`VICTORY_WAVE_FORCE32=1` (rewriting the emitted module) rescued nothing (LEDGER R79).
Wiring `VICTORY_PSV_WAVE32` into the real PSV construction point
(`shader.rs:1081`, `Psv { wave_size }` — not the value at `emit::mod.rs:705`) did:
all four compute pipelines built, `host pipeline refused` went 6 → 0 over 240 s, no
crash, no device removal, VRAM 10.29 / 12.23 GB (LEDGER R81). Runtime evidence that they
were created **and bound** followed: `2da4beb386509b77` (`alphaResolve:
CS_AlphTransmissionPrefilterMip`), `2cc4c197ee29e291` (`sr_tracing:
CS_Tiling_Generate_Opaque`), `02384a60fda6f2d0` created+bound, `6b82bc7d22016cb5`
created (LEDGER R89). Mechanically: we had written the container's declared
`WaveSize(min=max=64)` into PSV0, and a 32-lane driver rejects that; declaring nothing
lets it run 32 lanes (LEDGER R81).

### 2.3 The wave-size fix is a wrong answer, and that is the actual finding

A program that *creates* is not a program that *computes correctly*. Three independent
sources say so, and one of them is our own earlier documentation:

- Our own wave32 rescue note had already written it down: the rescue "create[s] PSOs but
  do[es] not dispatch the game shaders", and "GPU execution and visual correctness remain
  untested"; broadcast/vote scope changes from 64 lanes to each 32-lane half "can change
  results or loop termination" (`docs/wave32-rescue.md`).
- Community cross-statistics: 291 of 321 refused programs are wave64 because they *depend*
  on the lane layout; declaring 32 lanes does not make them compute the right answer, it
  makes them compute a wrong one (`GTA6-进度报告.md`, 23:45 entry, citing upstream issue #71).
- The picture: with the declaration forced, frames came out corrupted — overexposed white
  (`F:\shots\main93\frame-2232.png`), orange fog over the yard
  (`F:\shots\newmain_world\frame-2045.png`) — i.e. worse than the fault it was meant to
  remove (`GTA6-进度报告.md`, 23:45 entry).

We tried to verify this picture-side A/B **by frame number** and the method itself was
invalid: two runs at the same frame number land at different story positions because the
switch changes per-frame time. Measured brightness differences of 70.2 / 25.5 / <6.4 at
frames 1560 / 1680 / 1800+ were shown to be different scenes, not different rendering
(LEDGER R88). Picture verdicts need a fixed wall-clock moment and a stationary camera;
frame numbers are not comparable across configurations.

**Net position of the old line.** Genuine wins: `VICTORY_TEXTURE_SWEEP=0` gave world-band
p50 507.58 → 399.92 ms (−21.2%, p99 −76.9%, frames over 1 s 35 → 6, LEDGER R75); adding
`VICTORY_TEXTURE_NO_WAIT=1` reached p50 330.00 ms, p95 488 ms, long frames −55%, all.fps
11.93 → 15.23 (LEDGER R76); cumulative p50 507.6 → ~362 ms (~−29%) with the lookup cache
(LEDGER R83). But the same machine ran **~3 fps at ~5,000 draws** with correct-looking
geometry and wrong light (LEDGER R84), the picture was never confirmed correct, and the
remaining wall was structural: 813.5 ms per frame in the list-preparation step, ≈183 µs
per draw, against 124 ms/frame for the 7900 XTX approach (LEDGER R71). The line was
dropped in favour of a base that removes the passes instead of faking their lane width
(LEDGER R93).

---

## 3. What the working base does differently

### 3.1 It does not run the ray-tracing programs at all

NVIDIA refuses the 64-lane BVH/trace programs, so **every ray misses**, and those passes
turn a miss into light. A room therefore goes white depending on the view angle — the
symptom looks like a renderer bug and is actually a correct program fed an empty BVH.
The fork's answer is `VICTORY_SKIP_PIPELINES` loaded from `F:\victory-run\rt_off_hashes.txt`:
**1,034 program hashes**, about 1,000 `CS_RaytracingResolve` and 34 traced-shadow programs
(LEDGER R93; the file is one line, read by `set /p`, `play-trust.cmd:53`).

Deliberate exception: the screen probes and the visibility traces are **kept**. Skipping
those as well produces rainbow bright patches over the whole screen (`GTA6-进度报告.md`,
the fork's answer to the white haze). So the list is not "all ray-tracing programs"; it is
the resolve and traced-shadow set.

### 3.2 It bounds the fog instead of removing it

`VICTORY_AMBIENT_AIR=1.5` keeps fog inside 1.5x the light around it and
`VICTORY_AMBIENT_MOST=2` caps the most-lit contribution (`play-trust.cmd:34-35`,
LEDGER R93). With (3.1) alone the room is no longer white but the air can still smear;
with these two it is bounded. Both are required; neither alone is enough — the LEDGER
records them as "two conditions, neither one can be dropped" (LEDGER R93).

### 3.3 The rest of the preset is defensive, not performance

All four presets share: `VICTORY_NAN_SCRUB=1` (a shader's NaN becomes 0 before the blur and
the upscaler spread it — this is the "black blobs on faces and in the air" fix),
`VICTORY_OWN_UPSCALER=1` + `VICTORY_RENDER_SCALE` (the Series S profile),
`VICTORY_BVH_MISS=1`, `VICTORY_COMPOSE=1` (the interface as a second present plane, else
the screen is black until gameplay), `VICTORY_DYNAMIC_SLOTS=524288` and
`VICTORY_CARD_BUFFERS_MB=1536` (`play-trust.cmd:31-50`).

`VICTORY_DYNAMIC_SLOTS` deserves a line of its own: the NVIDIA view heap caps at
1,000,000 and views past it read a null slot; one run asked for 1,245,184, i.e. **245,184
views past the cap**, which is the documented reason for the switch
(`issue-draft.md` item 4; `README.md:106-112`). Without it, thousands of draws a frame are
dropped.

---

## 4. Measured settings that matter

Every row changed one thing on an otherwise identical configuration. "n" is the run
length per arm; where a run length was not recorded it says so.

| Setting | Before | After | Sample | Source |
|---|---|---|---|---|
| `TEXTURE_ROOM_PERCENT` 90 → 80 → 70 | 8.0 fps, 28.0 GB private, 25.1 GB WS | 9.0 fps / 26.0 GB; **70: 9.8 fps, 25.1 GB private, 22.7 GB WS** | 160 s per arm, scale 0.4 | LEDGER R97 |
| `RENDER_SCALE` 0.5 → 0.4 → 0.3333 | 6.6 fps (4,895 draws) | 7.8 fps (5,251); **9.7 fps (5,014)** | heavy scenes; run length not recorded | LEDGER R96 |
| `WINDOWS_BAR_MB` 0 → 1024 | 8.8 fps, 25.5 GB private | **9.2 fps, 24.9 GB private** | 170 s per arm, draws > 3,000 | LEDGER R98 |
| `TEXTURE_NO_WAIT` 1 vs 0 | 7.6 fps, **88** "changed after its upload", 10 not-mirrored | 6.2 fps, **39**, 13 | 160 s per arm | LEDGER R100 |
| `TEXTURE_SWEEP=0` (old line) | p50 507.58 ms, p99 3,705 ms, 35 frames > 1 s | p50 399.92 (−21.2%), p99 855 (−76.9%), 6 | 150 s per arm, base80 | LEDGER R75 |
| `TEXTURE_SWEEP=0` + `NO_WAIT=1` (old line) | p50 507.58, p95 1,104, all.fps 11.93 | **p50 330.00, p95 488, all.fps 15.23** | n = 188 world-band frames | LEDGER R76 |

### 4.1 The switches that did not work (same rule: one change, measured)

| Attempt | Result | Verdict | Source |
|---|---|---|---|
| `FAST_CORES=3` (pin translation threads to P-cores) | 8.2 fps vs 8.8 baseline, 25.7 GB | worse | LEDGER R98 |
| `RELAY=1` (drop only relay's second stage) | 6.2 fps vs 8.8 = **−30%**, 25.4 GB | quality-only arm | LEDGER R94, R98 |
| `JOIN_PARTS=16` | 6.8 fps vs 6.6 | inside noise | LEDGER R96 |
| `INDIRECT_BATCH=4` | 6.6 fps vs 6.6 | no change | LEDGER R96 |
| `UPLOAD_AHEAD=0` | 6.1 fps | worse | LEDGER R96 |
| `WINDOW_KB=1024` + `WINDOW_TAILS=1` | 6.1 fps, 23.5 GB WS, 27.1 GB private | no gain on the number it was sold for | LEDGER R94 |
| those two + `SNAPSHOT_POOL=8` + `LATE_WAIT_LISTS=64` | 5.1 fps, 24.7 GB WS, 28.0 GB private | worse | LEDGER R94 |
| `SCRATCH_RECLAIM=1` | 330 reclaims in 30 min, ≤188 MiB each time | kept, but it is not the lever | LEDGER R95 |
| `TEXTURE_COMPARE=1` (old line) | uploads 14–57 MB → 1–9 MB, but texture step 120 → ~2,000 ms | skipping works, comparison costs more than it saves | LEDGER R77 |
| `BATCH_GPUWRITE=1` (old line) | 33/159/267 ms vs 81/371/310 | no gain | LEDGER R78 |

Two methodology notes that govern the whole table. First, a single number is not a
measurement: the same build loads different parts of the city per run, with draws
swinging 1,889–4,769 and p95 swinging 1,100–5,700 ms, so only **same-draw-band** samples
may be compared (LEDGER R72, R85; `F:\bench\band.ps1`). Second, the shipped presets log at
`VICTORY_LOG=warn`, which suppresses the frame-time breakdown, the memory lines and the
counters — all `info` level. Any A/B must temporarily set `VICTORY_LOG=victory_gpu=info`
or the only number available is the fps in the window title (LEDGER R98).

---

## 5. Memory

### 5.1 Where the peak goes

Peak private commit **28,344 MB** (`soak`, 16:58:54 map line; figures quoted at LEDGER R97
and analysed in `内存峰值分析.md` §三):

| Component | MB | Share | Whose |
|---|---|---|---|
| Write-combined system memory | 12,146 | 43% | the driver's spill of resources that did not fit on the card; 3,393 MB of it is our upload buffers, 1,369 MB our card-buffer copies, ~7 GB has no owning code line (`内存峰值分析.md` §三) |
| Title's own heap | 4,779 | 17% | not ours, flat from load to world; cannot be moved |
| Process heap segments | 4,234 | 15% | 1,592 MB our Rust allocations, 2,070 host lists, driving 5,112 MB of growth |
| Other private ≥ 1 MB | 3,910 | 14% | `shared_buffer` (CUSTOM + WRITE_BACK) |
| 4 MiB windows | 2,788 | 10% | `shadow.rs:5034` window grid (mem-line basis 4,100 MB) |
| Other private < 1 MB | 235 | 1% | small blocks; the allocator's 1.78 M blocks are spread over the rows above |

### 5.2 It is a plateau, not a leak

Over 30 minutes, advancing the story every 25 s (LEDGER R95):

| t | fps | draws | working set |
|---|---|---|---|
| 20 s (menu) | 29 | 6 | 7.0 GB |
| 200 s | 7 | 4,900 | 20.3 GB |
| 560 s | 7 | 4,576 | 20.5 GB |
| 920 s | 7 | 4,937 | 20.4 GB |
| 1,280 s | 7 | 4,650 | 20.5 GB |
| 1,640 s | 7 | 4,836 | 20.2 GB |

Zero errors, zero device removals, no growth. One correction to the folklore: private
commit does **not** come back down when the world is entered — 28,344 MB at peak →
28,086 MB after 27 minutes, a 258 MB difference. What falls is the *working set*
(22,904 → 20,120 MB), because host texture copies are dropped (6,605 → 3,727 MB)
(`内存峰值分析.md` §四). So the machine needs 28 GB of commit for this build, and a 32 GB
box must close the browser first.

### 5.3 What actually reduces it

| Lever | Effect | Evidence |
|---|---|---|
| `TEXTURE_ROOM_PERCENT=70` (from 90) | **−2.9 GB private** (28.0 → 25.1), −1.7 GB WS, **+1.8 fps**, no picture loss | LEDGER R97 |
| `WINDOWS_BAR_MB=1024` | −0.6 GB private, +0.4 fps | LEDGER R98 |
| `SCRATCH_RECLAIM=1` | ≤188 MiB in total, 330 triggers / 30 min | LEDGER R95 |
| everything else in the memory survey's list | ≤188 MiB total | LEDGER R94 |

The mechanism behind row 1 is the 43% block: write-combined memory tracks how much we ask
the card to hold (WC 8,329 / 10,023 / 11,211 MB at card occupancy 64% / 87% / 96%), so
`room` is the only switch that moves the largest component (`内存峰值分析.md` §三, §五).

There is a hard ceiling on that direction too. On the old line, `room 90` with
`staging 768 MB` peaked at **11.86 GB of 12.23 GB** and lost the driver; 85/640 peaked at
10.9 GB and was safe (LEDGER R92). On the working base, 90 is safe in VRAM but costs
2.9 GB of RAM and 1.8 fps against 70 (LEDGER R97).

---

## 6. Stability

| Run | Length | Result | Source |
|---|---|---|---|
| Fork, first contact | 260 s | no crash, 0 errors, 6–8 fps at ~5,000 draws | LEDGER R93 |
| With mouse device | 240 s | 0 errors, 0 pipeline failures, fps unchanged vs without the device | LEDGER R91 |
| Speed set + mouse + `SCRATCH_RECLAIM` | **30 min** | survived, 0 errors, 0 device removed, WS flat, fps flat at 7 | LEDGER R95 |
| Old line, final config | 10 min | 0 errors, 0 pipeline failures, VRAM 10.3 / 12.2 GB, no decay | LEDGER R87 |
| Old line, 300 s | 300 s | survived, VRAM peak 10.3 / 12.2 GB, 3 fps at 4,940 draws | LEDGER R84 |

The 16-minute device-loss class that the 7900 XTX fork needed a separate copy queue to fix
does not appear here (LEDGER R87).

**The watchdog hook.** The title kills itself when the runtime's own code does not match
what it expects — the visible symptom is "quits after a cutscene". The fix is a hook that
drops the mismatched check and logs a byte-mismatch warning instead of exiting. Our old
line took it from upstream (`Marvisak/VIctory#454`; `VICTORY_NO_WATCHDOG=1` in the old
`play-fast.cmd`), and the working base **already has it, with the byte-mismatch warning**,
so nothing was re-implemented (LEDGER R94; `GTA6-进度报告.md`, stability section).

**What still crashes:** nothing was observed inside the soak windows above. The two known
crash mechanisms are configuration, not code: `room 90` + `staging 768 MB` on 12 GB
(driver loss, LEDGER R92), and a machine with no pagefile hitting the commit limit
(23.8 GB on the 3080 Ti test box, `os error 1455` after ~5 s, LEDGER R69/R52) — not
applicable to this 31.4 GB machine as configured.

---

## 7. What is still wrong

The picture is correct and the frame rate is usable; the remaining defects are all
"occasionally one texture is wrong". All counts below are from one 155-line `warn` run
(`r70.err`, 134.6 s of log window, frames 1–2048), i.e. the recommended preset
(`issue-draft.md`).

### 7.1 Rainbow / pixelated skin — 61 to 88 findings per run, one cause of two found

`victory_gpu::draw` at `draw.rs:2323-2333`:
*a texture picked by index changed after its upload with nothing to tell of it* — the
recheck path (`RECHECK_FRAMES` = +4, +12, +40, +120, +400, `draw.rs:624`) hashes 48 KiB of
guest memory and finds it different from the fingerprint kept at upload, then re-uploads.

- **61** lines in 134.6 s, in 10 bursts over 6 slots, `bytes` 176,128–8,294,400,
  `frames_after_upload` up to **576** (a minute at 10 fps), `blocks_shown_elsewhere` never
  0 (3–127 — every one of these textures is aliased, and the alias is where the writes
  land). The count 61 is exact, not deduplicated (`draw.rs:2315`) — `issue-draft.md` item 1.
- A/B on the switch that governs it: `TEXTURE_NO_WAIT=1` → **88** lines, 7.6 fps;
  `=0` → **39** lines, 6.2 fps (LEDGER R100, 160 s per arm). So waiting for the remap
  removes **56%** of them and costs ~18% of the frame rate.
- **A second cause remains.** 39 lines survive the wait. The documented mechanism on our
  side is that a staging buffer that holds nothing yet is filled with whatever pages are
  there and uploaded anyway, with its own comment recording 137 zeroed levels in one
  frame's 787 repeated uploads (`texture.rs:396-404`, 825-831, `issue-draft.md` §1).
- Status: `play-trust-clean.cmd` is the arm that trades 18% fps for this; the residual 39
  are unexplained and are the reason the upstream report exists (PR #9 comment).

### 7.2 12 placed unordered texture views resolve to null

`victory_gpu::descriptor::host`, `host.rs:4200-4226`: the fast path mirrors placed
*buffers* only, so a placed *texture* with a UAV gets no host descriptor and the
shader-visible slot stays null. Every write through it is discarded; every read returns
zeros. In one run: **12 distinct** placed 2D textures (256, 256, 512×4, 1024×3, 2048×2 in
`resource[4]`; formats 71/78/80/83/98/99; `view[5]=15`), with `times` running to **2048** —
i.e. at least 2,048 binds served by a null slot. This is the second-ranked candidate for
the rainbow skin because a 1024²/2048² compute-written texture whose writes are dropped,
then sampled as colour, is arbitrary pixels by construction (`issue-draft.md` item 4).
What it does *not* tell us is what those 12 textures are for; the log carries descriptor
words and sizes, not the first mip's dimensions.

### 7.3 29 unimplemented-pipeline-method warnings — two slots, ≥16,384 calls

`victory_gpu::device`, `device.rs:391-399`, returns `E_NOTIMPL`. The **29 lines are not 29
methods**: they are two slots, `slot=0x218` (15 lines) and `slot=0x1c8` (14), and `calls`
doubles 1, 2, 4, … 16,384 — the line is emitted at doubling counts, so the method was
called **at least 16,384 times**, in two kinds (`issue-draft.md` item 3). The title runs
and draws, so whatever these two creations are for is optional or supplied another way;
but if one of them is a pipeline the title wants per material, that material is drawn by a
fallback. Identifying `0x218` and `0x1c8` on the console device is the open question.

### 7.4 Smaller, quantified, and not the skin

| Finding | Meaning | Count |
|---|---|---|
| 9 compute pipelines refused (`0x80070057`), e.g. `2da4beb386509b77` | a dispatch whose host pipeline was never made is dropped; **≥10 compute passes wrote nothing** in that run | 9 hashes; 1 `once!` at `draw.rs:2657` |
| 3 null buffer/CBV refusals | a null CBV reads as zero constants — a classic collapsed/stretched skinned mesh | 3 `once!` lines |
| `texback` layout `Unsupported` | the read-back is abandoned, so compute-written textures (height maps, displacement) stay stale | 1 `once!`, frequency unknown |
| 4 console CPU instructions emulated | `ClzeroLoop`, `Monitorx`, `Mwaitx`, `Insertq` | 1 line, interpreted |
| 10 black frames + 8 `levels` readings | frames 1–112 only, whole-frame zeros, `copied=true`, then it stops | load transition, **not** a defect |

### 7.5 What was ruled out

- **Root signature** — six variants, all `E_INVALIDARG` (LEDGER R79).
- **Container HASH part** — was all zeros, fixed, targets unchanged (LEDGER R80).
- **Wave-size declaration as a picture fix** — creates and binds, but corrupts output
  (LEDGER R81 vs `GTA6-进度报告.md` 23:45 entry; `docs/wave32-rescue.md`; LEDGER R93).
- **Resolution** — 720p / 900p / 1080p / 4K all 395–416 ms on the same draw band; the cost
  is ~70 µs per draw, not per pixel (LEDGER R86). The swap chain is always 3840×2160
  regardless of the reported mode (LEDGER R86).
- **Texture sweep / compare / batch guards / GpuWrite batching** — all measured, none
  worth keeping (LEDGER R75, R77, R78, R72).
- **Black frames as a runtime defect** — whole-frame, confined, self-stopping
  (`issue-draft.md` item 2).
- **Stretched bodies** — no log line speaks to it; the fork's own table attributes it to
  relay stage two and card steps, i.e. to `RELAY=2` + `CARD_STEPS=2`, which is exactly what
  the speed set uses. Test with the quality arm, do not attribute it to a warning
  (`issue-draft.md` §"About the stretched bodies").
- **The 1,000,000-view heap cap** — known, documented, worked around with
  `DYNAMIC_SLOTS` (`issue-draft.md` item 4).

---

## 8. How to reproduce

### 8.1 Play — the four presets

All four run the same binary from `F:\victory-run` (the working directory matters: the
runtime looks for `victory.toml` in the cwd, not upward — LEDGER R6):

```
F:\victory-run\victory-trust-diag.exe run F:\victory-data\game\GTA6.exe --execute
```

| Preset | Distinctive settings | Measured |
|---|---|---|
| `play-trust.cmd` | `room 70`, `scale 0.4`, `WINDOWS_BAR_MB=1024`, `NO_WAIT=1`, `RELAY=2`, `CARD_STEPS=2` | 9.2–9.8 fps, 24.9–25.1 GB private (LEDGER R97, R98) |
| `play-trust-fast.cmd` | as above but `scale 0.3333` | `scale 0.3333` alone measured 9.7 fps (LEDGER R96); the combination with `room 70` is **not measured** |
| `play-trust-quality.cmd` | `room 90`, `scale 0.5`, `NO_WAIT=0`, `RELAY=1`, `CARD_STEPS=0` | 5–6 fps, 23.7 GB private (LEDGER R94) |
| `play-trust-clean.cmd` | `room 70`, `scale 0.4`, `NO_WAIT=0` | 6.2 fps, 39 warnings instead of 88 (LEDGER R100) |

`F:\victory-run\victory_sm.exe` is a copy of the same binary for NVIDIA App's Smooth
Motion (frame generation); it doubles the *perceived* rate, not the real one.

### 8.2 Build it yourself

```
git clone https://github.com/trustno1-coder/VIctory-NVIDIA && cd VIctory-NVIDIA
git checkout 0ef2b99
cargo build --profile diag --features diagnostics
```

The diagnostics build is required for `VICTORY_SKIP_PIPELINES`; without it the ray-tracing
passes run and the room goes white (`GTA6-进度报告.md`, section "how you play now"). The delivered binary
is `F:\build\target-trust\diag\victory.exe`, copied to
`F:\victory-run\victory-trust-diag.exe` (95,494,633 / 95,538,154 bytes respectively).

### 8.3 Measure

```
F:\bench\run-bench.ps1 -Exe F:\victory-run\victory-trust-diag.exe -Tag base -Seconds 170
F:\bench\band.ps1 -Tag @("base","candidate") -DrawLow 3000 -DrawHigh 6000
```

`band.ps1` is the only trustworthy comparison: it keeps only the 5-second window lines
whose draw count falls inside the band, which is what stops two runs that loaded different
amounts of the world from being compared (LEDGER R85). Anything that is not same-band is
scene drift.

Log level: keep `VICTORY_LOG=warn` for playing; set `VICTORY_LOG=victory_gpu=info` for any
measurement, or the breakdown, memory and counter lines are all gone (LEDGER R98).

### 8.4 Capture frames

Diagnostics build, `VICTORY_CAPTURE=<folder>` plus `VICTORY_CAPTURE_FRAMES=<frames>`
(capture happens at the named frame numbers; the internal sampler uses powers of two —
`docs/performance-record-20261126.md`, methods section). `F:\bench\shot.ps1 -Tag <name>
-Frames 600,900,1200,1500,1800` does this for the **old** build
(`F:\build\target\release\victory.exe`) with the old settings hard-coded
(`shot.ps1:24-43`), so for the trust build set those variables by hand.

To compare a picture fix, use a fixed **wall-clock** moment with the character standing
still, never a frame number: two configurations at the same frame number are at different
story positions, which is the error that invalidated our first wave32 picture comparison
(LEDGER R88). `VICTORY_CAPTURE_AT=<seconds>` is the tool for that; whether the trust build
implements it is **not measured**.

### 8.5 Evidence on disk

| Path | Shows |
|---|---|
| `F:\shots\trust2\frame-2400.png` | the cell, correct: walls, window grid, yard, characters, sunset (LEDGER R93) |
| `F:\shots\room70\frame-1514.png` | the yard at room 70: no missing textures, no fog (LEDGER R97) |
| `F:\shots\scale04\frame-2355.png` | render scale 0.4: soft but clean, story past the cell (LEDGER R96) |
| `F:\shots\main93\frame-2232.png` | old line: overexposed white, figures as silhouettes (`GTA6-进度报告.md`) |
| `F:\shots\newmain_world\frame-2045.png` | old line: orange fog over the yard (`GTA6-进度报告.md`) |

---

## 9. Open questions for upstream

1. What are `slot=0x218` and `slot=0x1c8` on the console device? Two pipeline-creation
   kinds are called ≥16,384 times and return `E_NOTIMPL` (`issue-draft.md` item 3).
2. Which 12 placed textures take a UAV that resolves to null, and what fills them? A
   one-field patch to the `draw.rs:2323` warning (first mip width/height/format) would
   answer it without a capture (`issue-draft.md` item 4 and "What we can add" 6).
3. Is the residual 39 "changed after its upload" findings the fill-with-what-is-there path?
   `NO_WAIT=1 + TEXTURE_FILL=0` is the arm that separates it from the deferred read
   (`issue-draft.md` "What we can add" 1).
4. Is the mouse device (PR #9, +172/−22) the shape you want, or should input be a GameInput
   kind the title polls differently? The scroll wheel is not implemented — `GetAsyncKeyState`
   cannot read rotation (LEDGER R90, R99).
5. The 72 mesh programs whose output exceeds 32 KiB by 128–896 components (1.6–11% over)
   are ped-skinning variants whose vertex form passes; primitive components are all 0, so
   the budget is spent on vertex outputs (LEDGER R62, R63, R66). Does that overlap the
   stretched bodies you see?
6. Cheap path past ~70 µs per draw of preparation: our number is 813 ms per frame in the
   list-preparation step at 4,451 draws (LEDGER R71); the 7900 XTX approach is 124 ms via
   instruction-level self-reported descriptor slots (LEDGER R70). Is that portable to a
   fork that has to keep the NVIDIA fixes?
7. Should PSV0 default to `wave_size = None` for console-width programs instead of echoing
   the container's 64? It makes them create (LEDGER R81) but corrupts output when the
   program depends on lane layout (LEDGER R93), so the question is whether any of them are
   lane-layout-independent.
8. Below `room 70` on 12 GB: 70 is the lowest we tested (LEDGER R97); 50 is permitted by
   the code (`内存峰值分析.md` §五). Is there a documented floor where textures stop
   arriving at all?

# Where the frame time goes on a 12 GB card

Measured on an RTX 5070 Ti Laptop (12,227 MiB, driver 32.0.16.1714), 31.4 GB of RAM,
Series 1.0.2.1, `--execute`, prologue into the world. Every number here comes from
`VICTORY_FRAME_LOG=1` and `VICTORY_PRESENT_TIMES`, and the benchmark that produced the
before/after pair is described at the end so it can be repeated.

## The wall is not the CPU and not the GPU

While the world ran at 1 to 2 fps, with 4,000 to 5,000 draws a frame:

```
CPU: 521 s of processor time over 300 s of wall clock  = 1.74 of 24 cores  =  7%
GPU: 7 to 13% utilisation
threads: 114 to 122, affinity mask all 24 logical cores
```

Both processors were idle while frames took 400 to 1,300 ms. The time sat in one thread
recording the title's lists (about 31,000 record calls a frame, 650 ms), and behind that
in preparing textures and in memory pressure.

## Memory was the real limit

By the time the world is loaded:

```
private memory     22,695 MB, of which 16,652 MB resident
RAM free            2,247 MB
VRAM in use        10.6 GB of 12.2 GB
log: "a texture already on the card is brought up to date although the card has no room
      for new ones"      and      "host memory pressure: limiting texture prefetch"
```

Once the card is full the system starts moving card memory into system memory, and each
move costs hundreds of milliseconds. That is what the 3,000 to 6,000 ms frames are.

## What actually moved the number

`VICTORY_TEXTURE_ROOM_PERCENT` says what share of the card's budget host copies of textures
may take; it defaults to 90. At 70, on this machine, over two 150-second runs:

| metric | 90 (default) | 70 | change |
|---|---:|---:|---|
| world p50 | 1,057.6 ms | 477.1 ms | −55% |
| world p95 | 3,340.8 ms | 759.7 ms | −77% |
| world p99 | 4,438.1 ms | 1,134.5 ms | −74% |
| frames over 1 s | 35 | 9 | −74% |
| long frames | 68 | 52 | −24% |

This costs texture residency: fewer textures stay on the card, so more of them are made
again later. On a 12 GB card that is the right trade; on 16 GB it may not be.

## Other knobs that touch the same pressure

Found by reading the code, not yet measured here - listed so the next person does not have
to grep for them:

```
VICTORY_TEXTURE_BUDGET_MB      absolute cap on host texture copies, instead of a percentage
VICTORY_REUPLOAD_WHEN_FULL     (default 1) re-uploads a texture when the card has no room;
                               0 stops that; the log says it is happening 14,500 times a run
VICTORY_TEXTURE_STAGE_MS       (default 250) time a frame may spend staging textures
VICTORY_TEXTURE_STAGE_MB       (default 192) bytes a frame may stage
VICTORY_TEXTURE_CATCHUP        (default 1) extra budget while a backlog drains
VICTORY_TEXTURE_SWEEP          (default 1) lets go of host copies nothing holds
VICTORY_LEVELS_ON_DEMAND       (default 0) card copies take levels level by level
VICTORY_STAGING_POOL_MB        (default 256) spare staging buffers kept
VICTORY_LAZY_TEXTURES          (default 1) make a host copy when a shader first reads it
```

## The per-draw cost, for whoever attacks that next

From the profiling scopes (`VICTORY_STATE_PROFILE=1`, `VICTORY_RECORD_PROFILE=1`):

```
with_state: get list 27 ns, lock 33 ns, execute f 99.9% of the call
execute_indirect, 496,096 calls, 73,400 ns each:
    descriptor table walk    17,253 ns  (23.5%)   draw.rs:2911 tables()
    resource refresh         15,506 ns  (21.1%)   draw.rs:3746 readable(), draw.rs:1013 memory_ready()
    unmeasured               30,515 ns  (41.6%)   gather_ends, pipeline lookup, window/bound
    bind 2,843 | state hash 765 | write list 807
dispatch, 289,758 calls, 94,392 ns each: resource refresh 30,046 (31.8%), unmeasured 56,221 (59.6%)
```

Three candidates were implemented and all three failed to clear a 20% bar, which is why
none of them is on by default:

- grouping descriptor tables statically: 71,465 to 76,058 ns per call, worse
- caching `tables()` behind a dirty version: world p50 553 ms against 487 ms, worse
- merging copy-back runs: the premise did not hold, an active frame has 757 runs and
  7.26 ms of writes, not 65,000 runs

## How to repeat the comparison

```
F:\bench\run-bench.ps1 -Exe <victory.exe> -Tag a -Seconds 150
F:\bench\run-bench.ps1 -Exe <victory.exe> -Tag b -Seconds 150 -Env @{VICTORY_TEXTURE_ROOM_PERCENT='70'}
F:\bench\compare.ps1 -A a -B b
```

The world starts around 60 to 72 s in, so only the last 80 s of a 150 s run is workspace.
Three repeats of the same build gave a coefficient of variation of 2% on `steady.p50`,
which is the only metric worth reading: the all-run `p50` is pinned at 33.3 ms by the
title's own 30 fps pacing and shows nothing.

## What actually moved it, in the order it was found

Three settings, each measured against the one before over a 150-second run, world segment only:

| configuration | world p50 | p95 | frames over 1 s | all-run fps |
|---|---:|---:|---:|---:|
| room 80, 4K mode, diagnostics build | 507.6 ms | 1,104 ms | 35 | 11.9 |
| room 80, 1080p, plain build, sweep on | 399.9 ms | 748 ms | 6 | 12.4 |
| the same, `VICTORY_TEXTURE_SWEEP=0` | 399.9 ms | 748 ms | 6 | 12.4 |
| plus `VICTORY_TEXTURE_NO_WAIT=1` | **330.0 ms** | **488 ms** | **4** | **15.2** |

Two of these were not performance settings at all as far as anyone knew:

- The **diagnostics build** was used for every earlier measurement. Upstream reports it turns the
  world into 9 to 12 seconds a frame because it runs a `VirtualQuery` per draw. Our numbers were
  inflated by it.
- The title was told it had a **4K display** while the window is 1080p, so it rendered four times
  the pixels shown.

The one that mattered most is a lock wait, not bandwidth. Texture refresh called
`waited(2, crate::watch::title_memory_reading)` - it queued behind the title's own mapping changes.
The step timings said so plainly once they existed:

```
before   texture_ms = 1100 to 4742 per five-second window
after    texture_ms =  119 to  142
```

`VICTORY_TEXTURE_SWEEP=0` is the other half: the sweep released 234 to 1177 host copies of
title-filled textures every second - 58 to 370 MB - and each was uploaded again later. Keeping them
cost 10.0 GB of a 12.2 GB card after four minutes, with no out-of-memory and no device loss.

## What was tried and did not work

- `VICTORY_TEXTURE_COMPARE=1` skips unchanged uploads (14 to 57 MB becomes 1 to 9 MB) but the
  comparison itself costs more than the uploads it saves, 119 ms becoming 2,000 ms, because it
  compares title memory against a detiled layout rather than against a snapshot of the same layout.
- `VICTORY_PREFETCH_TEXTURE=1`, `VICTORY_BATCH_GUARDS=1`, a static descriptor grouping, a dirty
  `tables()` cache, the cool_fox switch set, and `VICTORY_STEP_INDEX=1` all failed to clear the
  noise floor, and several made p95 worse.
- `VICTORY_WAVE_FORCE32=1` does **not** revive the four compute pipelines the driver refuses
  (`0x80070057`), so that refusal is not about lane count.

## Where the remaining time goes

With the settings above, one frame of about 300 ms:

```
texture steps      119 to 142 ms   (was the whole wall)
gpu writes         113 to 179 ms   about 60,000 steps per five seconds
fills              114 to 176 ms   about 120,000 per five seconds
title copies        59 to 81 ms   160 to 212 MB
copy-back          117 to 250 ms   waiting for the card's comparison
on the GPU          61 to 181 ms
```

The next honest target is the per-step cost of `gpuwrite` and `fill`, and the copy-back wait, not
the textures.

## Second round: the picture fix, and the tail

The four compute pipelines the driver refused turned out not to be a root signature problem - six
signature variants were refused with the same code - nor a lane-count problem. The pipeline state
validation part carried the container's declared wave size through, so a console shader that
declares 64 asked a 32-lane device for 64-lane waves. Leaving the declaration out
(`VICTORY_PSV_WAVE32=1`) creates all four, including the alpha transmission prefilter whose absence
hazes the screen, and pipeline creation failures in a 240-second run went from 6 to 0.

Frame rate did not move: at 5,000 draws a frame it stays at about 3 fps. It is a picture fix.

Two settings then moved the tail rather than the median, over three 150-second runs each
(baseline p50 365.9 ms, p95 602 ms, 55 long frames):

| added | p50 | p95 | p99 | long frames |
|---|---:|---:|---:|---:|
| `VICTORY_COPY_PLAIN=1` | 374.2 | **482.6** | **593.9** | **40** |
| plus `VICTORY_WIDE_BACK=1` | 362.4 | 537.7 | 684.3 | 44 |

`COPY_PLAIN` replaces the kernel's copy of title memory (`ReadProcessMemory`, about 3 GB/s) with a
guard-page-protected `rep movsb`, which is what upstream's own file says is three times faster. The
median is unmoved because the median frame is not the one doing large copies; the long frames are.

Three further switches - `VIEW_INLINE`, `ROOT_SRV_DIRECT`, `LOOKUP_CACHE` - were tried and none
cleared the noise floor on p50 (370, 372, 378 ms against 362), though `LOOKUP_CACHE` did cut long
frames from 44 to 36.

Running total for the world segment: **507.6 ms to about 362 ms, about 29 per cent**, with long
frames from 101 to about 40.

## Comparing two runs that loaded different scenes

The three-run repeats gave a coefficient of variation of two per cent on the median, which is what
the benchmark claims, but comparing two *different* builds turned out to be noisier: one run reached
the world at 60 s with 1,000 draws a frame, another at 105 s with 5,300. The median of the whole
world segment then says more about which part of the city each run happened to load than about the
change being measured.

The fix is to only compare five-second window lines whose draw count falls in the same band:

```
F:\bench\band.ps1 -Tag @("before","after") -DrawLow 3000 -DrawHigh 6000
```

With that, the four settings of this whole investigation, at 3,000 to 6,000 draws a frame:

| configuration | p50 in band | samples |
|---|---:|---:|
| start (4K mode, diagnostics build, sweep on) | 571 ms | 3 |
| plus the two texture settings | 369 ms | 6 |
| the same on the current build | 394 ms | 6 |
| plus the wave32 pipeline fix | 413 ms | 11 |

So the frame time fell about **28 per cent** at equal draw counts, and the picture fix - the four
compute pipelines now being created - costs about **five per cent**. That is the trade worth knowing:
the haze is gone for roughly one frame in twenty.

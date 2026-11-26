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

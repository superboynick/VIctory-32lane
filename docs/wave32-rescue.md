# Conservative late wave32 rescue

Base: `8b7c3e7` (upstream `0b0074e`). Only the final, optimised module is
classified. The returned wave size, DXIL entry-point metadata and PSV0 limits
are changed together. A change prints the original container's BLAKE3 prefix:
`wave: <hash> 64->32 reason=<no-wave-intrinsic|safe-intrinsics>`.

With no live wave/quad calls, wave64 becomes wave32 by default. With exactly
`VICTORY_WAVE_FORCE32=1`, the allowlist also permits `waveReadLaneFirst`,
`waveAnyTrue`, `waveAllTrue` and `waveActiveAllEqual`. Lane indices, lane counts,
ballots, indexed lane reads, reductions, prefixes, quad and unknown wave calls
are excluded. Any live barrier (even a fence without sync) or indirect exit
blocks both paths. The switch and source changes automatically join the
existing translation-cache fingerprint.

## Five captured shaders

Checked with `VICTORY_HOST_WAVE=32`, using the original containers extracted
from `F:/victory-data/game/shaders.rpf`. The baseline refused all five PSOs.
Both default and opt-in runs now create exactly one PSO:

| Hash | Default / FORCE32=1 | Remaining reason |
|---|---|---|
| `369cbe906ea72764` | 32 / 32, PSO created | No wave calls or barriers; group remains 64,1,1 and shared memory 512 bytes |
| `2da4beb386509b77` | 64 / 64 | Barrier, lane index, indexed lane read |
| `2cc4c197ee29e291` | 64 / 64 | Lane index and ballot |
| `02384a60fda6f2d0` | 64 / 64 | Barriers, lane index, ballot and indexed lane read |
| `6b82bc7d22016cb5` | 64 / 64 | Barriers, lane index and ballot |

PSV0 min/max wave limits and emitted function names were also inspected.
The opt-in path is covered by synthetic module tests; none of these five
containers qualifies for an additional opt-in rescue.

## Reproduce in PowerShell

Delivered `target/release/victory.exe` SHA256:
`960DF0EEBD2F5AACA4DDC1B2D00F51CA56E714FAB02D9E88D3D69A6C067BA4BE`.

The installed GNU toolchain uses the existing local dlltool wrapper below.
No SDK validator was available; PSO creation was checked on the host GPU.

```powershell
$env:PATH = 'C:/Users/admin/fork-new/target/gnu-tools;' + $env:PATH
rustup run 1.98.1-x86_64-pc-windows-gnu cargo test -p victory-shader --release wave32_tests
rustup run 1.98.1-x86_64-pc-windows-gnu cargo test -p victory-shader --release emit::bitcode::
rustup run 1.98.1-x86_64-pc-windows-gnu cargo test -p victory-shader --release emit::lower::
rustup run 1.98.1-x86_64-pc-windows-gnu cargo build --release -p victory
$env:PATH = 'C:/Users/admin/.rustup/toolchains/1.98.1-x86_64-pc-windows-gnu/bin;' + $env:PATH
$env:VICTORY_HOST_WAVE = '32'
$env:VICTORY_TRANSLATE_ONLY = '369cbe906ea72764,2da4beb386509b77,2cc4c197ee29e291,02384a60fda6f2d0,6b82bc7d22016cb5'
Remove-Item Env:VICTORY_WAVE_FORCE32 -ErrorAction SilentlyContinue
./target/release/victory.exe translate F:/victory-data/game/shaders.rpf --pipelines --list
$env:VICTORY_WAVE_FORCE32 = '1'
./target/release/victory.exe translate F:/victory-data/game/shaders.rpf --pipelines --list
Get-FileHash target/release/victory.exe -Algorithm SHA256
```

Six new tests passed, including a recorded failing-before/passing-after test
for stale wave64 metadata. Existing bitcode tests: 16 passed, 1 ignored;
lowering tests: 13 passed, 1 ignored. Build has one existing unused-function
warning in `victory-gpu::resource::placed_filler_twins`.
The commit helper's repository-wide `cargo fmt` check encounters existing
formatting differences, including untouched `apps/victory/src/main.rs`.
New/changed lines were checked separately with rustfmt; unrelated source was
not reformatted.

## Limits and risk

These checks create PSOs but do not dispatch the game shaders. No barriers
or cross-wave exchange are added, and shaders containing barriers remain
excluded from this rescue. This avoids introducing a new barrier wait; it
does not establish absence of races in existing LDS accesses or guarantee
freedom from deadlock/TDR in actual execution. Broadcast/vote scope changes
from 64 lanes to each 32-lane half and can change results or loop termination,
which is why it is opt-in. GPU execution and visual correctness remain untested.

The delivered build is ordinary GNU release. An additional diagnostics-feature
build compiled but exited with `0xc0000005` even on `--version`; that build is
not the delivered artifact and its startup fault has not been attributed.

## Commit delivery

The original worktree's Git metadata is outside the writable sandbox at
`C:/Users/admin/fork-new/.git/worktrees/wt-trace`; staging there fails with
`index.lock: Permission denied`. A separate checkout under
`target/wave-commit` contains the single commit on `fix/wave32-rescue`.
`target/wave32-rescue.bundle` carries that commit with `8b7c3e7` as prerequisite.
The original worktree retains its edits and HEAD; nothing was pushed.
Import into a clean checkout based on `8b7c3e7` with:

```powershell
git fetch F:/wt-trace/target/wave32-rescue.bundle fix/wave32-rescue
git cherry-pick FETCH_HEAD
```

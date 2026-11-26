# Diagnosing what the title does

Notes from a series of investigations on Series 1.0.2.1, written down
because the same few measurements kept answering questions that looked
unrelated. Each section names the switch that produces the data, what the
output looks like, and - as importantly - what the data cannot tell you.

Everything here is observation. None of the switches change what the title
is asked to do, and all of them are inert unless set.

## A breakpoint seconds into startup

Symptom: execution stops at a fixed `int3` inside the first second, with
nothing in the log to say why. The address looks like an assertion and it
is one, but not the one the neighbourhood suggests.

```
VICTORY_TRACE_HLE=1
```

Prints each HLE export name at the dispatch point, plus path metadata for
`CreateFileW` and `GetFileAttributesW` and byte counts for `ReadFile`.
Function names, paths and lengths only.

The useful pattern is the last few file probes before the halt:

```
GetFileAttributesW "G:\<archive>" -> 32
CreateFileW       "G:\<archive>"
ReadFile x5
GetFileAttributesW "G:\<archive C>" -> 0xFFFFFFFF
```

Two conditions produce the identical breakpoint: the archive is absent, or
it is present but another process holds it open with an incompatible share
mode. In the second case the attribute probe succeeds and the DirectStorage
open reports OS error 32. A file listing will therefore not distinguish
them; `victory check` does, because it tries to open each file.

## Geometry that is there, colour that is not

Symptom: the picture is nearly black while depth-driven effects still
behave, so something is being drawn but not shaded.

```
VICTORY_CAPTURE=DIR VICTORY_CAPTURE_DEPTH=1 VICTORY_CAPTURE_AT=70,85,100
```

Writes `<frame>.png` and `<frame>-depth.png` from the same present, the
depth normalised per frame across its finite range. Comparing the pair
separates "the depth pass holds geometry the colour pass never carries"
from "nothing was drawn at all".

Two cautions, both learned the hard way:

- Luminance by itself proves nothing. A title card that is a dark slate by
  design reads a mean of 31-35 out of 255 and is rendering correctly.
- Applying a gamma curve to the presented buffer and seeing the picture
  appear proves the pixels arrive, and nothing more. It is a useful way to
  tell "absent" from "dark", not evidence of a bug.

## Frames that hitch without dropping

Symptom: the frame rate is steady and the picture still freezes.

```
VICTORY_PRESENT_TIMES=out.csv
```

One row per present with the interval and running percentiles, `window,`
rows every 300 presents, and a `total:` row at the end. Wall-clock
scheduling matters here: with `VICTORY_CAPTURE_FRAMES` the same frame
number is a different moment on two machines, so two runs cannot be
compared.

A real run: `p50 33.4 ms, p95 105.4 ms, p99 370.8 ms, worst 3527 ms` with
77 presents over 100 ms - against `p99 34.1 ms` and three presents over
100 ms for the same build sitting in a menu. The average would never show
it.

```
VICTORY_STALL_LOG=100
```

Adds, for any present slower than the threshold, what happened during that
frame: pipelines created, shaders translated, texture uploads. Counters
that cannot be read cheaply are reported as `na` rather than guessed.

In that run nearly nine stalls in ten carried a non-zero upload count (81
of 91, and 88-90% on two further runs) while shader translation appeared in
a handful. That is a correlation and should be treated as one: batching
uploads to reduce the count made the same machine worse, and moving
pipeline-library writes to exit changed nothing.

## Audio that repeats instead of dropping out

Symptom: a short fragment loops rather than the sound cutting out.

```
VICTORY_TRACE_AUDIO=1
```

Per voice and per submission: byte count, pending compressed bytes, active
buffer count, read and write positions, a hash over the first 64 bytes,
and whether completion was reported immediately or deferred.

The hash is FNV-1a 64, checked against the published vectors, so a hash in
a log can be recomputed independently. Equal hash and byte count on one
voice points at a replay; a submission counter that stops while blocks keep
being taken points at a starved feed.

`pending` and `queue` are context snapshots for the command path this title
uses, not playback counters. The per-voice playback counters are only
advanced by the write-ring path and read zero here, which is exactly the
sort of number that looks like a finding and is not.

## Compute pipelines refused with an invalid-argument error

Symptom: draws silently go missing; some pipeline creations fail with
`0x80070057`.

```
VICTORY_TRACE_PSO=1
```

On failure only: kind, shader hash, root parameter count, bound descriptor
counts, node mask, flags, blob length and HRESULT. No shader contents.

The cases seen here were all compute, with a valid DXIL blob, a zero node
mask, no flags and a small root signature - which leaves the shader itself.
On a 32-lane host, a shader that still declares a 64-wide wave after
optimisation is refused; narrowing it where the shader cannot observe the
wave width is what #988 does.

## Two habits worth keeping

**One number is not a measurement.** A single voice looked starved at
2.8 KB/s until the same run was summed across all 351 voices and read
468 KB/s. Single-frame luminance looked broken until the stage was
identified. Either figure on its own would have sent the next hour in the
wrong direction.

**Say what the data does not cover.** A test that proves which shaders are
narrowed says nothing about numerical equivalence on the GPU. A stall count
on one machine says nothing about another. The limits belong next to the
result, in the same place a reader will look.

## Reproducing a shader census with validation

The numbers quoted in the sections above come from one command, and anyone
can repeat it. Two things are needed that are not present by default.

**A validator.** `dxil.dll` ships with the Windows SDK and with the DXC
releases. If it is not on the machine, take `dxc_<date>.zip` from the DXC
releases page, unpack it, and point `VICTORY_DXIL` at `bin/x64/dxil.dll`.
Without it the census still runs, but the validator column is absent and
two of the three defect classes below are invisible.

**The wave width of the host.** Set `VICTORY_HOST_WAVE=32` on a machine
whose waves are 32 lanes, otherwise the translation targets a width the
GPU does not have and the numbers are not comparable.

```
victory translate <shaders.rpf> --validate --pipelines --list
```

What the output is good for, in order of increasing cost:

- the per-stage summary lines (`DXIL ... emitted ... validated ... pipelines`)
  say whether a change moved anything at all
- `--list` gives one line per program, so a specific hash can be followed
  from "translated" to "pipeline created"
- the failure summary at the end groups what is left by reason, which is
  how the three classes below were found

To look at one program rather than twenty-six thousand, set
`VICTORY_TRANSLATE_ONLY` to a comma-separated list of the sixteen hex
digits the log names a shader by.

Three classes are worth checking by name, because none of them prevents a
pipeline from being created and so none shows up in a census run without
`--validate`:

```
interpolation mode that differs from another element packed into the same row
Container part 'Program Input/Output Signature' does not match expected for module
out of bound coord must be undef
```

On the retail pack, at the time of writing, the first two and the third are
zero and 72 mesh programs still exceed the combined vertex and primitive
output limit of 32768 components. That last class is a genuine budget
question rather than a code defect: the overshoot is 128 to 896 components
and the affected programs spend nothing on primitive outputs.

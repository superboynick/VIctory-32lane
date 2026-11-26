---
name: Report a run
about: A run that crashed, stalled, or rendered wrongly - with the data that makes it comparable
---

**What happened, in one sentence**

**Machine**
- GPU and driver:
- CPU, RAM:
- Windows build:
- Complete archive set? If not, what is missing:

**Before anything else**

```
victory check
```

Paste the summary line. A missing or locked archive produces the same startup breakpoint as a
defect, and this separates them.

**If it crashed**
- `VICTORY_TRACE_HLE=1` output around the last file operations, and the exception line with its
  image offset. Function names and paths only - no buffer contents.

**If it stalls**
- `VICTORY_PRESENT_TIMES=<file>` plus `VICTORY_STALL_LOG=100`, and the `window,` rows from the CSV.
  A frame rate figure hides hitching entirely.

**If it renders wrongly**
- `VICTORY_CAPTURE=<dir>` with `VICTORY_CAPTURE_DEPTH=1`, and the mean luminance of one colour frame
  and one depth frame from the same present, if you can compute it. A dark title card is not a
  defect, so say what the frame was supposed to show.

**If a pipeline is refused**
- `VICTORY_TRACE_PSO=1` output for the failure. It reports kind, hash, root parameters, descriptor
  counts, node mask, flags, blob length and HRESULT, never shader contents.

Please do not attach game assets, dumps of shader contents, or screenshots of the game itself.
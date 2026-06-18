---
id: executable-steps
label: Executable Steps (Run buttons)
type: decision
community: ux-capture
edges:
  - target: copy-per-code-block
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: ipc-boundary
    relation: implements
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: settings-and-hotkey
    relation: part_of
    confidence: EXTRACTED
    confidence_score: 0.9
---

Phase 6 / D11. Every fenced code block already has a [[copy
button|copy-per-code-block]]; an opt-in **▶ run** button sits beside it and
executes the command in the user's login shell (`run_command` → `$SHELL -c`),
rendering captured stdout/stderr + exit code inline below the block.

**Gated, off by default.** An `allow_run` setting (Settings → Execution) controls
it, and the gate is **re-checked inside the Rust command** — hiding the button is
not a security boundary, refusing the IPC call is. Trust model is intentionally
simple: a runbook is the user's own notes, commands run with the user's own
permissions, so the boundary is the explicit enable, not sandboxing.

Implementation notes worth remembering:
- Output rendered via `textContent`, never `innerHTML`, so command output can't
  inject markup into the overlay.
- The run text is `code.textContent`, so `{{var}}` placeholders are already
  filled (same as copy). A markdown re-render (e.g. variable change) clears the
  output panel — acceptable, it'd be stale.
- No streaming / long-running process support in Tier 1, but **a never-exiting
  command no longer hangs the Run button forever** (2026-06-18): `run_command`
  now pipes both streams, drains each on its own thread (so a >64KB-output
  command can't deadlock on a full pipe buffer — the classic capture-deadlock the
  old single `.output()` avoided only because it read concurrently), then polls
  `try_wait` until a `RUN_TIMEOUT` (300s) deadline and `kill()`s on overrun. On
  timeout it returns the partial output with `exit_code: None` (UI shows "exit ?")
  and a "…timed out after 300s…" note appended to stderr. Still `(async)` so the
  whole wait runs off the main thread.
- A custom `#[tauri::command]` needs **no capability grant** (those gate
  core/plugin perms only).

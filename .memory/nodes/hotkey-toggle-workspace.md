---
id: hotkey-toggle-workspace
label: Summon toggle must check focus, not just visibility
type: gotcha
community: Architecture
edges:
  - target: global-hotkey
    relation: part_of
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: overlay-window
    relation: references
    confidence: EXTRACTED
    confidence_score: 1.0
---

Reported as "the global hotkey doesn't open the window". The grab itself was
**fine** — verified live on X11: `Ctrl+Alt+Space` was actually grabbed (an Xlib
test grab returned `BadAccess`), `global-hotkey 0.8` grabs all lock-mod variants
(NumLock was on), and an `xdotool key ctrl+alt+space` (XTEST) toggled the window.
So registration/parse/delivery all worked.

Root cause: the old `toggle_overlay` keyed off `is_visible()` only. A window
mapped on **another workspace** still reports `is_visible() == true`, so the
toggle *hid* it (off where you can't see it) instead of bringing it forward —
indistinguishable from "nothing happened".

**Fix (2026-06-16, `src-tauri/src/lib.rs`):**
- `toggle_overlay` now checks **focus too**: `visible && focused → hide`, else
  `show + unminimize + set_focus`. So an unfocused/other-workspace overlay is
  summoned forward rather than hidden.
- At startup `win.set_visible_on_all_workspaces(true)` (GTK `window.stick()`, does
  **not** map the window — `visible:false` still holds) so summon lands on the
  current workspace. Side effect: the window's `_NET_WM_DESKTOP` becomes
  "all desktops", which makes `xdotool windowactivate` print a harmless error.
- Hotkey registration now `eprintln!`s success/failure instead of silently
  swallowing it (X11 grab clashes are otherwise invisible).

Diagnosing this needs a live X11 session; `cargo check`/`npm run build` won't
surface it. Tools used: `xwininfo`/`xdotool`/`xwd`+`ffmpeg`.

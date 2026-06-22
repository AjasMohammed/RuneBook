---
id: link-navigation-trap
label: Markdown Link Navigation Trap
type: gotcha
community: ux-and-capture
edges:
  - target: overlay-window
    relation: caused_by
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: ai-reports
    relation: references
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: ipc-boundary
    relation: implements
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: project-pinning
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: x11-target
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 0.9
---

A clickable link in rendered markdown — a runbook note or an [[AI report|ai-reports]]
(e.g. `[README](README.md)`, or any `http`/`file` URL `marked` produces) — was a
**dead end**. `marked` renders it to a real `<a href>`, and the default click action
navigates the **WebView itself** to the target, unloading the entire Svelte SPA.

**Symptom seen (2026-06-22):** clicked a README mention in a note → the README/target
loaded into the [[overlay-window]], which is frameless + transparent + always-on-top,
so the page showed with a **fully transparent background**, **Esc did nothing** (the
`on:keydown` handler was part of the SPA that just got unloaded), and there was **no
titlebar / Back button**. The only escape was killing the app/dev server.

**Why it traps so completely:** every recovery affordance lives *in* the SPA. Once the
WebView navigates away, the hotkey-toggle still shows/hides the (now-wrong) window but
can't restore the app; there is no chrome to go back. This is inherent to the overlay
being decoration-less and transparent ([[overlay-window]], [[x11-target]]).

**Fix:** intercept content-link clicks before they can navigate.
- Frontend (`App.svelte` `onAnchorClick`, wired via `<svelte:window on:click|capture>`):
  `e.target.closest("a[href]")`; leave `#anchors` (report TOC / in-page scroll) alone;
  `preventDefault()` everything else so the overlay never unloads. Scheme URLs
  (http/https/mailto/file/…) and absolute paths go straight to the OS.
- **Relative links** (`README.md`) — added after the first cut shipped, because
  cancelling them made a click *do nothing*, which read as a new bug. They're resolved
  against the runbook's **pinned folder** (`selected.projectDir`, [[project-pinning]]
  / D15) and opened. With **no folder pinned**, `pickAndPinFolder()` opens the native
  folder picker, pins the chosen dir (so later links + ▶ Run reuse it without
  re-asking; reversible via the pin chip ✕), then opens — user's choice over a passive
  banner or a silent fallback. Cancelling the picker bails quietly. (The MCP
  feature-test runbook #8 has `[README](README.md)` and is *not* pinned — that's the
  exact case that surfaced the silent-no-op.)
- Rust (`open_external(url, base)`, registered in `lib.rs`): `is_absolute_or_url()`
  decides pass-through vs. `Path::join(base, url)`; then `xdg-open <target>` as a single
  argv element (no shell → no injection), `(async)` + `.status()` so it's off the main
  thread and reaps the child. Keeps OS access in the core per [[ipc-boundary]]. Linux
  target → `xdg-open` ([[x11-target]]); no new dep or capability needed.

**Gotcha for future link-ish features:** anything that puts an `<a href>`, a `<form>`,
or `window.location`/`location.assign` into the overlay can re-introduce this trap.
The frontend guard only covers anchor clicks; a true catch-all would be a Rust
`WebviewWindowBuilder::on_navigation` deny (would require building the window in code
instead of `tauri.conf.json`).

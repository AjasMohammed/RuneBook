---
id: select-option-color
label: Native <select> can't be themed in WebKitGTK → custom dropdown
type: gotcha
community: UX & Capture
edges:
  - target: quick-add-capture
    relation: part_of
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: x11-target
    relation: conceptually_related_to
    confidence: INFERRED
    confidence_score: 0.6
---

The Quick-add runbook picker was a native `<select>`. On Linux/WebKitGTK its
**dropdown popup is rendered by GTK**, not the web engine, so it ignores
`<option>` CSS entirely — `option { color: #000; background: #fff }` had **no
effect** (the options stayed unreadable: light text on the system-light popup).
This was my first attempted fix and it failed; do not retry option-styling.

**Resolution (2026-06-16):** replaced the `<select>` with a **custom DOM
dropdown** in `src/App.svelte` — a `.rb-trigger` button toggling `pickerOpen`,
and a `.rb-menu` `<ul role="listbox">` of `<button>`s, fully styled in `app.css`
(opaque `#1d1d20` popup, `--fg` text, accent for the current item). Closes on
outside click (`onDocClick` via `<svelte:window on:click>`, trigger lives inside
`pickerEl` so its own click doesn't self-close) and on Esc (handled first in
`onKeydown`). Screenshot-verified readable. General rule for this app: **don't
rely on native form-control popups being styleable** under WebKitGTK; build the
control in the DOM.

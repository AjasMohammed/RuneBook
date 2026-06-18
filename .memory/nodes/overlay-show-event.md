---
id: overlay-show-event
label: overlay:show event
type: concept
community: Architecture
edges:
  - target: global-hotkey
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: quick-add-capture
    relation: part_of
    confidence: EXTRACTED
    confidence_score: 1.0
---

When the overlay is *shown* (via the global hotkey, in `toggle_overlay`), the
Rust core emits a Tauri event **`overlay:show`** (`app.emit`, needs the
`tauri::Emitter` trait). The frontend listens with `@tauri-apps/api/event`'s
`listen` and reacts by switching to Quick-add mode and focusing the composer —
so summoning always lands ready to capture.

Only emitted on the hidden→shown transition, not on hide. The listener's unlisten
handle is cleaned up in Svelte's `onDestroy` (an `async onMount` return value is
*not* used as cleanup — a Svelte gotcha).

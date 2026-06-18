---
id: settings-and-hotkey
label: Settings + runtime hotkey
type: decision
community: UX & Capture
edges:
  - target: current-runbook-setting
    relation: part_of
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: global-hotkey
    relation: references
    confidence: EXTRACTED
    confidence_score: 1.0
---

Phase 5 Settings (done 2026-06-16) — a third overlay mode (`⚙` tab) in
`src/App.svelte`:

- **Custom hotkey:** `set_hotkey(spec)` parses with `Shortcut::from_str`,
  `unregister_all()` then `register()`, and persists the spec in the
  [[current-runbook-setting|setting kv table]] under `hotkey`. Applied at startup
  from the setting (default `Control+Alt+Space`), falling back to default if a
  saved spec won't parse. Because only ever one shortcut is registered, the
  global-shortcut handler simply toggles on *any* Pressed event (no compare).
- **Accent theme:** presets persisted under `accent`; applied by setting the
  `--accent` CSS variable on `:root` (loaded on mount). Orange stays the default.
- **Launch on login:** `tauri-plugin-autostart`, toggled via `get/set_autostart`
  using the plugin's Rust `ManagerExt::autolaunch()` (no JS ACL needed).

All settings persist in SQLite, consistent with the IPC-boundary choice.

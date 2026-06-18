---
id: tauri-over-electron
label: Tauri chosen over Electron / native
type: decision
community: Architecture
edges:
  - target: overlay-window
    relation: implements
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: x11-target
    relation: conceptually_related_to
    confidence: INFERRED
    confidence_score: 0.6
---

Stack decided as **Tauri v2** (Rust core + web UI). Reasons: ~5MB bundle vs Electron's
~150MB, low RAM, native global-hotkey/tray/overlay support, and the UI reuses the
user's existing web + typography skills. Native GTK/Qt was rejected — too much UI
rebuilding, no design-system reuse. The [[overlay-window]] and [[global-hotkey]]
realize this choice. See `docs/05-decisions.md` D1.

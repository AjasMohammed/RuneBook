---
id: overlay-window
label: Single frameless overlay window
type: concept
community: Architecture
edges:
  - target: global-hotkey
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: system-tray
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: x11-target
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
---

The whole app is **one** Tauri window configured as a floating card: `decorations:
false`, `transparent: true`, `alwaysOnTop: true`, `skipTaskbar: true`, starts
`visible: false`. Quick-add and Browse are UI modes inside this one window, not
separate OS windows (D5). It [[depends on the hotkey|global-hotkey]] to toggle and the
[[system tray|system-tray]] to stay alive after close. Config lives in
`src-tauri/tauri.conf.json`; behavior in `src-tauri/src/lib.rs`.

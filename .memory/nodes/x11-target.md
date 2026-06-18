---
id: x11-target
label: X11 is the v1 target
type: decision
community: Environment & Build
edges:
  - target: overlay-window
    relation: implements
    confidence: EXTRACTED
    confidence_score: 0.9
---

`XDG_SESSION_TYPE=x11` confirmed on this machine (2026-06-15). On X11 transparency,
always-on-top, and global shortcuts all work without compositor workarounds, so the
[[overlay-window]] and [[global-hotkey]] need no Wayland-specific handling. Wayland
support is explicitly **out of scope for v1** (D6) — under Wayland global shortcuts
are restricted and overlay positioning differs.

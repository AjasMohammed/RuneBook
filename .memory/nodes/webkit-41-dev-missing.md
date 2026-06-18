---
id: webkit-41-dev-missing
label: webkit2gtk 4.1 dev headers (RESOLVED)
type: gotcha
community: Environment & Build
edges:
  - target: x11-target
    relation: part_of
    confidence: INFERRED
    confidence_score: 0.7
---

**RESOLVED (2026-06-15, Phase 2).** The **webkit2gtk 4.1 `-dev` headers are now
installed** on this machine — `pkg-config --exists webkit2gtk-4.1` succeeds and
`cargo check` compiles the Rust core (~55s incl. bundled SQLite). The earlier
build blocker is gone.

History: at Phase 0 scaffolding the machine had only the 4.0 runtime
(`libwebkit2gtk-4.0.so.37`) and not the 4.1 dev package, so `cargo build` failed.
On a fresh box, install with:

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev build-essential curl wget file
```

Note: a full interactive `npm run tauri dev` still needs a graphical X11 session;
`cargo check` + `npm run build` are the headless-verifiable checks.

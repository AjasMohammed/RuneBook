---
id: production-readiness
label: Production Readiness (2026-06-18)
type: task
community: environment-build
edges:
  - target: x11-target
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: executable-steps
    relation: references
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: webkit-41-dev-missing
    relation: references
    confidence: INFERRED
    confidence_score: 0.7
---

Readiness assessment + hardening pass on 2026-06-18.

**Verified working:** all 26 Rust tests pass (12 core + 14 MCP), `npm run build`
is clean, and the app now produces a real installable bundle —
`cargo tauri build` → `Runebook_0.1.0_amd64.deb` (~5.3 MB, binary ~15 MB).
Package is well-formed: `Depends` = libwebkit2gtk-4.1-0/libgtk-3-0/libayatana-
appindicator3-1, binary at `usr/bin/runebook`, icon in `hicolor/128x128/apps`,
and a `Runebook.desktop` with `Categories=Utility;`.

**Changes made this pass:**
- `bundle.targets` narrowed from `"all"` → `["deb"]` (appimagetool and rpmbuild
  are NOT installed on this box; "all" would fail on the appimage/rpm stages).
- Added `bundle.category: "Utility"` so the launcher entry isn't uncategorized.
- Replaced the 385-byte **placeholder icons** with a real set: `icon-source.gen.py`
  (Pillow) renders a 1024² source — warm-charcoal rounded tile + ember raidho rune
  (ᚱ) + bookmark ribbon — then `npx tauri icon` regenerates every size.
- `run_command` got a 300s timeout + kill (see [[executable-steps]]).
- Fixed the README "Stack" `tauri-plugin-sql` contradiction (it's `rusqlite`, D7).

**THE REMAINING GATE — not yet cleared:** the full interactive app
(`npm run tauri dev`, or launching the installed `.deb`) has **never been run in
any environment** — every "done" is verified by compile + unit tests only, because
the build host has no graphical session. The overlay transparency, global-hotkey
toggle, tray, summon-across-workspaces, and the live WebView flows are unobserved.
This [[needs an X11 session|x11-target]] (the user's Pop!_OS) to clear. Do this
before trusting it for daily use.

**Still open before distributing to others (not just dogfooding):** no LICENSE,
no auto-updater, no CI, zero frontend tests (App.svelte ~1300 lines), version
still 0.1.0, and secrets in steps remain plaintext in SQLite (decisions Q4 / D13).

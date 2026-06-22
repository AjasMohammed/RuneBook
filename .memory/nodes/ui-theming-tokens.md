---
id: ui-theming-tokens
label: Accent-derived Tokens & Bundled Fonts
type: decision
community: ux-capture
edges:
  - target: settings-and-hotkey
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: select-option-color
    relation: conceptually_related_to
    confidence: INFERRED
    confidence_score: 0.6
---

A 2026-06 UI-hardening pass fixed two design-system gaps that were invisible until
you looked:

1. **Fonts were declared but never loaded.** `app.css` set `font-family: "Hanken
   Grotesk"` but nothing imported it, so the whole overlay silently fell back to
   `system-ui`. Now `main.js` imports `@fontsource/hanken-grotesk` (400/500/600/700)
   as the reading face and `@fontsource/space-grotesk` (500/700) as the display
   face, applied via `--font-display` to `.title`, `.detail h2`, `.rendered h1-h3`,
   and the editor headings. Bundled (not a Google Fonts `<link>`) so the offline
   overlay never depends on the network.

2. **Accent tints were hard-coded orange.** Many backgrounds/borders used literal
   `rgba(232,93,4,…)` while their text used `var(--accent)`, so switching accent to
   Teal/Violet gave teal text on an orange background. Fix: `applyAccent()` now also
   sets `--accent-soft` (0.14α) and `--accent-line` (0.5α) in JS from the chosen hex
   (`accentRgba()`), and the CSS literals were replaced with those vars. Status
   colors became semantic tokens distinct from the accent: `--ok`/`--ok-soft` and
   `--err`/`--err-soft` — so an error banner is red, not "whatever the accent is."

Rule of thumb going forward: never write an accent-tinted `rgba(232,93,4,…)`
literal — derive from `--accent-soft`/`--accent-line`; use `--ok`/`--err` for
status, never `--accent`.

---
id: command-palette
label: Command Palette (⌘K)
type: decision
community: ux-capture
edges:
  - target: fts5-search
    relation: conceptually_related_to
    confidence: EXTRACTED
    confidence_score: 0.8
  - target: optional-steps
    relation: references
    confidence: INFERRED
    confidence_score: 0.6
---

Phase 8 / D14. `Ctrl/Cmd+K` opens a centered modal quick-switcher that filters the
**already-loaded** runbook list by title/tag substring (instant, no IPC), with
`↑↓` to move, `↵` to open the pick in Browse, `Esc`/click-outside to close.

Deliberately **not** wired to [[FTS5|fts5-search]]: the palette is for fast
*jumping* between runbooks you know exist, where client-side filtering beats a
round-trip; deep step-body search already lives in the Browse search box. Zero new
Rust surface — pure frontend. Capped at 50 results.

Implementation gotchas:
- Outside-click-to-close is handled via the existing window `onDocClick` (checking
  `!paletteCard.contains(target)`), **not** an `on:click` on a backdrop `<div>` —
  Svelte a11y-warns on click handlers on non-interactive elements. Safe because the
  palette only opens via the keybind, never a click (no open-then-close race).
- Both Ctrl+K (toggle) and Esc close paths live in the global `onKeydown`; Esc also
  has a per-input handler that `stopPropagation`s so it doesn't hide the overlay.

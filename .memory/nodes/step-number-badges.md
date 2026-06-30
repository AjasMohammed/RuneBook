---
id: step-number-badges
label: Step Number Badges + Dead Title
type: gotcha
community: ux-capture
edges:
  - target: markdown-step-model
    relation: caused_by
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: optional-steps
    relation: references
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: ui-theming-tokens
    relation: part_of
    confidence: INFERRED
    confidence_score: 0.8
---

2026-06: UI redesign pass on Browse. Two things worth remembering:

**The `.step-title` span was always a duplicate.** Under the [[single-notepad
model|markdown-step-model]] (D8) a step's `title` is always `""`, so the Browse
markup `s.title?.trim() || deriveLabel(s.body, i)` *always* fell through to
`deriveLabel`, which re-extracts the body's own first line. Result: every
multi-step runbook printed its first line twice ("Pull latest / Pull latest").
Fixed by **deleting the `.step-title` span** (and its dead CSS) — the rendered
body's first line/heading already is the label. `deriveLabel` is still used for
the replay checkbox `aria-label`, so keep the function.

**Step markers are now CSS-counter accent badges, not `list-style`.** `.steps`
is `list-style:none; counter-reset:step`; each `li::before` renders
`counter(step)` in an accent-tinted circle (display face). `.single` hides the
badge (one step = plain note, see [[optional-steps]]); `.replay` hides it too and
absolutely positions the `.step-check` checkbox in its place. The `.step-head`
now reserves no leading line — `.step-tools` (and the replay checkbox) are
absolutely positioned, so the body's first row sits *beside* the badge.

Other redesign tells fixed in the same pass: thin `::-webkit-scrollbar` (the
chunky default GTK bar was the biggest "unstyled" signal), segmented-control
tabs, input focus ring via `--accent-soft`, active sidebar row gets an inset
accent left-bar. Long unbroken tokens in `.rendered` now `overflow-wrap:anywhere`
so prose can't push the detail pane past the overlay (which clips it).

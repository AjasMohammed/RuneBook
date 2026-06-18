---
id: optional-steps
label: Steps are optional (emergent note shape)
type: decision
community: UX & Capture
edges:
  - target: markdown-step-model
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: quick-add-capture
    relation: conceptually_related_to
    confidence: INFERRED
    confidence_score: 0.7
  - target: ipc-boundary
    relation: implements
    confidence: EXTRACTED
    confidence_score: 1.0
---

A runbook no longer always presents as a numbered step list. The shape is
**emergent from the step count** (computed in `src/App.svelte`), not a stored
type/flag — so there is **no migration**:

- **0 steps** → Browse renders an inline note composer (`addBuffer` +
  `saveAddStep`), so a runbook made via "New runbook title…" can be filled in
  immediately. `createRunbook` focuses that composer. Replaced the old
  "No steps yet — capture some in Quick-add" empty state.
- **1 step** → a plain note: no `Step N` number, no ↑↓ reorder (gated on
  `selected.steps.length > 1`); `.steps.single` drops the list marker/indent.
- **2+ steps** → the numbered list with full tools (unchanged).
- An always-present **＋ Add step** button (`startAddStep`) grows a note into a
  multi-step runbook.

Also in this change (2026-06-16, user asked for both): the open runbook's
**title is editable** in Browse — inline `<input>` swaps in on click of the
heading/✎, commits on Enter or blur, cancels on Esc (its keydown stops
propagation so Esc doesn't hide the overlay). Reuses the existing
`update_runbook` title patch ([[ipc-boundary]]) — no new Rust. Previously a title
could only be set at creation (`createRunbook` / `createAndSelect`).

Recorded as **D9** in `docs/05-decisions.md`; updates the Browse section of
`docs/03-overlay-and-ux.md`. Chosen over an `is_simple`/note-type column because
it keeps one data model ([[markdown-step-model]]: runbook → steps) and a note
converts between shapes just by gaining/losing steps. Verified with
`npm run build`; no Rust changed.

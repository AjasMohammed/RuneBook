---
id: replay-progress
label: Replay Progress (checklist)
type: decision
community: ux-capture
edges:
  - target: optional-steps
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: rusqlite-data-layer
    relation: implements
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: ipc-boundary
    relation: implements
    confidence: EXTRACTED
    confidence_score: 1.0
---

Phase 6 / D10. A **Replay** toggle turns a 2+-step runbook into a checklist:
per-step checkboxes, a progress bar, **Reset**, and a `done/total` badge on cards.

Key design choice: store **one mutable checklist state per runbook**, *not* a
history of timestamped runs — the value is "resume where I left off", which a
single state delivers with far less schema/UI. Persisted in a [[step_progress
table|rusqlite-data-layer]] (migration **v5**), keyed by `step_id`, `ON DELETE
CASCADE`. Exposed two ways: `get_runbook` LEFT JOINs progress so each `step`
carries a `done` flag (powers the checkboxes); `list_progress()` returns grouped
counts (only where `done > 0`) for the card badge.

Replay is a per-open-runbook UI mode (`replayMode`), reset to off on open, and
only offered when `steps.length > 1` (a one-note runbook has nothing to track).
Only works because [[a step is already a self-contained note|optional-steps]].
IPC: `set_step_done`, `reset_progress`, `list_progress`.

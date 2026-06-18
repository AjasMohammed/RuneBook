---
id: quick-add-capture
label: Quick-add capture mode
type: concept
community: UX & Capture
edges:
  - target: overlay-show-event
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: current-runbook-setting
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: markdown-step-model
    relation: references
    confidence: EXTRACTED
    confidence_score: 1.0
---

Phase 4 (done 2026-06-16). The overlay has **two modes in one window** (D5):
**Quick-add** (capture) and **Browse** (replay), switched by tabs in the header.
Quick-add is the default and the hotkey landing — `src/App.svelte`.

Capture loop (mouse-free): summon → composer focused → type the markdown note →
**Cmd/Ctrl+Enter = save & next** (appends the step, clears the form, refocuses,
shows a "Saved ✓" flash + session counter). Handled in the window `keydown` gated
on `mode === "quick"` (not a handler on the `<section>` — that tripped a Svelte
a11y warning). The composer is now the [[markdown-editor-component|MarkdownEditor]]
(single notepad + toolbar + live preview); `focusComposer` calls its exported
`focus()` via `bind:this`, not a raw `<input>` ref.

New steps go to the [[current-runbook-setting|current runbook]]; the picker is a
`<select>` of existing runbooks plus an inline "＋ new runbook" field. If none is
set, the first save creates a runbook **named from the first note** (its
`deriveLabel`, capped at 50 chars) — *not* the old "Untitled runbook" — and makes
it current.

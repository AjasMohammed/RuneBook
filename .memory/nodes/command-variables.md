---
id: command-variables
label: Command variables / placeholders
type: concept
community: UX & Capture
edges:
  - target: copy-per-code-block
    relation: part_of
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: markdown-export
    relation: conceptually_related_to
    confidence: INFERRED
    confidence_score: 0.7
---

Backlog item, built 2026-06-16. Steps may contain `{{name}}` placeholders (e.g.
`ssh deploy@{{host}}`). In the Browse view the distinct placeholder names across
the open runbook get fill-in fields (a "Variables" panel); values are substituted
**live** in the rendered markdown preview and in copied commands. Frontend only —
`src/App.svelte`: scan with `/\{\{\s*([\w.-]+)\s*\}\}/g`, `fillVars()` replaces
filled placeholders (unfilled ones stay literal so they're visible), and the
`markdown` action re-renders when either the body or `varValues` changes.

Design choices: values are **in-memory only, never persisted** (they may be
secrets), and reset when switching runbooks. **Export keeps the raw `{{…}}`
template** (export_markdown reads raw bodies from the DB), so an exported
`RUNBOOK.md` stays reusable.

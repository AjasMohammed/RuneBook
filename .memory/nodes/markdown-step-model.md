---
id: markdown-step-model
label: Steps are free-form markdown
type: decision
community: Data & Storage
edges:
  - target: rusqlite-data-layer
    relation: implemented_by
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: copy-per-code-block
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
---

A **step** is an optional `title` + a free-form **markdown** `body` — *not* the
original fixed fields (command/why/where_ctx/example/note). The user wants a
customizable scratchpad to capture and revise a workflow however they think
about it, not a rigid form. Decided 2026-06-15 when the user rejected the
form-based Phase 2 UI ("it should be a more advanced customisable tool… support
markdown").

Recorded as **D8** in `docs/05-decisions.md`; supersedes the fixed-field model in
docs 02/03. **Migration v2** (`src-tauri/src/db.rs`, tracked by
`PRAGMA user_version`) backfills any v1 rows into markdown — command → fenced
block, why → text, where/example → list, note → blockquote — then `DROP`s the old
columns (needs SQLite ≥ 3.35, satisfied by bundled rusqlite). Consequence: the
old `where_ctx` reserved-word workaround is moot, that column no longer exists.

**Update 2026-06-16 — title field dropped from the UI.** The user found every
quick-add note showed as "Untitled" and asked for a single notepad. The
[[markdown-editor-component|MarkdownEditor]] now has **no title input**: new steps
save `title: ""` and the body is the whole note. The `title` *column* stays
(legacy rows + FTS index) and is preserved on edit (patch omits it → COALESCE
keeps it), but the display label is derived from the body's first line. So a step
is now effectively **body-only** at the UX layer.

---
id: fts5-search
label: FTS5 ranked search
type: concept
community: Data & Storage
edges:
  - target: markdown-step-model
    relation: references
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: bm25-materialized-gotcha
    relation: caused_by
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: rusqlite-data-layer
    relation: part_of
    confidence: EXTRACTED
    confidence_score: 1.0
---

Phase 3 search (done 2026-06-16). Migration v4 adds an external-content FTS5
table `step_fts(title, body)` mirroring `step`, kept in sync by AFTER
INSERT/UPDATE/DELETE triggers (the `'delete'` command form), backfilled with
`INSERT INTO step_fts(step_fts) VALUES ('rebuild')`. `list_runbooks(query)` ranks
step hits by **bm25** and also matches runbook title/description/tags (title
first, then bm25, then recency). Free text is tokenized into quoted prefix terms
(`"ssh"*`) so partial typing matches and punctuation can't break MATCH syntax.

FTS5 **is** compiled into the bundled rusqlite build (confirmed by a unit test in
`db.rs`). If a build ever lacks it, migration v4 skips without bumping
`user_version` and search falls back to `LIKE` over the same fields — so the
feature degrades, never crashes. Verified by tests covering ranking, trigger
sync on update/delete, and tag/title matching.

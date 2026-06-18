---
id: bm25-materialized-gotcha
label: bm25 needs AS MATERIALIZED
type: gotcha
community: Data & Storage
edges:
  - target: fts5-search
    relation: part_of
    confidence: EXTRACTED
    confidence_score: 1.0
---

SQLite's `bm25()` (and other FTS5 auxiliary functions) can only be evaluated in a
query that *directly* `MATCH`es the FTS table — **not** inside an aggregate and
**not** across a join. Both forms fail at runtime with:

> `unable to use function bm25 in the requested context`

Hit twice while building [[fts5-search]]: `MIN(bm25(step_fts))` (aggregate) and
scoring it in a CTE joined to `step` both failed, because SQLite *flattened* the
CTE back into the join. Fix: score each hit in an inner CTE that directly matches
the FTS table, and mark it **`AS MATERIALIZED`** (SQLite ≥ 3.35) to block
flattening; aggregate the materialized scores in an outer CTE. Caught only
because there were unit tests — `cargo check` compiles the bad SQL fine.

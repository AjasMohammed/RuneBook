---
id: local-rustfmt-mismatch
label: Local rustfmt ≠ project rustfmt
type: gotcha
community: Environment & Build
edges:
  - target: release-ci-and-self-update
    relation: references
    confidence: EXTRACTED
    confidence_score: 0.8
  - target: pipe-masks-exit-code
    relation: conceptually_related_to
    confidence: INFERRED
    confidence_score: 0.6
---

This machine's **`rustfmt` is a different (newer) version than the one the project's
code was formatted with / CI uses.** Running `cargo fmt --check` against the repo at
**HEAD** (code nobody touched this session) reports **13+ diffs** in `db.rs` alone
(plus several in `lib.rs`) — all the same flavour: the local rustfmt wants to
*collapse* multi-line `conn.execute_batch("…")?;` / `Ok((r.get(0)?, …))` calls that
the codebase keeps multi-line.

**Implication:** do **NOT** run `cargo fmt` (write) here to "clean up" a change — it
reformats the whole pre-existing file to the local formatter's taste, producing a
huge spurious diff that diverges from what the project's CI formatter accepts. Match
the **surrounding file style** instead (e.g. the v8 `kind` migration mirrors the v7
`project_dir` one verbatim). Verify a change with `cargo build` / `cargo test` /
`cargo clippy` (all clean this session) rather than local `cargo fmt --check`, whose
baseline is untrustworthy on this box. CI's own pinned rustfmt is the authority.

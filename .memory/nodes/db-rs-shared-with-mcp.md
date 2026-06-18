---
id: db-rs-shared-with-mcp
label: db.rs is shared with the MCP binary
type: gotcha
community: environment-build
edges:
  - target: rusqlite-data-layer
    relation: references
    confidence: EXTRACTED
    confidence_score: 1.0
---

`mcp-server/src/main.rs` includes the app's data layer verbatim via
`#[path = "../../src-tauri/src/db.rs"] mod db;` (D13). So **changing a shared type
in `db.rs` can break the MCP build even though the app compiles.**

Hit this in Phase 8: adding `project_dir` to `RunbookPatch` broke the MCP's struct
literal (`update_runbook` handler) with `missing field project_dir` — the app was
green but `runebook-mcp` wasn't. Fix was a one-liner (`project_dir: None`) in
mcp-server.

**Rule: after touching a `pub struct`/enum or function signature in `db.rs`, run
`cargo check` in BOTH `src-tauri/` and `mcp-server/`.** Additive things that are
safe: new `db` functions, new fields on `Serialize`-only structs (e.g. `Step.done`,
which the MCP only serializes, never constructs). Breaking: new fields on structs
the MCP constructs with a literal (the `*Patch` types). See [[pipe-masks-exit-code]]
— the first MCP "pass" was a false green from `| tail` eating cargo's exit code.

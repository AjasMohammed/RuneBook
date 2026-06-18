---
id: project-pinning
label: Project Pinning (run cwd)
type: decision
community: ux-capture
edges:
  - target: executable-steps
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: rusqlite-data-layer
    relation: implements
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: db-rs-shared-with-mcp
    relation: caused_by
    confidence: EXTRACTED
    confidence_score: 0.7
---

Phase 8 / D15. A runbook can be pinned to a directory via a `project_dir` column
on `runbook` (migration **v7**; "" = unpinned). Its one concrete behavior: become
the **working directory for executed commands** — `run_command(text, cwd)` gets
`selected.projectDir`, so a pinned runbook's `git pull` / `npm run build` run in
the right repo. Composes directly with [[executable-steps]].

We do **not** auto-surface a runbook by "current directory": a global desktop
overlay has no cwd to match against, so that would be fiction. `project_dir` is
populated only by `get_runbook` (detail view), left "" in list/FTS rows so those
queries stay untouched. Reuses `update_runbook`'s patch (a new `projectDir` field).

Note: adding `project_dir` to `RunbookPatch` broke the MCP build — see
[[db-rs-shared-with-mcp]].

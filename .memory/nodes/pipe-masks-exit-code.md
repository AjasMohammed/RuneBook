---
id: pipe-masks-exit-code
label: "`| tail` masks cargo's exit code"
type: gotcha
community: environment-build
edges:
  - target: db-rs-shared-with-mcp
    relation: references
    confidence: EXTRACTED
    confidence_score: 0.8
---

`cargo check 2>&1 | tail -20` reports the exit code of **`tail`** (always 0), not
of `cargo`. A failing build can look like it passed. Bit me in Phase 8: an MCP
`cargo check` that actually failed (`missing field project_dir`) was reported as
"exit code 0" because of the pipe.

**Verify build/test success from the output text** ("Finished", "test result: ok",
"error[...]"), not from the reported exit code — or use `set -o pipefail` /
`${PIPESTATUS[0]}` / echo `$?` of the unpiped command. When in doubt, read the tail
of the output file the run wrote and look for `error`/`Finished`.

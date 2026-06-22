---
id: app-svelte-nul-bytes
label: App.svelte has NUL bytes (grep -a)
type: gotcha
community: Environment & Build
edges:
  - target: markdown-render-key-guard
    relation: references
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: ai-reports
    relation: caused_by
    confidence: EXTRACTED
    confidence_score: 0.7
---

`src/App.svelte` contains **two literal NUL bytes (`\0`)** — they are the string
separators inside the `markdown` action's `keyOf()` (`… + "\0" + …`, rendered as
`^@` by `cat -A`). Because of them, `file src/App.svelte` reports **`data`**, and
**plain `grep` silently skips the file** ("binary file matches" / no output). This
wasted real time during the AI-reports work: searches for `import`, `marked`,
`markdown` etc. returned nothing even though they're clearly present.

**How to work with it:**
- Use **`grep -a`** (treat as text) — or `Read`, which is unaffected — to search it.
- The NULs are confined to the one `keyOf` line; `tr -cd '\000' < src/App.svelte | wc -c` == 2. Everything else is clean ASCII.
- **Don't "fix" them blindly** and don't let an `Edit` that re-types that line turn
  `"\0"` into a normal space — that would change the cache-key separator. If an
  `Edit` on the `keyOf` line fails to match, it's the NULs: edit a neighbouring line
  instead (the `rich` key addition was skipped for exactly this reason — it's
  constant per node, so omitting it from `keyOf` is harmless).

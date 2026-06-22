---
id: ai-reports
label: AI Reports (kind='report')
type: decision
community: UX & Capture
edges:
  - target: mcp-server
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: markdown-step-model
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: optional-steps
    relation: conceptually_related_to
    confidence: EXTRACTED
    confidence_score: 0.9
  - target: copy-per-code-block
    relation: part_of
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: markdown-render-key-guard
    relation: references
    confidence: EXTRACTED
    confidence_score: 0.8
---

Phase 9 / **D17**: an MCP-connected AI can author a **report** — a project intro,
a "what changed" summary, an analysis — that the user reads *inside* Runebook,
instead of dumping a standalone HTML file.

**A report is a `kind='report'` runbook, not a new entity.** Migration **v8** adds
`runbook.kind TEXT NOT NULL DEFAULT 'runbook'`; a report is a runbook with
`kind='report'` whose single step body holds the whole Markdown document. Created
by `db::create_report` (one INSERT + one `add_step`). This reuses all of runbook
storage, FTS search, and Markdown export — the "shape is emergent" idea of
[[optional-steps]] taken one step further (a 1-step note already renders as a plain
doc). `Runbook.kind` is returned by both `list_runbooks` (so cards badge ✦) and
`get_runbook`. **When changing the `Runbook` struct, remember it's shared with the
MCP binary** — but the struct is only *constructed* in `db.rs` (list/get), and the
MCP server builds JSON by hand, so adding a field was safe (see [[db-rs-shared-with-mcp]]).

**One new MCP tool: `create_report(title, body, tags?, description?)`** — mutating
(hidden under `RUNEBOOK_MCP_READONLY=1`). Its verbose description + the server
`instructions` teach the agent the house format so "make a dev intro" reliably
yields a self-contained Markdown doc using the renderable blocks.

**Rendering is rich but sanitized.** The report body goes through the same
`marked` + per-code-block-copy pipeline as steps ([[copy-per-code-block]]), via the
shared `markdown` action with a new `rich: true` param that adds GitHub-style
callouts (`> [!NOTE]`/`[!TIP]`/`[!IMPORTANT]`/`[!WARNING]`/`[!CAUTION]`) and an auto
table of contents from the headings. **Because a report body is AI/agent-authored,
the parsed HTML is now run through DOMPurify (`clean()`) before it ever reaches
`innerHTML`** — this was added to the shared action, so ordinary steps (also
writable over MCP) are hardened too. This sanitize step is the security boundary the
feature turns on.

**The report view is read-only and copy-only.** Opening a `kind='report'` runbook
shows a dedicated reading layout (✦ badge, title, TOC, rendered body, Copy/Save
`.md`, Delete) — branch `{:else if isReport}` in `App.svelte`'s detail section —
instead of the step/replay/variable chrome. Code blocks keep **copy** but get **no
▶ run** button: an AI-authored doc is read & copied from, never executed wholesale.

**Deferred (documented in D17):** Mermaid diagrams (heavy JS dep — load via dynamic
import later, keep the base bundle light per [[tauri-over-electron]]); live push of a
new report to an open overlay (separate processes — appears on next list load for
now); a sandboxed-iframe "raw AI HTML" escape hatch (rejected as default: big
security surface, ignores design tokens, breaks IPC-routed copy buttons).

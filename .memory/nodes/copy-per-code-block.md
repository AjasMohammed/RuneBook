---
id: copy-per-code-block
label: Copy button per code block
type: concept
community: UX & Capture
edges:
  - target: markdown-step-model
    relation: part_of
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: ipc-boundary
    relation: implements
    confidence: EXTRACTED
    confidence_score: 1.0
---

The replay loop (read → copy → paste → next) survives the move to free-form
markdown by attaching a **copy button to every fenced code block** in the
rendered view, rather than giving commands their own field. Commands live inside
the markdown body as ```` ``` ```` blocks.

Implementation: the frontend renders markdown with **`marked`** (`src/App.svelte`,
a Svelte `use:markdown` action), then injects a `copy` button into each `<pre>`.
The button calls the Rust **`copy_to_clipboard`** IPC command (backed by
`tauri-plugin-clipboard-manager`'s `ClipboardExt` in `lib.rs`) — clipboard access
stays in the core per the [[ipc-boundary]] rule, not `navigator.clipboard`. This
copy capability was brought forward from Phase 3 because it's now core to the
view.

**Inline code also gets a copy button now (2026-06-18).** First real-run feedback
exposed a discoverability gap: the user typed commands as **plain paragraph text**
(e.g. `git status`, not a fenced block), so no `<pre>` was produced and **no copy
button appeared** — the headline feature looked broken. The buttons were never
broken; they only ever attach to fenced blocks. Fix (user picked it from 3
options): the `markdown` action also walks `node.querySelectorAll("code")`,
skips any `code.closest("pre")` (fenced, handled above), and appends a small
`.copy-inline` glyph button (⧉ → ✓) after each inline `<code>`. **▶ run stays
fenced-only** — running mid-sentence code makes no sense. NOTE the real trap for
users: a command must still be *some* kind of code (inline `</>` or fenced `{ }`
in the editor toolbar) — **bare paragraph text gets no copy button**, by design.

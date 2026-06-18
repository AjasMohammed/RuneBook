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

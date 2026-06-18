---
id: markdown-editor-component
label: MarkdownEditor — true WYSIWYG (TipTap)
type: concept
community: UX & Capture
edges:
  - target: markdown-step-model
    relation: implements
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: quick-add-capture
    relation: part_of
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: copy-per-code-block
    relation: conceptually_related_to
    confidence: INFERRED
    confidence_score: 0.7
---

`src/MarkdownEditor.svelte` is the **one notepad** used by both Quick-add and the
Browse step editor (no separate title field). Evolved in two steps on 2026-06-16:

1. First a textarea + side-by-side live **preview** + a markdown **toolbar**.
2. Then the user asked to render formatting **in the typing area itself** and chose
   **true WYSIWYG** (symbols hidden) over live-styled source. So it's now a
   **TipTap v2 (ProseMirror)** contenteditable — you see real bold/headings/code,
   no `**`/`#` — but it **stores plain markdown** via `tiptap-markdown`
   (`editor.storage.markdown.getMarkdown()` to serialize; `setContent(str)` parses
   markdown). Markdown stays the source of truth, so Rust export, FTS5 search, and
   the Browse [[copy-per-code-block]] replay are all untouched. Deps:
   `@tiptap/core`, `@tiptap/starter-kit` (no Link/Placeholder in it), plus
   `@tiptap/extension-link`, `@tiptap/extension-placeholder`, `tiptap-markdown`.
   Bundle jumped ~88KB→~528KB (185KB gz) — fine for a locally-loaded desktop app.

Props: `value` (markdown, two-way), `placeholder`, `grow` (fill Quick-add vs a
150–260px box in edit). Exports `focus()` ([[quick-add-capture]] focuses on
summon). Toolbar = bold/italic/heading(cycles H1–H3)/inline-code/code-block/
bullet/numbered/quote/link, each `editor.chain().focus().toggleX().run()`, with the
active mark/​node highlighted in accent (`editor.isActive`). Link uses a small
inline URL input. Ctrl+B/I come from StarterKit.

**Gotchas baked in (don't regress these):**
- **Selection under `user-select:none`** — app.css sets `html,body{user-select:none}`;
  a textarea was exempt but a contenteditable is **not**, so `.ProseMirror` must
  re-enable `-webkit-user-select:text; user-select:text` or you can't select text.
- **Save shortcut** — `editorProps.handleKeyDown` swallows **Mod+Enter** (else
  StarterKit's HardBreak inserts a line break), `preventDefault`s, and
  `dispatch('submit')`. App wires `on:submit` → `saveAndNext` / `saveEdit(s.id)`,
  and its window keydown now only handles **Escape**. A `saving` flag in App guards
  the async save against a double-fire creating two steps.
- **Escape in the link input** must `stopPropagation()` (not just `preventDefault`)
  or it bubbles to the window handler and hides the whole overlay.
- **Sync loop** — `onUpdate` sets `lastEmitted=md; value=md`; a reactive
  `$: if (editor && value!==lastEmitted) setContent(value,false)` re-syncs only
  *external* changes (draft reset after save, switching steps) — no caret-jump loop.
- **ProseMirror base CSS** isn't injected by TipTap — add `white-space:pre-wrap`,
  `word-wrap:break-word`, ligature-off rules, and a placeholder `::before` on
  `> :first-child.is-editor-empty` (any block, not just `p`).

Display label still = `s.title || deriveLabel(s.body, i)` (first body line, marks
stripped, → `Step N`), which is what killed the old "Untitled" labels; a fresh
auto-created runbook is named from its first note's `deriveLabel`.

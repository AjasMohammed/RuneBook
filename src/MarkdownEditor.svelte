<script>
  // A true WYSIWYG notepad: a contenteditable rich editor (TipTap / ProseMirror)
  // that hides markdown symbols but stores plain **markdown** via tiptap-markdown,
  // so export, FTS search, and the Browse copy-per-code-block replay keep working.
  // Used by Quick-add (capture) and the Browse step editor — one field, no title.
  import { onMount, onDestroy, tick, createEventDispatcher } from "svelte";
  import { Editor } from "@tiptap/core";
  import StarterKit from "@tiptap/starter-kit";
  import Link from "@tiptap/extension-link";
  import Placeholder from "@tiptap/extension-placeholder";
  import { Markdown } from "tiptap-markdown";

  export let value = ""; // markdown (two-way bound)
  export let placeholder = "";
  export let grow = false; // fill the parent (Quick-add) vs. a bounded box (edit)

  const dispatch = createEventDispatcher();

  let element; // contenteditable mount point
  let editor;
  let lastEmitted = value; // last markdown we pushed up — guards the sync loop
  let active = {}; // toolbar active-state snapshot
  let showLink = false;
  let linkUrl = "";

  function refreshActive() {
    if (!editor) return;
    active = {
      bold: editor.isActive("bold"),
      italic: editor.isActive("italic"),
      heading: editor.isActive("heading"),
      code: editor.isActive("code"),
      codeBlock: editor.isActive("codeBlock"),
      bulletList: editor.isActive("bulletList"),
      orderedList: editor.isActive("orderedList"),
      blockquote: editor.isActive("blockquote"),
      link: editor.isActive("link"),
    };
  }

  onMount(() => {
    editor = new Editor({
      element,
      extensions: [
        StarterKit,
        Link.configure({ openOnClick: false, autolink: true }),
        Placeholder.configure({ placeholder: () => placeholder }),
        Markdown.configure({
          html: false,
          breaks: true,
          transformPastedText: true,
          bulletListMarker: "-",
        }),
      ],
      content: value || "",
      editorProps: {
        handleKeyDown: (_view, event) => {
          // Cmd/Ctrl+Enter = save & next. Swallow it here so StarterKit's
          // HardBreak (bound to Mod-Enter) doesn't insert a line break first,
          // and let the parent persist via on:submit.
          if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
            event.preventDefault();
            dispatch("submit");
            return true;
          }
          return false;
        },
      },
      onUpdate: ({ editor }) => {
        const md = editor.storage.markdown.getMarkdown();
        lastEmitted = md;
        value = md;
        refreshActive();
      },
      onSelectionUpdate: refreshActive,
    });
    refreshActive();
  });

  onDestroy(() => editor?.destroy());

  // External value changes (draft reset after save, switching steps) → re-sync
  // the editor. Our own keystrokes set value===lastEmitted, so this won't fire
  // on them (no caret-jump loop). setContent parses the string as markdown.
  $: if (editor && value !== lastEmitted) {
    lastEmitted = value;
    editor.commands.setContent(value || "", false);
  }

  export function focus() {
    editor?.commands.focus("end");
  }

  // ── Toolbar ──────────────────────────────────────────────────────
  $: onMap = {
    bold: active.bold,
    italic: active.italic,
    heading: active.heading,
    code: active.code,
    codeBlock: active.codeBlock,
    bulletList: active.bulletList,
    orderedList: active.orderedList,
    blockquote: active.blockquote,
    link: active.link,
  };

  const TOOLS = [
    { key: "bold", label: "B", title: "Bold  (Ctrl+B)", cls: "b" },
    { key: "italic", label: "I", title: "Italic  (Ctrl+I)", cls: "i" },
    { key: "heading", label: "H", title: "Heading (cycles H1–H3)", cls: "h" },
    { key: "code", label: "</>", title: "Inline code", cls: "mono" },
    { key: "codeBlock", label: "{ }", title: "Code block", cls: "mono" },
    { key: "bulletList", label: "•", title: "Bullet list", cls: "" },
    { key: "orderedList", label: "1.", title: "Numbered list", cls: "mono" },
    { key: "blockquote", label: "❝", title: "Quote", cls: "" },
    { key: "link", label: "↗", title: "Link", cls: "" },
  ];

  // Heading cycles none → H1 → H2 → H3 → none on the current block.
  function cycleHeading() {
    const c = editor.chain().focus();
    if (editor.isActive("heading", { level: 1 })) c.toggleHeading({ level: 2 }).run();
    else if (editor.isActive("heading", { level: 2 })) c.toggleHeading({ level: 3 }).run();
    else if (editor.isActive("heading", { level: 3 })) c.setParagraph().run();
    else c.toggleHeading({ level: 1 }).run();
  }

  function runTool(key) {
    if (!editor) return;
    const c = editor.chain().focus();
    switch (key) {
      case "bold": c.toggleBold().run(); break;
      case "italic": c.toggleItalic().run(); break;
      case "heading": cycleHeading(); break;
      case "code": c.toggleCode().run(); break;
      case "codeBlock": c.toggleCodeBlock().run(); break;
      case "bulletList": c.toggleBulletList().run(); break;
      case "orderedList": c.toggleOrderedList().run(); break;
      case "blockquote": c.toggleBlockquote().run(); break;
      case "link": openLink(); break;
    }
  }

  function openLink() {
    if (editor.isActive("link")) {
      editor.chain().focus().unsetLink().run();
      return;
    }
    linkUrl = "";
    showLink = true;
    tick().then(() => document.getElementById("md-link-url")?.focus());
  }

  function applyLink() {
    const url = linkUrl.trim();
    showLink = false;
    if (!url) {
      editor.commands.focus();
      return;
    }
    const { from, to } = editor.state.selection;
    if (from === to) {
      // No selection: insert the URL as its own linked text.
      editor
        .chain()
        .focus()
        .insertContent(url)
        .setTextSelection({ from, to: from + url.length })
        .setLink({ href: url })
        .run();
    } else {
      editor.chain().focus().extendMarkRange("link").setLink({ href: url }).run();
    }
  }
</script>

<div class="md-editor" class:grow>
  <div class="md-toolbar">
    {#each TOOLS as t}
      <button
        type="button"
        class="md-tool {t.cls}"
        class:on={onMap[t.key]}
        title={t.title}
        on:click={() => runTool(t.key)}
      >
        {t.label}
      </button>
    {/each}
  </div>

  {#if showLink}
    <form class="md-link" on:submit|preventDefault={applyLink}>
      <input
        id="md-link-url"
        placeholder="https://…"
        bind:value={linkUrl}
        on:keydown={(e) => {
          if (e.key === "Escape") {
            // Don't let Escape bubble to App's window handler (which would hide
            // the overlay / cancel the edit) — just close this link input.
            e.preventDefault();
            e.stopPropagation();
            showLink = false;
            editor.commands.focus();
          }
        }}
      />
      <button type="submit">Add link</button>
      <button type="button" class="ghost" on:click={() => (showLink = false)}>Cancel</button>
    </form>
  {/if}

  <div class="md-prose" bind:this={element}></div>
</div>

<style>
  .md-editor {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .md-editor.grow {
    flex: 1;
    min-height: 0;
  }

  /* Toolbar */
  .md-toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
    align-items: center;
  }
  .md-tool {
    min-width: 28px;
    padding: 4px 7px;
    font-size: 12px;
    line-height: 1;
    color: var(--muted);
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 7px;
  }
  .md-tool:hover {
    color: var(--fg);
    background: rgba(255, 255, 255, 0.12);
  }
  .md-tool.b {
    font-weight: 700;
  }
  .md-tool.i {
    font-style: italic;
    font-family: Georgia, "Times New Roman", serif;
  }
  .md-tool.h {
    font-weight: 700;
  }
  .md-tool.mono {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 11px;
  }
  /* Active formatting at the caret is highlighted in the accent. */
  .md-tool.on {
    color: var(--accent);
    border-color: rgba(232, 93, 4, 0.5);
    background: rgba(232, 93, 4, 0.12);
  }

  /* Inline link-URL entry */
  .md-link {
    display: flex;
    gap: 6px;
  }
  .md-link input {
    flex: 1;
  }
  .md-link button {
    flex: none;
  }

  /* The editable surface — looks like the old textarea field. */
  .md-prose {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    overflow-y: auto;
  }
  .md-prose:focus-within {
    border-color: var(--accent);
  }
  .md-editor:not(.grow) .md-prose {
    min-height: 150px;
    max-height: 260px;
  }
  .md-editor.grow .md-prose {
    flex: 1;
    min-height: 0;
  }

  /* ProseMirror content: WYSIWYG, but selectable despite the global
     user-select:none, and styled to mirror the Browse `.rendered` look. */
  .md-prose :global(.ProseMirror) {
    outline: none;
    min-height: 100%;
    box-sizing: border-box;
    padding: 9px 11px;
    color: var(--fg);
    font-size: 13px;
    line-height: 1.55;
    -webkit-user-select: text;
    user-select: text;
    cursor: text;
    /* Essential ProseMirror base (TipTap doesn't inject prosemirror.css). */
    position: relative;
    white-space: pre-wrap;
    word-wrap: break-word;
    -webkit-font-variant-ligatures: none;
    font-variant-ligatures: none;
    font-feature-settings: "liga" 0;
  }
  .md-prose :global(.ProseMirror) > :global(:first-child) {
    margin-top: 0;
  }
  .md-prose :global(.ProseMirror p),
  .md-prose :global(.ProseMirror ul),
  .md-prose :global(.ProseMirror ol) {
    margin: 6px 0;
  }
  .md-prose :global(.ProseMirror h1),
  .md-prose :global(.ProseMirror h2),
  .md-prose :global(.ProseMirror h3) {
    font-size: 15px;
    font-weight: 700;
    margin: 12px 0 4px;
  }
  .md-prose :global(.ProseMirror h1) {
    font-size: 17px;
  }
  .md-prose :global(.ProseMirror a) {
    color: var(--accent);
    cursor: pointer;
  }
  .md-prose :global(.ProseMirror blockquote) {
    margin: 6px 0;
    padding-left: 10px;
    border-left: 2px solid rgba(255, 255, 255, 0.15);
    color: var(--muted);
  }
  .md-prose :global(.ProseMirror code) {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 0.9em;
    background: rgba(255, 255, 255, 0.09);
    padding: 1px 5px;
    border-radius: 4px;
  }
  .md-prose :global(.ProseMirror pre) {
    margin: 6px 0;
    padding: 9px 11px;
    background: rgba(0, 0, 0, 0.38);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 8px;
    overflow-x: auto;
  }
  .md-prose :global(.ProseMirror pre code) {
    background: none;
    padding: 0;
    font-size: 12px;
  }
  /* Empty-state placeholder (tiptap Placeholder extension). Match any first
     block type, not just <p>, so it still shows if the first line is a heading. */
  .md-prose :global(.ProseMirror > :first-child.is-editor-empty::before) {
    content: attr(data-placeholder);
    color: var(--muted);
    float: left;
    height: 0;
    pointer-events: none;
  }
</style>

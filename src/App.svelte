<script>
  import { onMount, onDestroy, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen } from "@tauri-apps/api/event";
  import { save, open } from "@tauri-apps/plugin-dialog";
  import { marked } from "marked";
  import MarkdownEditor from "./MarkdownEditor.svelte";

  const appWindow = getCurrentWindow();
  const CURRENT_RB_KEY = "current_runbook";
  const ACCENT_KEY = "accent";
  const ALLOW_RUN_KEY = "allow_run";
  const GIT_DIR_KEY = "git_sync_dir";

  // Accent presets (orange is the default — "orange strictly as an accent").
  const ACCENTS = [
    { name: "Ember", value: "#e85d04" },
    { name: "Gold", value: "#e0a106" },
    { name: "Teal", value: "#16a085" },
    { name: "Violet", value: "#8b5cf6" },
    { name: "Rose", value: "#e0436b" },
  ];

  // Three modes in one overlay window (docs D5): Quick-add (capture, the hotkey
  // landing), Browse (replay), Settings. A step is an optional title + markdown body.

  let mode = "quick"; // "quick" | "browse" | "settings"

  // Switch top-level mode and drop any transient banners that belonged to the
  // mode we're leaving (a stale error / flash shouldn't follow the user across).
  function setMode(m) {
    error = "";
    clearFlash();
    mode = m;
  }

  // Settings state.
  let hotkey = "Control+Alt+Space";
  let hotkeyInput = hotkey;
  let autostart = false;
  let accent = ACCENTS[0].value;
  let allowRun = false; // execution gate for the Run buttons (off by default)

  let runbooks = [];
  let error = "";

  // ── Quick-add state ──────────────────────────────────────────────
  let currentRunbookId = null; // persisted target for new steps
  let draft = { body: "" }; // single markdown notepad — no separate title (D8)
  let newRbName = "";
  let composer; // MarkdownEditor instance, focused on summon
  let flash = ""; // "saved" confirmation
  let flashScope = "quick"; // which mode the flash belongs to — so a Browse "Copied ✓" can't paint in the Quick-add footer
  let addedThisSession = 0;
  let flashTimer;
  let unlistenShow;
  let saving = false; // guards against a double save (rapid ⌘↵) creating two steps
  // Custom runbook picker. A native <select> can't be styled in WebKitGTK (its
  // popup uses the GTK theme and ignores option CSS), so we render our own.
  let pickerOpen = false;
  let pickerEl;

  // ── Browse state ─────────────────────────────────────────────────
  let selected = null;
  let search = "";
  let newRunbookTitle = "";
  let editingId = null;
  let editBuffer = { body: "" };
  let activeTag = null; // sidebar tag filter
  let newTag = ""; // tag-editor input
  let editingTitle = false; // inline rename of the open runbook
  let titleBuffer = "";
  let titleInput;
  // Steps are optional: a runbook can be one plain note or a numbered list. The
  // composer below grows a note into multiple steps (and writes the first one).
  let addingStep = false;
  let addBuffer = { body: "" };
  let stepComposer;

  // Replay mode: work a multi-step runbook as a checklist (D10). Step `done`
  // flags persist, so closing the overlay and reopening resumes where you left
  // off; the bar shows progress and a reset.
  let replayMode = false;
  let progressMap = {}; // runbookId -> { done, total } for the in-progress badge

  // Variable profiles (D12): named saved value sets per runbook for the {{vars}}
  // below, plus a per-variable "secret" mark — a secret var is masked and never
  // written into a profile. Only the secret *names* persist (in the setting kv),
  // never their values, so a key/password is retyped each session.
  let varProfiles = [];
  let activeProfile = null;
  let newProfileName = "";
  let secretVars = new Set();

  // ── Command palette (D14) — Ctrl/Cmd+K quick switcher ────────────
  let paletteOpen = false;
  let paletteQuery = "";
  let paletteIndex = 0;
  let paletteInput;
  let paletteCard; // the modal card; clicks outside it close the palette

  // ── Git sync (D16) ───────────────────────────────────────────────
  let gitDir = "";
  let gitStatus = "";

  marked.setOptions({ breaks: true, gfm: true });

  async function run(fn) {
    try {
      error = "";
      return await fn();
    } catch (e) {
      error = String(e);
    }
  }

  async function loadRunbooks(query) {
    runbooks = (await run(() => invoke("list_runbooks", { query: query || null }))) ?? [];
    await loadProgress();
  }

  // Replay progress for the card badges — only runbooks with a step checked off.
  let progressSeq = 0;
  async function loadProgress() {
    // Tag each request; if a newer one started before this resolved, drop the
    // stale response so rapid step-toggling can't paint an out-of-order badge.
    const seq = ++progressSeq;
    const list = (await run(() => invoke("list_progress"))) ?? [];
    if (seq !== progressSeq) return;
    const map = {};
    for (const p of list) map[p.runbookId] = { done: p.done, total: p.total };
    progressMap = map;
  }

  $: currentRunbook = runbooks.find((r) => r.id === currentRunbookId) ?? null;

  // All distinct tags (for the sidebar filter) and the tag-filtered list.
  $: allTags = [...new Set(runbooks.flatMap((r) => r.tags))].sort();
  $: displayRunbooks = activeTag ? runbooks.filter((r) => r.tags.includes(activeTag)) : runbooks;

  // ── Variables / placeholders ─────────────────────────────────────
  // Steps can contain {{name}} placeholders (e.g. `ssh deploy@{{host}}`). The
  // distinct names across the open runbook get fill-in fields; values are
  // substituted live in the rendered markdown and in copied commands. Values
  // are kept in memory only (never persisted — they may be secrets).
  const VAR_RE = /\{\{\s*([\w.-]+)\s*\}\}/g;
  let varValues = {};

  $: varNames = selected ? distinctVars(selected.steps) : [];

  function distinctVars(steps) {
    const names = new Set();
    for (const s of steps) for (const m of s.body.matchAll(VAR_RE)) names.add(m[1]);
    return [...names];
  }

  // Replace filled placeholders; leave unfilled ones literal so they're visible.
  // Used for display and for the Copy button (human-facing text).
  function fillVars(text, vars) {
    return text.replace(VAR_RE, (whole, name) => {
      const v = vars?.[name];
      return v != null && v !== "" ? v : whole;
    });
  }

  // Single-quote a value for POSIX sh, so it's treated as literal data.
  function shellQuote(v) {
    return "'" + String(v).replace(/'/g, "'\\''") + "'";
  }

  // Like fillVars, but shell-escapes each substituted value. Used ONLY for the
  // Run path (`run_command`), so a variable value containing shell metacharacters
  // (`; rm -rf ~`, `$(...)`, backticks) is data, not injected code — even though
  // the command template itself is already the user's own (D11 trust model).
  // Copy/display keep raw substitution (nicer to read; still the user's choice).
  function fillVarsShell(text, vars) {
    return text.replace(VAR_RE, (whole, name) => {
      const v = vars?.[name];
      return v != null && v !== "" ? shellQuote(v) : whole;
    });
  }

  // Extract the raw (unfilled) source of each fenced code block, in document
  // order, by parsing the markdown body. Lets the Run path re-substitute with
  // shell-escaped values instead of reusing the already-filled rendered text.
  function rawCodeBlocks(body) {
    try {
      const doc = new DOMParser().parseFromString(marked.parse(body ?? ""), "text/html");
      return [...doc.querySelectorAll("pre")].map((p) => (p.querySelector("code") ?? p).textContent);
    } catch {
      return [];
    }
  }

  // Derive a one-line display label for a step from its markdown body — the
  // first non-empty line, stripped of markdown marks. Falls back to "Step N".
  // (There is no separate title field anymore; the body is the whole note.)
  function deriveLabel(body, index) {
    const firstLine = (body ?? "")
      .split("\n")
      .map((l) => l.trim())
      .find((l) => l.length > 0);
    const clean = (firstLine ?? "")
      .replace(/^#{1,6}\s+/, "") // heading marks
      .replace(/^[-*+]\s+/, "") // bullet
      .replace(/^\d+\.\s+/, "") // numbered
      .replace(/^>\s+/, "") // quote
      .replace(/[*_`~]/g, "") // emphasis / code marks
      .trim();
    if (!clean) return `Step ${index + 1}`;
    return clean.length > 60 ? clean.slice(0, 60) + "…" : clean;
  }

  // ── Replay progress (D10) ────────────────────────────────────────
  $: doneCount = selected ? selected.steps.filter((s) => s.done).length : 0;
  $: totalSteps = selected ? selected.steps.length : 0;

  // ── Command palette (D14) ────────────────────────────────────────
  // Filter the already-loaded list by title/tag substring (instant, no IPC);
  // cap to keep the modal short. Deeper step-body search lives in Browse.
  $: paletteResults = paletteOpen ? filterPalette(runbooks, paletteQuery) : [];

  function filterPalette(list, q) {
    const query = q.trim().toLowerCase();
    const items = !query
      ? list
      : list.filter(
          (r) =>
            r.title.toLowerCase().includes(query) ||
            r.tags.some((t) => t.toLowerCase().includes(query))
        );
    return items.slice(0, 50);
  }

  // ── Mode + focus ─────────────────────────────────────────────────
  async function focusComposer() {
    await tick();
    composer?.focus();
  }

  async function goQuick() {
    setMode("quick");
    await focusComposer();
  }

  // ── Current runbook (persisted) ──────────────────────────────────
  async function setCurrentRunbook(id) {
    currentRunbookId = id;
    // Persist the choice; "" clears it so "— new runbook —" sticks across
    // sessions (onMount treats an empty/unknown id as "none selected").
    await run(() => invoke("set_setting", { key: CURRENT_RB_KEY, value: id == null ? "" : String(id) }));
  }

  function chooseRunbook(id) {
    pickerOpen = false;
    setCurrentRunbook(id);
  }

  // Close the picker on any click outside it (the trigger lives inside pickerEl,
  // so its own click toggles rather than immediately re-closing).
  function onDocClick(e) {
    if (pickerOpen && pickerEl && !pickerEl.contains(e.target)) pickerOpen = false;
    // Click outside the palette card closes it (the palette opens via Ctrl/Cmd+K,
    // never a click, so there's no open-then-immediately-close race).
    if (paletteOpen && paletteCard && !paletteCard.contains(e.target)) closePalette();
  }

  async function createAndSelect() {
    const title = newRbName.trim();
    if (!title) return;
    const id = await run(() => invoke("create_runbook", { title }));
    newRbName = "";
    await loadRunbooks();
    if (id != null) await setCurrentRunbook(id);
    await focusComposer();
  }

  // ── Quick-add: save & next ───────────────────────────────────────
  async function saveAndNext() {
    if (saving || !draft.body.trim()) return;
    saving = true;
    try {
      // Resolve the target runbook. If none is set, name a fresh one after this
      // first note (its derived label) rather than a generic "Untitled runbook".
      let target = currentRunbookId;
      if (target == null) {
        const title = deriveLabel(draft.body, 0).slice(0, 50) || "Untitled runbook";
        target = await run(() => invoke("create_runbook", { title }));
        if (target == null) return;
        await setCurrentRunbook(target);
      }

      const id = await run(() =>
        invoke("add_step", { runbookId: target, step: { title: "", body: draft.body } })
      );
      if (id == null) return;

      draft = { body: "" };
      addedThisSession += 1;
      showFlash("Saved ✓");
      await loadRunbooks();
      await focusComposer();
    } finally {
      saving = false;
    }
  }

  function showFlash(msg) {
    flash = msg;
    flashScope = mode; // tie this confirmation to the mode it fired in
    clearTimeout(flashTimer);
    flashTimer = setTimeout(() => (flash = ""), 1400);
  }

  function clearFlash() {
    flash = "";
    clearTimeout(flashTimer);
  }

  // ── Browse: runbook + step CRUD ──────────────────────────────────
  async function openRunbook(id) {
    editingId = null;
    editingTitle = false;
    addingStep = false;
    addBuffer = { body: "" };
    varValues = {}; // variable values are per-runbook and ephemeral (never persisted)
    replayMode = false; // start in normal view; user clicks Replay to begin a run
    activeProfile = null;
    newProfileName = "";
    selected = await run(() => invoke("get_runbook", { id }));
    if (selected) {
      // Load this runbook's saved profiles and which vars are marked secret.
      varProfiles = (await run(() => invoke("list_var_profiles", { runbookId: id }))) ?? [];
      const sv = await run(() => invoke("get_setting", { key: `secret_vars:${id}` }));
      secretVars = new Set(parseSecretNames(sv));
    }
  }

  // Tolerate a malformed secret_vars setting (we always write valid JSON, but a
  // hand-edited DB shouldn't throw out of openRunbook).
  function parseSecretNames(sv) {
    if (!sv) return [];
    try {
      const arr = JSON.parse(sv);
      return Array.isArray(arr) ? arr : [];
    } catch {
      return [];
    }
  }

  // Re-fetch the open runbook after an in-runbook mutation (edit/add/delete/
  // reorder/tag/rename) WITHOUT the full reset openRunbook does — so replay
  // mode, typed variable values, and the active profile survive the edit.
  async function refreshSelected() {
    if (!selected) return;
    selected = await run(() => invoke("get_runbook", { id: selected.id }));
    await loadProgress();
  }

  async function createRunbook() {
    const title = newRunbookTitle.trim();
    if (!title) return;
    const id = await run(() => invoke("create_runbook", { title }));
    newRunbookTitle = "";
    await loadRunbooks(search);
    if (id != null) {
      // A fresh runbook has no steps — open it and drop the cursor straight in
      // the note composer so the user can write a simple note immediately.
      await openRunbook(id);
      await tick();
      stepComposer?.focus();
    }
  }

  async function deleteRunbook(id) {
    await run(() => invoke("delete_runbook", { id }));
    if (selected?.id === id) selected = null;
    if (currentRunbookId === id) currentRunbookId = null;
    await loadRunbooks(search);
  }

  function startEdit(step) {
    editingId = step.id;
    // Single-notepad model: edit the body only. Any legacy `title` is left
    // untouched (preserved via COALESCE on save) and still shows as the step's
    // label, so nothing is lost and Markdown export stays consistent.
    editBuffer = { body: step.body };
  }

  async function saveEdit(id) {
    if (saving) return;
    saving = true;
    try {
      // Send only `body`; omitting `title` makes the patch's title NULL, so the
      // Rust COALESCE keeps any existing title rather than overwriting it.
      await run(() => invoke("update_step", { id, patch: { body: editBuffer.body } }));
      editingId = null;
      if (selected) await refreshSelected();
    } finally {
      saving = false;
    }
  }

  async function deleteStep(id) {
    await run(() => invoke("delete_step", { id }));
    if (selected) await refreshSelected();
  }

  // ── Browse: rename the open runbook ──────────────────────────────
  async function startEditTitle() {
    if (!selected) return;
    titleBuffer = selected.title;
    editingTitle = true;
    await tick();
    titleInput?.focus();
    titleInput?.select();
  }

  // Commit on Enter (form submit) or blur. The `editingTitle` guard makes the
  // two paths idempotent: Enter closes the field, the resulting blur is a no-op.
  async function commitTitle() {
    if (!editingTitle) return;
    editingTitle = false;
    const title = titleBuffer.trim();
    if (!selected || !title || title === selected.title) return;
    await run(() => invoke("update_runbook", { id: selected.id, patch: { title } }));
    await refreshSelected();
    await loadRunbooks(search);
  }

  function cancelTitle() {
    editingTitle = false;
  }

  // ── Browse: add a step (grow a note into multiple steps) ─────────
  async function startAddStep() {
    addingStep = true;
    addBuffer = { body: "" };
    await tick();
    stepComposer?.focus();
  }

  function cancelAddStep() {
    addingStep = false;
    addBuffer = { body: "" };
  }

  async function saveAddStep() {
    if (saving || !selected || !addBuffer.body.trim()) return;
    saving = true;
    try {
      await run(() =>
        invoke("add_step", { runbookId: selected.id, step: { title: "", body: addBuffer.body } })
      );
      addingStep = false;
      addBuffer = { body: "" };
      await refreshSelected();
      await loadRunbooks(search);
    } finally {
      saving = false;
    }
  }

  // ── Tags ─────────────────────────────────────────────────────────
  async function setTags(tags) {
    if (!selected) return;
    await run(() => invoke("update_runbook", { id: selected.id, patch: { tags } }));
    await refreshSelected();
    await loadRunbooks(search);
  }

  async function addTag() {
    const t = newTag.trim().replace(/^#/, "");
    if (!t || selected.tags.includes(t)) {
      newTag = "";
      return;
    }
    newTag = "";
    await setTags([...selected.tags, t]);
  }

  async function removeTag(t) {
    await setTags(selected.tags.filter((x) => x !== t));
    if (activeTag === t && !runbooks.some((r) => r.tags.includes(t))) activeTag = null;
  }

  function toggleTagFilter(t) {
    activeTag = activeTag === t ? null : t;
  }

  // ── Export ───────────────────────────────────────────────────────
  async function exportMarkdown() {
    if (!selected) return null;
    return await run(() => invoke("export_markdown", { runbookId: selected.id }));
  }

  async function copyMarkdown() {
    const md = await exportMarkdown();
    if (md == null) return;
    await run(() => invoke("copy_to_clipboard", { text: md }));
    showFlash("Copied markdown ✓");
  }

  async function saveMarkdown() {
    const md = await exportMarkdown();
    if (md == null) return;
    const path = await run(() =>
      save({
        defaultPath: `${selected.title || "runbook"}.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      })
    );
    if (!path) return; // user cancelled
    await run(() => invoke("save_text_file", { path, contents: md }));
    showFlash("Saved .md ✓");
  }

  // ── Settings ─────────────────────────────────────────────────────
  // Convert a #rrggbb accent to an rgba() string at the given alpha, so the
  // tinted backgrounds/borders can follow the runtime accent (a literal orange
  // rgba would stay orange after the user switches accent to e.g. Teal).
  function accentRgba(hex, alpha) {
    const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
    if (!m) return hex;
    const n = parseInt(m[1], 16);
    return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`;
  }

  function applyAccent(value) {
    accent = value;
    const root = document.documentElement.style;
    root.setProperty("--accent", value);
    root.setProperty("--accent-soft", accentRgba(value, 0.14));
    root.setProperty("--accent-line", accentRgba(value, 0.5));
  }

  async function chooseAccent(value) {
    applyAccent(value);
    await run(() => invoke("set_setting", { key: ACCENT_KEY, value }));
  }

  async function applyHotkey() {
    const spec = hotkeyInput.trim();
    if (!spec) return;
    await run(() => invoke("set_hotkey", { spec }));
    hotkey = spec;
    showFlash("Hotkey updated ✓");
  }

  async function toggleAutostart(e) {
    const enabled = e.target.checked;
    await run(() => invoke("set_autostart", { enabled }));
    autostart = enabled;
  }

  async function toggleAllowRun(e) {
    const enabled = e.target.checked;
    await run(() => invoke("set_setting", { key: ALLOW_RUN_KEY, value: enabled ? "1" : "0" }));
    allowRun = enabled;
  }

  async function backupDatabase() {
    const path = await run(() =>
      save({
        defaultPath: "runebook-backup.db",
        filters: [{ name: "SQLite database", extensions: ["db"] }],
      })
    );
    if (!path) return;
    await run(() => invoke("backup_database", { path }));
    showFlash("Backup saved ✓");
  }

  async function restoreDatabase() {
    const path = await run(() =>
      open({
        multiple: false,
        filters: [{ name: "SQLite database", extensions: ["db"] }],
      })
    );
    if (!path) return;
    await run(() => invoke("restore_database", { path }));
    // The DB contents were replaced — reset and reload everything.
    selected = null;
    activeTag = null;
    currentRunbookId = null;
    await loadRunbooks();
    showFlash("Restored ✓");
  }

  async function moveStep(index, dir) {
    if (!selected) return;
    const target = index + dir;
    if (target < 0 || target >= selected.steps.length) return;
    const ids = selected.steps.map((s) => s.id);
    [ids[index], ids[target]] = [ids[target], ids[index]];
    await run(() => invoke("reorder_steps", { runbookId: selected.id, orderedIds: ids }));
    await refreshSelected();
  }

  // ── Replay mode (D10) ────────────────────────────────────────────
  function toggleReplay() {
    replayMode = !replayMode;
  }

  async function setStepDone(s, done) {
    await run(() => invoke("set_step_done", { stepId: s.id, done }));
    // Reassign the array so the progress bar / done styling re-derive.
    selected.steps = selected.steps.map((x) => (x.id === s.id ? { ...x, done } : x));
    selected = selected;
    await loadProgress();
  }

  async function resetProgress() {
    if (!selected) return;
    await run(() => invoke("reset_progress", { runbookId: selected.id }));
    await refreshSelected(); // refresh `done` flags without leaving replay mode
  }

  // Set an input's type imperatively. Svelte 4 forbids a dynamic `type`
  // attribute on an input that also uses bind:value, so secret-masking toggles
  // the property here instead (bind:value works the same for text/password).
  function inputType(node, secret) {
    node.type = secret ? "password" : "text";
    return {
      update(s) {
        node.type = s ? "password" : "text";
      },
    };
  }

  // ── Variable profiles (D12) ──────────────────────────────────────
  async function applyProfile(name) {
    if (!selected) return;
    const vals = await run(() => invoke("get_var_profile", { runbookId: selected.id, name }));
    // Overlay the saved values, but only for variables the runbook still uses —
    // so a profile saved before a step was edited can't leave stale values in
    // memory. Existing typed values for other current vars are kept.
    if (vals) {
      const merged = { ...varValues };
      for (const n of varNames) if (vals[n] != null) merged[n] = vals[n];
      varValues = merged;
    }
    activeProfile = name;
  }

  async function saveProfile() {
    const name = newProfileName.trim();
    if (!selected || !name) return;
    // Exclude secret-marked vars so a key/password never reaches the database.
    const values = {};
    for (const n of varNames) {
      if (secretVars.has(n)) continue;
      const v = varValues[n];
      if (v != null && v !== "") values[n] = v;
    }
    await run(() => invoke("save_var_profile", { runbookId: selected.id, name, values }));
    newProfileName = "";
    varProfiles = (await run(() => invoke("list_var_profiles", { runbookId: selected.id }))) ?? [];
    activeProfile = name;
    showFlash("Profile saved ✓");
  }

  async function deleteProfile(name) {
    if (!selected) return;
    await run(() => invoke("delete_var_profile", { runbookId: selected.id, name }));
    if (activeProfile === name) activeProfile = null;
    varProfiles = (await run(() => invoke("list_var_profiles", { runbookId: selected.id }))) ?? [];
  }

  async function toggleSecret(name) {
    if (!selected) return;
    const next = new Set(secretVars);
    if (next.has(name)) next.delete(name);
    else next.add(name);
    secretVars = next;
    await run(() =>
      invoke("set_setting", { key: `secret_vars:${selected.id}`, value: JSON.stringify([...next]) })
    );
  }

  // ── Project pinning (D15) ────────────────────────────────────────
  async function pinFolder() {
    if (!selected) return;
    const path = await run(() => open({ directory: true, multiple: false }));
    if (!path) return;
    await run(() => invoke("update_runbook", { id: selected.id, patch: { projectDir: path } }));
    await refreshSelected();
  }

  async function unpinFolder() {
    if (!selected) return;
    await run(() => invoke("update_runbook", { id: selected.id, patch: { projectDir: "" } }));
    await refreshSelected();
  }

  // ── Command palette (D14) ────────────────────────────────────────
  let paletteReturnFocus = null; // element to restore focus to when the modal closes
  async function openPalette() {
    paletteReturnFocus = document.activeElement;
    pickerOpen = false; // only one transient layer at a time
    paletteQuery = "";
    paletteIndex = 0;
    paletteOpen = true;
    await tick();
    paletteInput?.focus();
  }

  function closePalette() {
    paletteOpen = false;
    // Restore focus to whatever had it before the modal opened (don't strand it
    // on <body>). palettePick sets its own focus afterward, so skip there.
    paletteReturnFocus?.focus?.();
    paletteReturnFocus = null;
  }

  async function palettePick(r) {
    paletteOpen = false;
    paletteReturnFocus = null;
    setMode("browse");
    await openRunbook(r.id);
  }

  function paletteKeydown(e) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      // max(len-1, 0) keeps the index at 0 (never -1) when results are empty.
      paletteIndex = Math.min(paletteIndex + 1, Math.max(paletteResults.length - 1, 0));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      paletteIndex = Math.max(paletteIndex - 1, 0);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const r = paletteResults[paletteIndex];
      if (r) palettePick(r);
    } else if (e.key === "Escape") {
      // Close the palette without bubbling to the window handler (which hides
      // the overlay).
      e.preventDefault();
      e.stopPropagation();
      closePalette();
    } else if (e.key === "Tab") {
      // Trap focus: the modal's only real focus stop is its input (results are
      // arrow-navigated), so keep Tab from moving focus to controls behind the
      // scrim.
      e.preventDefault();
    }
  }

  // ── Git sync (D16) ───────────────────────────────────────────────
  async function chooseGitDir() {
    const path = await run(() => open({ directory: true, multiple: false }));
    if (!path) return;
    gitDir = path;
    await run(() => invoke("set_setting", { key: GIT_DIR_KEY, value: path }));
  }

  async function gitSyncNow(push) {
    if (!gitDir) return;
    gitStatus = push ? "Syncing & pushing…" : "Syncing…";
    const res = await run(() => invoke("git_sync", { dir: gitDir, push }));
    // run() returns undefined on error (the banner shows the reason) — don't
    // silently blank the status; make the failure visible here too.
    gitStatus = res ?? "Sync failed — see the error above.";
  }

  // Inline SVG icons for the inline-code copy affordance. A literal glyph (⧉ /
  // ✓) can render as a tofu box in WebKitGTK's system font; an SVG with
  // currentColor is reliable and follows the accent on hover.
  const COPY_ICON =
    '<svg viewBox="0 0 16 16" width="11" height="11" aria-hidden="true" focusable="false">' +
    '<rect x="5.5" y="5.5" width="8.5" height="8.5" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.3"/>' +
    '<path d="M10.5 5.5V3.5A1.5 1.5 0 0 0 9 2H3.5A1.5 1.5 0 0 0 2 3.5V9A1.5 1.5 0 0 0 3.5 10.5H5.5" fill="none" stroke="currentColor" stroke-width="1.3"/>' +
    "</svg>";
  const CHECK_ICON =
    '<svg viewBox="0 0 16 16" width="11" height="11" aria-hidden="true" focusable="false">' +
    '<path d="M3 8.5l3.2 3.2L13 5" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/>' +
    "</svg>";

  // Render markdown into the node (with {{vars}} filled) and wire a copy button
  // — and, when execution is enabled, a Run button — onto every code block.
  // Re-runs when the body, variable values, or the run gate change.
  function markdown(node, param) {
    let current = param;
    let lastKey = null;

    // A cheap signature of the *visible* output. Re-render only when this changes,
    // so typing into a variable field that this step doesn't reference (or any var
    // edit on a step with no {{placeholders}}) no longer re-parses the markdown and
    // destroys the run-output panel on every keystroke.
    function keyOf(p) {
      return (
        (p.body ?? "") + " " + (p.allowRun ? "1" : "0") + " " + fillVars(p.body ?? "", p.vars)
      );
    }

    function render() {
      const { body, vars } = current;
      lastKey = keyOf(current);
      node.innerHTML = marked.parse(fillVars(body ?? "", vars));
      // Raw (unfilled) source of each code block, aligned by document order with
      // the rendered <pre>s — so the Run path can substitute shell-escaped values.
      const raw = rawCodeBlocks(body);
      node.querySelectorAll("pre").forEach((pre, i) => {
        const code = pre.querySelector("code") ?? pre;
        const text = code.textContent; // filled values — human-facing (copy/display)
        // Lay Run + Copy out in one top-right flex group so their labels can't
        // collide (they used to be two hard-coded `right` offsets that overlapped
        // once a label like "running…" grew past the gap).
        const tools = document.createElement("div");
        tools.className = "code-tools";
        if (current.allowRun) {
          // Execute with shell-escaped variable values (fall back to the filled
          // text if raw extraction didn't line up).
          const cmd = raw[i] != null ? fillVarsShell(raw[i], vars) : text;
          const runBtn = document.createElement("button");
          runBtn.type = "button";
          runBtn.className = "run";
          runBtn.textContent = "▶ run";
          runBtn.title = "Run this command in your shell";
          runBtn.setAttribute("aria-label", "Run command");
          runBtn.addEventListener("click", () => execBlock(pre, cmd, runBtn));
          tools.appendChild(runBtn);
        }
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "copy";
        btn.textContent = "copy";
        btn.setAttribute("aria-label", "Copy command");
        btn.addEventListener("click", async () => {
          await run(() => invoke("copy_to_clipboard", { text }));
          btn.textContent = "copied";
          setTimeout(() => (btn.textContent = "copy"), 1200);
        });
        tools.appendChild(btn);
        pre.appendChild(tools);
      });

      // Inline code (e.g. `git status` written mid-sentence) gets a small copy
      // affordance too. Skip <code> inside a <pre> (fenced blocks, handled above)
      // and inside links/headings — there an injected button would join the link's
      // click target or mis-size against headline type. Wrap the code + its button
      // in a nowrap span so the affordance never wraps to a new line, orphaned from
      // the command it copies. Run stays fenced-only.
      node.querySelectorAll("code").forEach((codeEl) => {
        if (codeEl.closest("pre, a, h1, h2, h3, h4, h5, h6")) return;
        const text = codeEl.textContent; // filled values — human-facing (copy)
        const wrap = document.createElement("span");
        wrap.className = "inline-code";
        codeEl.replaceWith(wrap);
        wrap.appendChild(codeEl);
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "copy-inline";
        btn.innerHTML = COPY_ICON;
        btn.title = "Copy";
        btn.setAttribute("aria-label", "Copy inline command");
        btn.addEventListener("click", async (e) => {
          e.stopPropagation(); // don't bubble (e.g. into surrounding interactive text)
          await run(() => invoke("copy_to_clipboard", { text }));
          btn.innerHTML = CHECK_ICON;
          setTimeout(() => (btn.innerHTML = COPY_ICON), 1200);
        });
        wrap.appendChild(btn);
      });
    }

    render();
    return {
      update(p) {
        const key = keyOf(p);
        current = p;
        if (key === lastKey) return; // nothing visible changed — keep run-output, skip re-parse
        render();
      },
    };
  }

  // Run a code block's command and render its captured output just below the
  // block. Output is set via textContent (never innerHTML) so command output
  // can't inject markup into the overlay. A re-render (e.g. a variable change)
  // clears the panel, which is fine — it would be stale anyway.
  async function execBlock(pre, text, btn) {
    btn.disabled = true;
    const label = btn.textContent;
    btn.textContent = "running…";
    // Run in the runbook's pinned directory, if any (D15).
    const cwd = selected?.projectDir || null;
    const res = await run(() => invoke("run_command", { text, cwd }));
    btn.disabled = false;
    btn.textContent = label;
    let out = pre.nextElementSibling;
    if (!out || !out.classList.contains("run-output")) {
      out = document.createElement("div");
      pre.after(out);
    }
    out.className = "run-output";
    if (res == null) {
      out.remove(); // run() already surfaced the error in the banner
      return;
    }
    renderRunOutput(out, res);
  }

  function renderRunOutput(el, res) {
    el.innerHTML = "";
    const head = document.createElement("div");
    head.className = "run-head " + (res.exitCode === 0 ? "ok" : "fail");
    head.textContent = res.exitCode === 0 ? "✓ exit 0" : `✕ exit ${res.exitCode ?? "?"}`;
    const close = document.createElement("button");
    close.type = "button";
    close.className = "run-close";
    close.textContent = "✕";
    close.title = "Dismiss output";
    close.addEventListener("click", () => el.remove());
    head.appendChild(close);
    el.appendChild(head);
    const stream = (txt, cls) => {
      const p = document.createElement("pre");
      p.className = "run-stream " + cls;
      p.textContent = txt;
      el.appendChild(p);
    };
    if (res.stdout) stream(res.stdout, "out");
    if (res.stderr) stream(res.stderr, "err");
    if (!res.stdout && !res.stderr) {
      const empty = document.createElement("div");
      empty.className = "run-empty";
      empty.textContent = "(no output)";
      el.appendChild(empty);
    }
  }

  async function onKeydown(e) {
    // Ctrl/Cmd+K — toggle the quick switcher from any mode (D14).
    if ((e.ctrlKey || e.metaKey) && (e.key === "k" || e.key === "K")) {
      e.preventDefault();
      if (paletteOpen) closePalette();
      else await openPalette();
      return;
    }
    // Cmd/Ctrl+Enter (save & next) is dispatched by the editor itself via
    // on:submit, so it works from both Quick-add and the Browse step editor.
    if (e.key === "Escape") {
      // Title-edit Esc is handled on its own input (stops propagation), so it
      // never reaches here. Cancel an open palette/picker/editor/composer before hiding.
      if (paletteOpen) closePalette();
      else if (pickerOpen) pickerOpen = false;
      else if (editingId !== null) editingId = null;
      else if (addingStep) cancelAddStep();
      else await appWindow.hide();
    }
  }

  onMount(async () => {
    await loadRunbooks();
    // Restore the persisted current runbook (if it still exists).
    const saved = await run(() => invoke("get_setting", { key: CURRENT_RB_KEY }));
    const savedId = saved != null ? Number(saved) : null;
    if (savedId != null && runbooks.some((r) => r.id === savedId)) currentRunbookId = savedId;

    // Load settings: accent (apply immediately), hotkey, autostart.
    const savedAccent = await run(() => invoke("get_setting", { key: ACCENT_KEY }));
    if (savedAccent) applyAccent(savedAccent);
    hotkey = (await run(() => invoke("get_hotkey"))) ?? hotkey;
    hotkeyInput = hotkey;
    autostart = (await run(() => invoke("get_autostart"))) ?? false;
    allowRun = (await run(() => invoke("get_setting", { key: ALLOW_RUN_KEY }))) === "1";
    gitDir = (await run(() => invoke("get_setting", { key: GIT_DIR_KEY }))) ?? "";

    // The hotkey/tray summon lands on Quick-add, focused.
    unlistenShow = await listen("overlay:show", () => goQuick());
    await focusComposer();
  });

  onDestroy(() => unlistenShow?.());
</script>

<svelte:window on:keydown={onKeydown} on:click={onDocClick} />

<main class="overlay">
  <header class="bar" data-tauri-drag-region>
    <span class="dot"></span>
    <span class="title">Runebook</span>
    <nav class="tabs">
      <button class:active={mode === "quick"} aria-current={mode === "quick" ? "page" : undefined} on:click={goQuick}>Quick-add</button>
      <button class:active={mode === "browse"} aria-current={mode === "browse" ? "page" : undefined} on:click={() => setMode("browse")}>Browse</button>
      <button class:active={mode === "settings"} aria-current={mode === "settings" ? "page" : undefined} on:click={() => setMode("settings")} title="Settings" aria-label="Settings">⚙</button>
    </nav>
    <span class="hint">
      {#if mode === "quick"}⌘↵ save &amp; next &middot; {/if}⌘K jump &middot; {hotkey} toggles &middot; Esc hides
    </span>
  </header>

  {#if error}
    <p class="error" role="alert">
      <span>{error}</span>
      <button class="error-dismiss" title="Dismiss" aria-label="Dismiss error" on:click={() => (error = "")}>✕</button>
    </p>
  {/if}

  {#if paletteOpen}
    <!-- ── Command palette / quick switcher (D14) ───────────────── -->
    <div class="palette-backdrop">
      <div class="palette" bind:this={paletteCard} role="dialog" aria-modal="true" aria-label="Quick switcher">
        <input
          class="palette-input"
          bind:this={paletteInput}
          bind:value={paletteQuery}
          on:input={() => (paletteIndex = 0)}
          on:keydown={paletteKeydown}
          placeholder="Jump to a runbook…"
          aria-label="Jump to a runbook"
        />
        {#if paletteResults.length === 0}
          <p class="muted palette-empty">No runbooks match.</p>
        {:else}
          <ul class="palette-list">
            {#each paletteResults as r, i (r.id)}
              <li>
                <button
                  class="palette-item"
                  class:active={i === paletteIndex}
                  on:click={() => palettePick(r)}
                  on:mousemove={() => (paletteIndex = i)}
                >
                  <span class="palette-title">{r.title}</span>
                  {#if progressMap[r.id]}
                    <span class="rb-progress">{progressMap[r.id].done}/{progressMap[r.id].total}</span>
                  {/if}
                  {#each r.tags as t}<span class="tag">#{t}</span>{/each}
                </button>
              </li>
            {/each}
          </ul>
        {/if}
        <div class="palette-foot"><kbd>↑↓</kbd> navigate &middot; <kbd>↵</kbd> open &middot; <kbd>esc</kbd> close</div>
      </div>
    </div>
  {/if}

  {#if mode === "quick"}
    <!-- ── Quick-add (capture) ──────────────────────────────────── -->
    <section class="quick">
      <div class="rb-picker">
        <span class="rb-label">Runbook</span>
        <!-- Custom dropdown (see pickerOpen): a native <select> popup can't be
             themed in WebKitGTK, so its options render unreadable. -->
        <div class="rb-dropdown" bind:this={pickerEl}>
          <button
            type="button"
            class="rb-trigger"
            aria-haspopup="menu"
            aria-expanded={pickerOpen}
            aria-label="Choose runbook for new notes"
            on:click={() => (pickerOpen = !pickerOpen)}
          >
            <span class="rb-trigger-label">
              {currentRunbook ? currentRunbook.title : "— new runbook from this note —"}
            </span>
            <span class="rb-caret" aria-hidden="true">▾</span>
          </button>
          {#if pickerOpen}
            <!-- A menu of real <button>s (not a listbox): putting a button inside
                 role=option is invalid nesting, so we use menu/menuitem and mark
                 the current target with aria-current. -->
            <ul class="rb-menu" role="menu">
              <!-- Always offered, so you can start a fresh note even after one is
                   selected — picking this makes the next save create a new runbook. -->
              <li role="none">
                <button type="button" role="menuitem" aria-current={currentRunbookId == null} class:on={currentRunbookId == null} on:click={() => chooseRunbook(null)}>
                  — new runbook from this note —
                </button>
              </li>
              {#each runbooks as r (r.id)}
                <li role="none">
                  <button type="button" role="menuitem" aria-current={currentRunbookId === r.id} class:on={currentRunbookId === r.id} on:click={() => chooseRunbook(r.id)}>
                    {r.title}
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
        <form class="new-rb" on:submit|preventDefault={createAndSelect}>
          <input placeholder="＋ new runbook…" aria-label="Create new runbook" bind:value={newRbName} />
        </form>
      </div>

      <MarkdownEditor
        bind:this={composer}
        bind:value={draft.body}
        grow
        on:submit={saveAndNext}
        placeholder={"Capture the step — type freely, or use the toolbar to format. ⌘↵ saves."}
      />

      <div class="q-foot">
        <button class="primary" on:click={saveAndNext}>Save &amp; next</button>
        <span class="flash" class:show={flash && flashScope === "quick"} role="status">{flashScope === "quick" ? flash : ""}</span>
        <span class="counter">
          {#if currentRunbook}→ {currentRunbook.title}{/if}
          {#if addedThisSession > 0}&nbsp;· {addedThisSession} added{/if}
        </span>
      </div>
    </section>
  {:else if mode === "browse"}
    <!-- ── Browse (replay) ──────────────────────────────────────── -->
    <div class="panes">
      <aside class="list">
        <form class="new" on:submit|preventDefault={createRunbook}>
          <input placeholder="New runbook title…" aria-label="New runbook title" bind:value={newRunbookTitle} />
          <button type="submit" aria-label="Create runbook">＋</button>
        </form>

        <input
          class="search"
          placeholder="Search runbooks & steps…"
          aria-label="Search runbooks and steps"
          bind:value={search}
          on:input={() => loadRunbooks(search)}
        />

        {#if allTags.length > 0}
          <div class="tag-filter">
            {#each allTags as t}
              <button class="tag-chip" class:on={activeTag === t} aria-pressed={activeTag === t} on:click={() => toggleTagFilter(t)}>
                #{t}
              </button>
            {/each}
          </div>
        {/if}

        {#if displayRunbooks.length === 0}
          <p class="muted empty">{activeTag ? "No runbooks with that tag." : "No runbooks yet."}</p>
        {:else}
          <ul>
            {#each displayRunbooks as r (r.id)}
              <li class:active={selected?.id === r.id}>
                <button class="rb" aria-current={selected?.id === r.id ? "true" : undefined} on:click={() => openRunbook(r.id)}>
                  <span class="rb-title">{r.title}</span>
                  {#if progressMap[r.id]}
                    <span
                      class="rb-progress"
                      class:complete={progressMap[r.id].done === progressMap[r.id].total}
                      title="Replay progress"
                    >
                      {progressMap[r.id].done}/{progressMap[r.id].total}
                    </span>
                  {/if}
                  {#each r.tags as t}<span class="tag">#{t}</span>{/each}
                </button>
                <button class="del" title="Delete runbook" aria-label="Delete runbook {r.title}" on:click={() => deleteRunbook(r.id)}>✕</button>
              </li>
            {/each}
          </ul>
        {/if}
      </aside>

      <section class="detail">
        {#if !selected}
          <p class="muted">Select a runbook to replay it.</p>
        {:else}
          {#if editingTitle}
            <form class="title-edit" on:submit|preventDefault={commitTitle}>
              <input
                bind:this={titleInput}
                bind:value={titleBuffer}
                aria-label="Runbook title"
                on:blur={commitTitle}
                on:keydown={(e) => {
                  if (e.key === "Escape") {
                    e.preventDefault();
                    e.stopPropagation();
                    cancelTitle();
                  }
                }}
              />
            </form>
          {:else}
            <h2 class="rb-heading">
              <button class="title-btn" title="Rename runbook" aria-label="Rename runbook" on:click={startEditTitle}>{selected.title}</button>
              <button class="icon rename" title="Rename runbook" aria-label="Rename runbook" on:click={startEditTitle}>✎</button>
            </h2>
          {/if}

          <div class="tag-editor">
            {#each selected.tags as t}
              <span class="tag-chip on">
                #{t}
                <button class="tag-x" title="Remove tag" aria-label="Remove tag {t}" on:click={() => removeTag(t)}>✕</button>
              </span>
            {/each}
            <form on:submit|preventDefault={addTag}>
              <input class="tag-input" placeholder="＋ tag" aria-label="Add tag" bind:value={newTag} />
            </form>
            <span class="export-actions">
              <button class="ghost sm" on:click={copyMarkdown}>Copy .md</button>
              <button class="ghost sm" on:click={saveMarkdown}>Save .md</button>
            </span>
            <span class="flash" class:show={flash && flashScope === "browse"} role="status">{flashScope === "browse" ? flash : ""}</span>
          </div>

          <!-- Project pinning (D15): the folder commands run in. -->
          <div class="pin-row">
            {#if selected.projectDir}
              <span class="pin-chip" title="Run buttons execute commands here">
                📁 <span class="pin-path">{selected.projectDir}</span>
                <button class="tag-x" title="Unpin folder" aria-label="Unpin folder" on:click={unpinFolder}>✕</button>
              </span>
            {:else}
              <button class="ghost sm" on:click={pinFolder}>📁 Pin to folder…</button>
            {/if}
          </div>

          {#if varNames.length > 0}
            <div class="vars">
              <span class="vars-label">Variables</span>
              {#each varNames as name (name)}
                <!-- A div (not <label>) so it doesn't wrap both the input and the
                     secret button; the input is named explicitly via aria-label. -->
                <div class="var">
                  <span class="var-name" aria-hidden="true">{name}</span>
                  <input
                    use:inputType={secretVars.has(name)}
                    placeholder={`{{${name}}}`}
                    aria-label={`Value for ${name}`}
                    bind:value={varValues[name]}
                  />
                  <button
                    type="button"
                    class="var-secret"
                    class:on={secretVars.has(name)}
                    aria-pressed={secretVars.has(name)}
                    aria-label={secretVars.has(name) ? `${name}: secret (masked, not saved)` : `Mark ${name} secret`}
                    title={secretVars.has(name)
                      ? "Secret — masked, never saved to a profile"
                      : "Mark as secret"}
                    on:click={() => toggleSecret(name)}
                  >
                    🔒
                  </button>
                </div>
              {/each}

              <!-- Profiles: named value sets (e.g. prod / staging). Click to
                   apply; save the current values under a name; ✕ deletes. -->
              <div class="profiles">
                <span class="vars-label">Profiles</span>
                {#each varProfiles as p (p)}
                  <span class="tag-chip prof" class:on={activeProfile === p}>
                    <button class="prof-load" aria-label={`Apply profile ${p}`} on:click={() => applyProfile(p)}>{p}</button>
                    <button class="tag-x" title="Delete profile" aria-label={`Delete profile ${p}`} on:click={() => deleteProfile(p)}>✕</button>
                  </span>
                {/each}
                <form class="prof-save" on:submit|preventDefault={saveProfile}>
                  <input class="tag-input" placeholder="＋ save as…" aria-label="Save current values as a profile" bind:value={newProfileName} />
                </form>
              </div>
            </div>
          {/if}

          {#if selected.steps.length > 1}
            <!-- Replay (D10): work the runbook as a checklist; progress persists. -->
            <div class="replay-bar">
              <button class="ghost sm replay-toggle" class:on={replayMode} aria-pressed={replayMode} on:click={toggleReplay}>
                {replayMode ? "✓ Replaying" : "▶ Replay"}
              </button>
              {#if replayMode}
                <div class="progress" title="{doneCount} of {totalSteps} steps done">
                  <div
                    class="progress-fill"
                    style="width: {totalSteps ? (doneCount / totalSteps) * 100 : 0}%"
                  ></div>
                </div>
                <span class="progress-label">
                  {doneCount}/{totalSteps}{#if totalSteps > 0 && doneCount === totalSteps} · done ✓{/if}
                </span>
                <button class="ghost sm" on:click={resetProgress}>Reset</button>
              {/if}
            </div>
          {/if}

          {#if selected.steps.length === 0}
            <!-- A fresh note: write the body inline. Steps are optional — you
                 can stop at one note or add more below later. -->
            <div class="composer">
              <p class="muted add-hint">Write your note — add more steps later only if you want.</p>
              <MarkdownEditor
                bind:this={stepComposer}
                bind:value={addBuffer.body}
                on:submit={saveAddStep}
                placeholder={"Write the note — ⌘↵ to save."}
              />
              <div class="actions">
                <button class="primary" on:click={saveAddStep}>Save note</button>
              </div>
            </div>
          {:else}
            <!-- One step renders as a plain note (no number, no reorder);
                 two or more render as a numbered list. -->
            <ol class="steps" class:single={selected.steps.length === 1}>
              {#each selected.steps as s, i (s.id)}
                <li class:done={replayMode && s.done}>
                  {#if editingId === s.id}
                    <MarkdownEditor bind:value={editBuffer.body} on:submit={() => saveEdit(s.id)} />
                    <div class="actions">
                      <button on:click={() => saveEdit(s.id)}>Save</button>
                      <button class="ghost" on:click={() => (editingId = null)}>Cancel</button>
                    </div>
                  {:else}
                    <div class="step-head">
                      {#if replayMode}
                        <input
                          type="checkbox"
                          class="step-check"
                          checked={s.done}
                          title="Mark this step done"
                          aria-label={`Mark step ${i + 1} done: ${s.title?.trim() || deriveLabel(s.body, i)}`}
                          on:change={(e) => setStepDone(s, e.target.checked)}
                        />
                      {/if}
                      {#if selected.steps.length > 1}
                        <span class="step-title">{s.title?.trim() || deriveLabel(s.body, i)}</span>
                      {/if}
                      <span class="step-tools">
                        {#if selected.steps.length > 1}
                          <button class="icon" title="Move up" aria-label={`Move step ${i + 1} up`} on:click={() => moveStep(i, -1)}>↑</button>
                          <button class="icon" title="Move down" aria-label={`Move step ${i + 1} down`} on:click={() => moveStep(i, 1)}>↓</button>
                        {/if}
                        <button class="icon" title="Edit" aria-label={`Edit step ${i + 1}`} on:click={() => startEdit(s)}>✎</button>
                        <button class="icon" title="Delete" aria-label={`Delete step ${i + 1}`} on:click={() => deleteStep(s.id)}>✕</button>
                      </span>
                    </div>
                    {#if s.body.trim()}
                      <div class="rendered" use:markdown={{ body: s.body, vars: varValues, allowRun }}></div>
                    {/if}
                  {/if}
                </li>
              {/each}
            </ol>

            {#if addingStep}
              <div class="composer">
                <MarkdownEditor
                  bind:this={stepComposer}
                  bind:value={addBuffer.body}
                  on:submit={saveAddStep}
                  placeholder={"Write the next step — ⌘↵ to save."}
                />
                <div class="actions">
                  <button class="primary" on:click={saveAddStep}>Add step</button>
                  <button class="ghost" on:click={cancelAddStep}>Cancel</button>
                </div>
              </div>
            {:else}
              <button class="ghost add-step" on:click={startAddStep}>＋ Add step</button>
            {/if}
          {/if}
        {/if}
      </section>
    </div>
  {:else}
    <!-- ── Settings ─────────────────────────────────────────────── -->
    <section class="settings">
      <div class="setting">
        <label for="hotkey">Global hotkey</label>
        <form class="hotkey-row" on:submit|preventDefault={applyHotkey}>
          <input id="hotkey" bind:value={hotkeyInput} placeholder="Control+Alt+Space" />
          <button type="submit">Apply</button>
        </form>
        <p class="setting-hint">e.g. <code>Control+Alt+Space</code>, <code>Super+R</code>. Applies immediately.</p>
      </div>

      <div class="setting">
        <span class="setting-label" id="accent-label">Accent</span>
        <div class="swatches" role="group" aria-labelledby="accent-label">
          {#each ACCENTS as a}
            <button
              class="swatch"
              class:on={accent === a.value}
              style="background: {a.value}"
              title={a.name}
              aria-label={a.name}
              on:click={() => chooseAccent(a.value)}
            ></button>
          {/each}
        </div>
      </div>

      <div class="setting">
        <label class="checkbox">
          <input type="checkbox" checked={autostart} on:change={toggleAutostart} />
          Launch on login
        </label>
      </div>

      <div class="setting">
        <span class="setting-label">Execution</span>
        <label class="checkbox">
          <input type="checkbox" checked={allowRun} on:change={toggleAllowRun} />
          Enable Run buttons (execute commands from steps)
        </label>
        <p class="setting-hint">
          Adds a <strong>▶ run</strong> button to every code block in Browse. Commands run in
          your shell with your own permissions — only enable this for runbooks you trust.
          Off by default.
        </p>
      </div>

      <div class="setting">
        <span class="setting-label">Data</span>
        <div class="hotkey-row">
          <button on:click={backupDatabase}>Back up database…</button>
          <button class="ghost" on:click={restoreDatabase}>Restore…</button>
        </div>
        <p class="setting-hint">
          The whole runbook database is one portable SQLite file. Restore replaces
          all current data.
        </p>
      </div>

      <div class="setting">
        <span class="setting-label">Git sync</span>
        <p class="setting-hint">
          Export every runbook as Markdown into a folder and commit it with git
          (one <code>.md</code> per runbook under <code>runbooks/</code>). Push is
          optional. The folder is <code>git init</code>'d if it isn't a repo yet.
        </p>
        <div class="hotkey-row">
          <button on:click={chooseGitDir}>{gitDir ? "Change folder…" : "Choose folder…"}</button>
          <button class="ghost" on:click={() => gitSyncNow(false)} disabled={!gitDir}>Sync now</button>
          <button class="ghost" on:click={() => gitSyncNow(true)} disabled={!gitDir}>Sync &amp; push</button>
        </div>
        {#if gitDir}<p class="setting-hint">Folder: <code>{gitDir}</code></p>{/if}
        {#if gitStatus}<p class="setting-hint git-status">{gitStatus}</p>{/if}
      </div>

      <span class="flash" class:show={flash && flashScope === "settings"} role="status">{flashScope === "settings" ? flash : ""}</span>
    </section>
  {/if}
</main>

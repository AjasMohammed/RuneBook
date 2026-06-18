---
id: current-runbook-setting
label: Current runbook via setting kv table
type: decision
community: Data & Storage
edges:
  - target: rusqlite-data-layer
    relation: part_of
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: ipc-boundary
    relation: implements
    confidence: EXTRACTED
    confidence_score: 1.0
---

The "current runbook" that Quick-add appends to is persisted in a generic
key/value **`setting`** table (migration v3), under key `current_runbook`, via
`get_setting`/`set_setting` IPC commands. Chosen over `tauri-plugin-store`
(named in early docs) for the same reason as [[rusqlite-data-layer]] (D7): keep
state in the Rust core behind IPC, not in a frontend-facing plugin. The kv table
is reusable for later prefs (custom hotkey, theme, window position).

On launch the UI restores `current_runbook` but ignores a stale id whose runbook
was deleted. Deleting the current runbook in Browse also clears it.

**Fix 2026-06-16 — Quick-add could only ever append.** The picker's
"— new runbook from this note —" option was gated `{#if currentRunbookId == null}`,
so once a runbook was selected/persisted it disappeared and every save appended a
step to it (no way to start a new note). Now the option is **always rendered**,
and `setCurrentRunbook(null)` persists `""` to clear the setting (onMount treats
empty/unknown as "none"), so "new runbook" sticks across sessions. Saving with no
current runbook still creates one from the first note's derived label and selects
it (the multi-step "save & next" flow is unchanged).

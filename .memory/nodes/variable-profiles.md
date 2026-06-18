---
id: variable-profiles
label: Variable Profiles + Secrets
type: decision
community: ux-capture
edges:
  - target: command-variables
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: rusqlite-data-layer
    relation: implements
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: current-runbook-setting
    relation: references
    confidence: EXTRACTED
    confidence_score: 0.9
---

Phase 6 / D12. Extends the in-memory [[{{var}} placeholders|command-variables]]
with **named profiles per runbook** (e.g. prod / staging). Stored in a
`var_profile` table (migration **v6**) as a JSON `name→value` map per profile —
one JSON column rather than a normalized value table, because a profile is a
small bag of strings and the blob keeps CRUD + IPC (`HashMap<String,String>`)
trivial. Saving the same name overwrites (save == update). UI is tag-style chips:
click to apply, "save as…" input to create, ✕ to delete.

**Secret vars never persist.** A per-variable 🔒 toggle masks the input
(`type=password`) and **excludes that var when a profile is saved**, so a
key/password never reaches the DB. Only the set of secret *names* persists — in
the [[setting kv|current-runbook-setting]] under `secret_vars:<runbookId>` — so
the mask is remembered; the value is retyped each session. This is the
no-new-dependency partial answer to open question Q4 (full encrypted DB / keyring
still deferred).

Gotcha hit here: Svelte 4 forbids a dynamic `type` attribute on an input that
also uses `bind:value` — see [[svelte-dynamic-input-type]].

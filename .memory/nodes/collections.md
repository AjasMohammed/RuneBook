---
id: collections
label: Collections (folder grouping)
type: decision
community: Data & Storage
edges:
  - target: tag-filtering
    relation: conceptually_related_to
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: db-rs-shared-with-mcp
    relation: caused_by
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: select-option-color
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: rusqlite-data-layer
    relation: implements
    confidence: EXTRACTED
    confidence_score: 1.0
---

D18: a **collection** is a named group (title + description) of runbooks.
**Many-to-many, like tags** — a runbook can be in several collections, via a
`runbook_collection` junction table (mirrors `runbook_tag`). First shipped as a
one-collection folder (migration **v9**, `runbook.collection_id`), then **widened to
many-to-many on request** (migration **v10**: create junction, backfill the v9 folder
assignment, drop the dead column — migrations stay append-only, so v9 is left intact).
Distinct from [[conceptually related tags|tag-filtering]]: tags = lightweight
cross-cutting labels, a collection = a curated home with its own title/description.

Key choices:
- **Junction CASCADE both ways**: deleting a collection drops only its membership rows
  (runbooks survive, un-filed); deleting a runbook drops its memberships. Same as
  `runbook_tag`.
- **`set_runbook_collections(id, ids[])`** replaces the whole set (mirrors `set_tags`),
  NOT a `RunbookPatch` field — on purpose, to avoid [[breaking the MCP struct literal|db-rs-shared-with-mcp]].
  `Runbook` gained a serialized `collectionIds` array (read-only on the MCP side; MCP
  tools don't expose collections). `collections_for()` hydrates it per runbook, like
  `tags_for()`.
- **Counts are client-side** — every loaded runbook carries `collectionIds`, so no
  count query (`collCounts` reduce in App.svelte).
- **UI**: a collection-chip filter row + inline title/description editor in the Browse
  sidebar (mirrors the tag-chip filter); the open runbook's memberships show as
  removable chips + an "＋ add" menu in its detail pane
  ([[native select can't be themed in WebKitGTK|select-option-color]], reuses
  `.tag-chip` + `.rb-dropdown`/`.rb-menu`).
- **Scope kept minimal**: no nesting, no color/icon, no collection support in MCP
  tools or in Markdown/git export.

Test: `collections_membership_is_many_to_many` in `db.rs` (multi-membership, set
replacement, and CASCADE-on-delete from both the collection and the runbook side).

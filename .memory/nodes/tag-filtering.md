---
id: tag-filtering
label: Tag filtering
type: concept
community: UX & Capture
edges:
  - target: fts5-search
    relation: conceptually_related_to
    confidence: INFERRED
    confidence_score: 0.7
---

Phase 3 tag filtering (done 2026-06-16), in `src/App.svelte`. Tags are assigned
per runbook in the Browse detail header (chip list + "＋ tag" input) by calling
`update_runbook` with a `tags` patch — the Rust `set_tags` upserts tag names and
rewrites the `runbook_tag` links.

Filtering itself is **client-side**: the sidebar shows distinct tag chips
(derived from the loaded list); clicking one filters the displayed runbooks to
those containing it. This composes with server-side search — search runs in Rust
(FTS), the tag filter narrows the returned set in the UI — so no extra backend
parameter was needed. Deleting the last use of a tag clears an active filter.

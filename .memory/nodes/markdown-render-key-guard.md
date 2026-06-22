---
id: markdown-render-key-guard
label: Markdown Action Re-render Key Guard
type: gotcha
community: ux-capture
edges:
  - target: copy-per-code-block
    relation: part_of
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: command-variables
    relation: caused_by
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: executable-steps
    relation: references
    confidence: EXTRACTED
    confidence_score: 0.9
---

The `markdown` Svelte action in `App.svelte` (`use:markdown={{ body, vars: varValues, allowRun }}`)
does `node.innerHTML = marked.parse(...)` on every `update()`. Because `varValues`
is two-way bound to the variable fill-in inputs, **every keystroke** in a variable
field re-invoked `render()` — on *every* rendered step, even ones with no
`{{placeholder}}` — re-parsing markdown, re-wiring all copy/run buttons, and
**destroying any `.run-output` panel** (it lives as a sibling of `<pre>` inside the
same node). Output vanished character-by-character exactly while filling vars to
re-run.

Fix (2026-06): `update(p)` now computes a cheap `keyOf(p)` = body + allowRun +
`fillVars(body, vars)` and skips `render()` when the key is unchanged. So typing a
variable a step doesn't reference (or any var edit on a placeholder-free step) no
longer re-renders. Steps that genuinely contain the changed `{{var}}` still
re-render (and still clear their own output — stale anyway). If you ever need
run-output to *survive* a legitimate re-render, it has to move out of the
action-managed node (e.g. a Svelte-owned sibling), not just be re-appended.

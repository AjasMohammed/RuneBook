---
id: svelte-dynamic-input-type
label: Svelte dynamic input type + bind:value
type: gotcha
community: environment-build
edges:
  - target: variable-profiles
    relation: caused_by
    confidence: EXTRACTED
    confidence_score: 1.0
---

Svelte 4 rejects at compile time: `<input type={cond ? 'password' : 'text'}
bind:value={x} />` → *"'type' attribute cannot be dynamic if input uses two-way
binding"*. The build (`npm run build`) fails, not just a warning.

Fix used for the [[secret variable masking|variable-profiles]]: drop the `type`
attribute from markup and set the property imperatively with a tiny action —

```js
function inputType(node, secret) {
  node.type = secret ? "password" : "text";
  return { update(s) { node.type = s ? "password" : "text"; } };
}
```

`use:inputType={secretVars.has(name)}` + `bind:value` — binding works the same
for text and password, so this is clean. Alternative (rejected): two `{#if}`
branches duplicating the input.

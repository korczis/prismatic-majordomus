+++
title = "Rendering reference"
description = "Representative Markdown for validating Flowbite Typography, syntax highlighting, tables, callouts and Mermaid across viewports. Not linked from navigation."
template = "docs-page.html"
[extra]
source = "site/content-src/render-test.md"
noindex = true
+++
{% raw %}
This page exists to validate rendering. It is the one Markdown file under `site/content/` that is hand-written, and it carries no product claims.

## Second level heading

Paragraph with **strong**, *emphasis*, `inline code`, a [link to the CLI reference](@/docs/cli-specification.md), and a very long unbroken token: `aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`.

### Third level

#### Fourth level

##### Fifth level

###### Sixth level

- unordered item
- another item
  - nested item
  - nested item with `code`
    - third level
- [ ] task not done
- [x] task done

1. ordered item
2. ordered item
   1. nested ordered
   2. nested ordered

> A blockquote that runs long enough to wrap on a narrow viewport, so that its left border and padding can be judged against the surrounding text.

<div class="not-format my-6 flex gap-3 rounded-base border p-4 text-sm border-brand-subtle bg-brand-softer text-fg-brand-strong" role="note">
<span class="font-mono text-xs font-semibold uppercase tracking-wide">Note</span>
<div class="min-w-0 flex-1">

A note callout, projected from GitHub's own alert syntax. On GitHub it renders as an alert; here as a Flowbite alert.

</div></div>


<div class="not-format my-6 flex gap-3 rounded-base border p-4 text-sm border-warning-subtle bg-warning-soft text-fg-warning" role="note">
<span class="font-mono text-xs font-semibold uppercase tracking-wide">Warning</span>
<div class="min-w-0 flex-1">

A warning callout. Only five kinds exist: note, tip, important, warning, caution.

</div></div>


```bash
majordomus start "fix OAuth callback" --scope lib/auth --profile debugging
majordomus check --explain | head -20   # a deliberately long line so that horizontal scrolling inside the block can be checked on a 320px viewport
```

```yaml
version: 1
context:
  always_loaded_budget_lines: 150
  strategy: minimum-sufficient
```

```json
{"ts":"2026-09-03T19:30:12Z","event":"task.started","task_id":"t-20260903193012-a4f1"}
```

<div class="overflow-x-auto">

| command | answers | writes | exit |
|---|---|---|---|
| `doctor` | is Majordomus itself real and wired here? | no | 0 / 10 / 12 |
| `check` | is the task consistent with policy, scope, state? | no, except `--checkpoint` | 0 / 10 |
| `finish` | evaluate the finish contract | task record, ledger | 0 / 10 / 15 |
| `a very wide column to force horizontal scrolling on phones` | `and another wide one so the wrapper has to do its job rather than the page` | yes | 0 |

</div>


---

<pre class="mermaid">
flowchart LR
    A[canonical Markdown] --&gt; B[generate-site-data]
    B --&gt; C[site/content/docs]
    C --&gt; D[Zola]
    D --&gt; E[Flowbite Typography]
</pre>


A footnote reference[^1] closes the page.

[^1]: Footnotes are supported by Zola's Markdown renderer.
{% endraw %}

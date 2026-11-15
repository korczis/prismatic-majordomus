# Published surfaces for any repository: site, wiki and Markdown regions

Majordomus serves its own surfaces from one registry ([`SURFACES.md`](SURFACES.md)). A
repository that Majordomus supervises publishes documentation too: a README, the documents
around it and the `.ai/` layer. These go out as a website, sometimes as a wiki, and as
index tables inside the documents themselves. If they are written by hand, each published
copy is a second source of truth. [`share/surfaces/`](../share/surfaces/README.md) is the
producer that removes those copies. It is shipped in every release and is
repository-agnostic.

## What it produces

| Surface | Derived from | Published to | Declared as |
|---|---|---|---|
| Site | Every tracked Markdown document. The overview's ECharts views are computed from the history, the documentation map and the plan. The `.ai/` layer page has a Mermaid dependency graph derived from each issue's `depends_on` | GitHub Pages | `target/web/site/surface.json` |
| Wiki | The same documents, with relative links rewritten to wiki pages | The repository's GitHub wiki | `target/web/wiki/surface.json` |
| Markdown regions | The documents' titles and first paragraphs, and the `.ai/` features, decisions, rules and plan | The documents, between `surfaces:begin` and `surfaces:end` markers | `target/web/markdown/surface.json` (state report) |

Each declaration is `web-surface/v1`, under the generated root that
`majordomus_web_surfaces` discovers (`apps/majordomus-cli/src/web/discover.rs`). A
repository's published documentation therefore appears in the same topology as the
routes the executable serves.

## Why it is derived

- A copy that a person keeps in step drifts. The producer publishes nothing that has
  content of its own. A page is its document rendered, a chart is computed from the
  history, and a table is generated from the records it summarises.
- Drift is a failing check, not a review comment. `surfaces check` refuses a stale region,
  or a relative link or anchor that resolves to nothing, and names each with its file and
  line. An adopting repository runs it in its pre-commit hook (as a policy `enforcement`
  entry that `majordomus doctor` reconciles), in its gate, and in the shipped workflow,
  which publishes nothing from a failing run.
- It is generic. No repository is named. Facts come from git, the `.ai/manifest.yaml`
  sections and front matter. Issue status is derived from the issue stamps, as
  `majordomus plan` derives it. A repository declares only what data cannot decide, in
  `surfaces.toml`.

## Adopting it

The steps are in [`share/surfaces/README.md`](../share/surfaces/README.md), under
"Adopting it in a repository":

1. vendor the directory;
2. declare `surfaces.toml` if needed;
3. copy [`share/surfaces/workflow.yml`](../share/surfaces/workflow.yml);
4. enable Pages and the wiki;
5. wire the check;
6. adopt [`share/surfaces/rule.template.md`](../share/surfaces/rule.template.md) as a
   project rule.

Its behaviour is held by `test/cases/4611_published_surfaces_are_derived_and_checked.sh`.
That case runs against a throwaway repository: it covers the refusals, the regeneration,
the fenced example left alone, and the site and wiki outputs with their declarations.

## What it does not do yet

- It is shipped as files that a repository vendors. No `majordomus` command runs it, and
  no capability in the registry describes it.
- The rule is a template with no doctor validator. The adopting repository's hook, gate
  and CI enforce it.
- The workflow's YAML is not checked by `surfaces check`. Run `actionlint` on the copy.

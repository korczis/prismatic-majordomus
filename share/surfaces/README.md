# Published surfaces

One generator turns what a repository already states into everything it publishes. The
sources are its tracked Markdown, its git history and its `.ai/` layer. The outputs are:

| Surface | Command | What it is | Where it goes |
|---|---|---|---|
| Site | `surfaces site` | Every Markdown document as a page, plus an overview with interactive charts, the `.ai/` layer (features, decisions, rules, plan, dependency graph) and search | GitHub Pages |
| Wiki | `surfaces wiki` | The same documents as wiki pages: `Home`, a sidebar, links rewritten to pages | The repository's GitHub wiki |
| Markdown regions | `surfaces sync` | Tables inside the documents themselves (document indexes, features, decisions, rules, plan, surfaces), between markers | The repository |
| Gate | `surfaces check` | Fails while a region is stale, or a relative link or anchor leads nowhere | Pre-commit, the repository's own gate, CI |

Nothing in the generator names a repository. Every fact comes from git (`origin`, `HEAD`,
the tracked files, the log), from the `.ai/manifest.yaml` sections, and from each
document's own front matter and headings. The only declared input is `surfaces.toml` at
the repository root, and only for the choices data cannot make. Today that is which
documents to publish and which to exempt from the link check.

## Why

Documentation drifts the moment it restates something. A hand-kept index misses the
newest document. A status table says "planned" a week after the work merged. A landing
page describes a repository that changed visibility, and links point at files that were
renamed. Each copy is a second source of truth, so it is wrong as soon as the first one
moves.

The fix is not more diligence. Publish nothing that a person has to keep in step:

- **Derive, never restate.** A page is rendered from its document. A chart is computed
  from the history. A table is generated from the records it summarises. The published
  copy cannot disagree with the source because it has no content of its own.
- **One source, many surfaces.** A reader on GitHub, on the site, in the wiki or in an
  editor sees the same text. The region tables put the derived summaries inside the
  documents, where an agent working in the repository reads them too.
- **Enforced, not advised.** A stale region or a dangling link fails the commit, the gate
  and CI. Drift becomes a failing check with a name and a line number, not something a
  reviewer has to notice.
- **Readers drive the views.** Every chart is a model the reader controls: a window, a
  metric or a cut-off, kept in the URL so a view can be linked. Each chart maximizes to the
  viewport and Escape restores it. A diagram is Mermaid source in the document, rendered in
  place, never a pasted image.
- **Generic.** The same files work in any repository. A repository supervised by
  Majordomus also gets its `.ai/` layer rendered: the `.ai/manifest.yaml` sections name
  where features, decisions, rules and the plan live, and the issue status is derived from
  the stamps as `majordomus plan` derives it. The site, wiki and region report each declare
  themselves as `web-surface/v1` under `target/web/<id>/`, the generated root that
  `majordomus_web_surfaces` discovers (`apps/majordomus-cli/src/web/discover.rs`,
  `GENERATED_ROOT` and `DECLARATION_FILE`).

## Use

```bash
surfaces check          # the gate: exit 10 on a finding
surfaces sync           # regenerate every Markdown region in place
surfaces sync --check   # write nothing; exit 11 when a region would change
surfaces site           # build target/web/site (needs node for the assets)
surfaces wiki           # build target/web/wiki
surfaces list           # what each surface publishes, one document per line
```

Prerequisites: `python3` 3.11 or newer. The wrapper creates `.venv` beside itself from
`requirements.txt` on first use. `site` also needs `node`; it runs `npm ci` in this
directory the first time. Exit codes follow Majordomus: 0 ok, 2 usage, 10 findings,
11 drift.

To preview the site, serve `target/web/site` with any static server, for example
`python3 -m http.server -d target/web/site`.

## Markdown regions

A region is a pair of HTML comments whose contents the generator owns:

```text
<!-- surfaces:begin docs-index dir=research -->
(generated table)
<!-- surfaces:end docs-index -->
```

| Generator | Arguments | Table |
|---|---|---|
| `docs-index` | `dir=<path>` (default: all), `depth=<n>` | Every published document under the path, with its derived summary |
| `ai-features` | — | `.ai/` features: name, status, headline |
| `ai-adrs` | — | Architecture decisions: id, status, title |
| `ai-rules` | `origin=project\|vendored\|all` | Rules: id, class, statement |
| `ai-plan` | — | Milestones with issues done and still open (status derived from the issue stamps) |
| `surfaces` | — | Where the site and the wiki are published |

Edit the text around a region, never inside it. `surfaces sync` rewrites the inside, and
`surfaces check` fails while it would.

A document's title comes from its front matter `title`, or else its first `# ` heading.
Its summary comes from front matter `headline`, `description` or `statement`, or else its
first paragraph. A better first paragraph therefore improves every index that lists the
document.

## Site

- **Shell.** The site uses the Flowbite admin shell: a navbar with search, a sidebar
  grouped by top-level directory, a breadcrumb, cards and tables. It is built from
  `assets/input.css` with Tailwind 4, Flowbite 4 and `flowbite-typography`.
- **Front-end libraries.** Mermaid 12 renders diagrams and Apache ECharts 6 renders charts.
  Both are copied from `node_modules` and loaded only on pages that need them. Nothing is
  fetched from a CDN at view time.
- **Links.** A relative link to a published document becomes a link to its page, with
  GitHub-compatible heading anchors. A link to any other tracked file goes to GitHub at
  the exact commit the site was built from. Images load from that same commit.
- **Data.** `data/site.json` holds the chart inputs: weekly commits, the documentation map,
  per-document age at `HEAD` and plan progress. `data/search.json` holds the search index.
  Both are computed at build time from git, so the same commit always builds the same site.

## Wiki

GitHub keeps a wiki in a separate repository, `<repo>.wiki.git`, which exists only after
its first page has been saved once in the web interface. Until then the workflow's wiki
job fails and says so. Page names are derived from document paths, `README.md` becomes
`Home`, and `_Sidebar.md` and `_Footer.md` are generated. Each sync replaces the whole
wiki: any edit made in the wiki is overwritten, and the footer of every page says so.

## Adopting it in a repository

Majordomus ships the producer in `share/surfaces/`. It is copied whole into every release
(`scripts/release-package`), so an installed copy has it at
`$(dirname "$(readlink -f "$(command -v majordomus)")")/../share/surfaces`, which is
`~/.local/share/majordomus/versions/<version>/share/surfaces` with the default prefix.

1. **Vendor it.** Copy `share/surfaces/` to `tools/surfaces/` in the repository. CI needs it
   in the tree, because a runner has no Majordomus installed. Run `npm install` once in the copy
   and commit the `package-lock.json` it writes: the workflow caches on it and the site build
   then runs `npm ci` against it. The `.venv/` and
   `node_modules/` it creates beside itself belong in `.gitignore`, along with `/target/`.
   Messages and generated text name the command by its path from the repository root, so a
   vendored copy prints `tools/surfaces/surfaces sync`. `SURFACES_COMMAND` overrides the
   name.
2. **Declare what data cannot decide.** If needed, add `surfaces.toml` at the root:
   - `[sources] include/exclude` chooses which tracked Markdown is published;
   - `[check] exclude` lists documents whose links are someone else's to keep (for example,
     files carried unmodified from an upstream), each with its reason in a comment;
   - `[site]`, `[wiki]` and `[regions]` set output paths, which must stay under
     `target/web/`.

   Unknown sections and keys are refused.
3. **Publish.** Copy `workflow.yml` to `.github/workflows/surfaces.yml`, then enable Pages
   with GitHub Actions as its source (`gh api -X POST repos/<owner>/<repo>/pages -f
   build_type=workflow`) and enable the wiki (`gh api -X PATCH repos/<owner>/<repo> -F
   has_wiki=true`). GitHub creates the wiki's repository with its first page, so save one
   page in the web interface once.
4. **Enforce.** Add an `enforcement` entry to `.ai/repo/policy.yaml`:

   ```yaml
   - name: surfaces-on-commit
     path: tools/surfaces/surfaces
     args: [check]
     wired_by: git-hook:pre-commit
   ```

   Call it from the pre-commit hook as `tools/surfaces/surfaces check || exit $?`, guarded
   by `[ -x tools/surfaces/surfaces ]` so that a branch without the tool can still commit.
   `majordomus doctor` reconciles the two. Add the same check to the repository's own gate,
   and adopt the contract as a project rule: `rule.template.md` is a ready-made one, with
   no validator of its own because the hook and CI enforce it.

## In CI

`workflow.yml` runs `check`, `site` and `wiki` on every push and pull
request. From the default branch only, it deploys the site to Pages and pushes the wiki.
The checkout is full-history but blobless, because the charts read the commit log.

## Enforcement

Once adopted, `surfaces check` runs in three places:

- the pre-commit hook, as an `enforcement` entry reconciled by `majordomus doctor`;
- the repository's own gate;
- the workflow, which publishes nothing from a failing run.

The contract each repository adopts is `rule.template.md`.

## Files

| File | Role |
|---|---|
| `surfaces` | Entry point. Creates the virtual environment, then runs `surfaces.py` |
| `workflow.yml` | The GitHub Actions template: check on every push and pull request, publish the site and the wiki from the default branch |
| `rule.template.md` | The project rule a repository adopts with the producer |
| `surfaces.py` | The generator: discovery, rendering, regions, check |
| `assets/input.css`, `assets/app.js` | The site's stylesheet source and client script |
| `package.json` | The front-end dependencies, pinned exactly. The distribution ships no lock, because it states no third-party version by hand; a vendoring repository runs `npm install` once in its copy and commits the `package-lock.json` that `npm ci` and the workflow's cache then use |
| `requirements.txt` | Pinned Python dependencies |

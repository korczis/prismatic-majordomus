+++
title = "GitHub Pages architecture"
description = "how the website is derived from the repository and checked for drift"
weight = 29
[extra]
source = "docs/GITHUB_PAGES_ARCHITECTURE.md"
+++

{% raw %}

The website at `https://korczis.github.io/prismatic-majordomus/` is a projection of this
repository. It is built the way Majordomus asks projects to build their own instruction
files: one canonical source, generated outputs, and a check that fails when the two drift.

## Purpose

Let a visitor understand in about two minutes what Majordomus is, what it guarantees, what
is only advisory, what is not built, and how to start, without any sentence on the site
being maintained separately from the repository that backs it.

## Canonical sources

<div class="overflow-x-auto">

| layer | owns | lives in | edited by hand |
|---|---|---|---|
| product truth | policy schema, profiles, the worker instructions, the shell CLI | `share/skeleton/**`, `bin/majordomus`, `lib/**`, `share/commands.yaml` | yes |
| the Rust executable | every capability with its schemas, exposures, stability, provenance, benchmark and cache policy; the modules; the command line with the examples of every command | `capability!` and `module!` declarations under `apps/majordomus-cli/src/capability/builtin/`, `apps/majordomus-cli/src/cli.rs` (clap for the structure, `cli::EXAMPLES` for the examples) | yes |
| the executable's narrative | what the crate is, owns and refuses; discovery, transports, side effects, architecture | `apps/majordomus-cli/README.md`, `docs/CAPABILITIES.md`, `docs/MCP.md` | yes |
| benchmark evidence | the regression policy and the accepted baselines | `.ai/repo/benchmarks/rust/policy.yaml`, `.ai/repo/benchmarks/rust/baseline.<platform>.json` | policy yes; a baseline by `majordomus bench baseline update` |
| registry projections | the OpenAPI document, the registry manifest, the capability, module and benchmark references, and the command line as Markdown and as data | `docs/generated/**` | never — `majordomus generate` |
| registry dataset | everything the site renders about the executable: descriptors, modules, command line, MCP and HTTP surfaces, benchmarks, the index | `site/data/registry/registry.json` | never — `majordomus generate site` |
| public narrative | what it is, why, how, what it refuses | `README.md`, `docs/*.md` | yes |
| claims | every capability with status, source, implementation, test | `docs/CLAIMS.yaml` | yes |
| marketing copy | the hero's three propositions, section leads, button labels; no claims, no numbers, 60-line budget | `site/data/marketing.toml` | yes |
| navigation | five intents, their dropdown items and hrefs | `site/data/nav.toml` | yes |
| claim detail | what each claim means, how it works, how to see it, what it does not cover, why it exists | `docs/claims/<id>.md` | yes |
| case studies | the recognition moments, each with its homepage hook in front matter | `site/content-src/why/*.md` | yes |
| skills | the repository's skills, one directory each; the site reads the catalogue `lib/skills.sh` derives from the source class `skill` | `.ai/repo/skills/<id>/SKILL.md` | yes |
| rendering reference | representative Markdown for visual validation | `site/content-src/render-test.md` | yes |
| derived data | stable JSON the templates read | `site/data/generated/*.json` | never |
| derived release artifact | the claims matrix as Markdown | `docs/SITE_CLAIMS.md` | never |
| derived content | canonical Markdown with generated front matter; one page per profile, claim, status, responsibility, command, doctrine, use case, application, milestone, issue, case study, skill, module and capability | `site/content/{docs,profiles,guarantees,supervises,commands,doctrines,use-cases,applications,plan,why,registry,skills}/`, `render-test.md`, `architecture.md` | never |
| derived routes and links for the executable | one route per module and per capability, the executable's pages, the API anchor, the source on GitHub, the claims attached to each surface | `site/data/generated/executable.json` | never |
| the native command line, as the site renders it | the command tree flattened, each command with its route, usage, arguments, children and executed examples | `site/data/generated/cli.json` (from `docs/generated/cli.json`, via `scripts/lib/cli-site.jq`) | never |
| build provenance | the commit and its cleanliness, the site's input hash, the registry and index fingerprints | `site/data/build.json`, served as `/build.json` | never — `scripts/site-build`, not committed |
| presentation | Zola templates and Tera 2 components | `site/templates/**` | yes |
| styling entry | Tailwind v4 + Flowbite v4 directives | `site/tailwind.css` | yes |
| behaviour | theme toggle, Mermaid init | `site/theme.js`, `site/diagrams.js` | yes |
| output | the static site | `site/public/**` | never |

</div>


## Projection pipeline

Two generators, one graph. The Rust executable projects its own registry; the site
generator projects the canonical files and consumes the executable's projections through
one machine-readable boundary, `docs/generated/registry.json` (and `openapi.json`); Zola
renders templates over what both wrote. `scripts/derive` runs the stages in order,
`scripts/derive-check` proves the committed result, and no stage reads its own output.

<pre class="mermaid">
flowchart TB
    subgraph canonical
        R[capability! and module! declarations, src/cli.rs with cli::EXAMPLES]
        K[share/kinds.yaml, schemas, providers, policy]
        D[README, docs/*.md, docs/claims, CLAIMS.yaml, RESPONSIBILITIES.yaml]
        C[share/commands.yaml, use-cases.yaml, applications.yaml]
        P[.ai/repo/project, rules, benchmarks]
    end
    subgraph stageA[stage A · majordomus generate, code only]
        GA[docs/generated: openapi.json, registry.json, capabilities.md, cli.md, cli.json, modules/*.md · share/allow/*.txt · AGENTS.md, CLAUDE.md]
    end
    subgraph stageB[stage B · scripts/generate-site-data]
        GB[site/data/generated/*.json · site/content/** · docs/SITE_CLAIMS.md, PLAN_STATUS.md]
    end
    subgraph stageC[stage C · majordomus generate, over the index]
        GC[site/data/registry/registry.json · docs/generated/benchmarks.md]
    end
    subgraph build[scripts/site-build]
        Z[zola + Tailwind → site/public · build.json]
    end
    R --&gt; GA
    K --&gt; GA
    D --&gt; GB
    C --&gt; GB
    P --&gt; GB
    GA --&gt;|registry.json, openapi.json, cli.json| GB
    R --&gt; GC
    P --&gt; GC
    GB --&gt;|the derived documents are objects of the index| GC
    GB --&gt; Z
    GC --&gt; Z
</pre>


<div class="overflow-x-auto">

| stage | generator | reads | writes | checked by |
|---|---|---|---|---|
| A | `majordomus generate` | the Rust declarations, `share/`, the policy and the provider templates | `docs/generated/{openapi.json,registry.json,capabilities.md,cli.md,modules/*.md}`, `share/allow/*.txt`, the provider bootstraps | `majordomus generate --check` |
| B | `scripts/generate-site-data` | every canonical file (`--inputs` lists them) and stage A's `registry.json` and `openapi.json` | `site/data/generated/*.json`, `site/content/**`, `docs/SITE_CLAIMS.md`, `docs/PLAN_STATUS.md` | `scripts/generate-site-data --check` (by input hash) |
| C | `majordomus generate` | the Rust declarations and the index of the layer, which now holds the documents stage B wrote | `site/data/registry/registry.json`, `docs/generated/benchmarks.md` | `majordomus generate --check` |
| build | `scripts/site-build` | stages B and C, the templates, the assets | `site/public/**`, `site/data/build.json`, `site/static/build.json` | `scripts/site-check`, `scripts/site-probe` |

</div>


The boundary between the executable and the site is data, in both directions of reading:
the site generator reads the registry manifest for the ids it turns into routes and never
parses Rust; the templates read the registry dataset for every fact about a capability and
never restate one; the executable knows nothing of Zola. The manifest is a projection used
as an interface between stages, not a second truth: a capability exists in one place, and
`generate --check` refuses a manifest that no longer says what the declarations say.

Why stage C exists: the dataset lists every object of the layer with its size, and the
derived documents stage B writes are objects of the layer, so the dataset is derived after
them. Stage C's outputs are read by nothing but the site build, so the graph has no cycle
and a second `scripts/derive` on a clean tree changes nothing — `git status` stays empty,
which `test/cases/51_derived_artifacts_committed.sh` and the release criterion require.

`scripts/generate-site-data` reads every canonical input, normalises it once, and writes:

<div class="overflow-x-auto">

| file | from | what |
|---|---|---|
| `project.json` | `bin/majordomus`, `README.md`, `LICENSE` | name, version, tagline, licence, commands, exit codes |
| `profiles.json` | `share/skeleton/profiles/*.yaml` | every profile, every field |
| `policy.json` | `share/skeleton/policy.yaml` | the policy as structure, plus the raw text |
| `capabilities.json` | `docs/CLAIMS.yaml` | every claim; the generator fails on a missing path or an untested guaranteed claim |
| `commands.json` | `share/commands.yaml`, `docs/CLI.md`, the test cases | every shell command with its semantics, narrative and evidence |
| `catalogue.json` | `share/use-cases.yaml`, `share/applications.yaml` | the use cases and applications, cross-referenced |
| `doctrines.json` | the rule packages | every doctrine with its enforcement chain |
| `plan.json` | `.ai/repo/project/` | milestones, issues, the dependency graph, derived status |
| `openapi.json` | `docs/generated/openapi.json` | the HTTP API in the shape `api.html` renders (`scripts/lib/openapi-site.jq`) |
| `executable.json` | `docs/generated/registry.json`, `capabilities.json` | one route per module and per capability, the executable's pages, API anchors, sources on GitHub, the claims attached to each surface by the path of their implementation (`scripts/lib/executable-site.jq`) |
| `lifecycle.json` | `lib/finish.sh`, `share/skeleton/ai/repo/workflows/task-lifecycle.md`, `share/standard/majordomus/` | outcome vocabulary, divergence labels, lifecycle steps, the ten principles (the rules tagged `principle`) |
| `diagrams.json` | the files above | Mermaid source projected from data |
| `readme.json` | `README.md` | the sections the homepage renders, by heading; a renamed heading fails the build |
| `docs.json` | `docs/README.md` | the documentation index |
| `source.json` | the inputs | version, input hash, generator version, input list; no commit (see build provenance) |

</div>


`site/content/docs/*.md` is written from `docs/*.md` with a generated front matter, links
rewritten to site routes, and the whole body wrapped in a Tera `raw` block so that Tera never
templates canonical Markdown. The tag itself cannot be written out here: a literal closing
`raw` tag in a canonical document would end the wrapper the projection puts around it.
`site/content/registry/executable.md` is `apps/majordomus-cli/README.md` through the same
projection.

## Derived files, classified

<div class="overflow-x-auto">

| class | what | examples | rule |
|---|---|---|---|
| canonical | authored, the only place a fact lives | the Rust declarations, `share/*.yaml`, `docs/*.md`, `docs/CLAIMS.yaml`, `.ai/repo/**`, templates, `nav.toml`, `marketing.toml` | edited by hand |
| derived, committed | a projection reviewers see on GitHub and CI compares with its sources | `docs/generated/**`, `share/allow/**`, `AGENTS.md`, `CLAUDE.md`, `site/data/generated/**`, `site/data/registry/**`, `docs/SITE_CLAIMS.md`, `docs/PLAN_STATUS.md` | regenerated with `scripts/derive`, committed with the change that moved it; `scripts/derive-check` refuses drift |
| derived, build only | rebuilt on every build, never committed | `site/content/{docs,registry,commands,...}/`, `site/static/{app.css,js/,openapi.json,build.json}`, `site/data/build.json`, `site/public/**` | gitignored; `scripts/site-check` proves the build's own guards (`13e`, `13a`) |
| local, ephemeral | this checkout's state | `.ai/local/**`, `apps/majordomus-cli/target/` | never a source of anything the site shows |

</div>


## Where do I edit this?

<div class="overflow-x-auto">

| I want to change | edit | never edit |
|---|---|---|
| a Rust capability's title, description, schema, stability, tags, benchmark or cache policy | its `capability!` declaration under `apps/majordomus-cli/src/capability/builtin/` | `docs/generated/**`, `site/data/registry/registry.json`, any `/registry/` page |
| how a capability is exposed (MCP tool, resource, HTTP route, CLI path) | the `exposure` of the same declaration | `docs/generated/openapi.json`, `/docs/api/`, `/registry/mcp/` |
| a module's title, description or stability | its `module!` declaration | `docs/generated/modules/*.md`, `/registry/modules/` |
| the native command line: a command, an argument, a default, a value set | the clap declaration in `apps/majordomus-cli/src/cli.rs` | `docs/generated/cli.md`, `docs/generated/cli.json`, `site/data/generated/cli.json`, `/registry/cli/`, `/docs/cli/**` |
| an example of a native command, or adding a command that has none | `cli::EXAMPLES` in the same file, beside the declaration | an example written into a template, a page or `cli.md`: the pages render the argument vectors the crate's tests execute |
| the benchmark regression thresholds | `.ai/repo/benchmarks/rust/policy.yaml` | the policy table on `/registry/benchmarks/` |
| an accepted baseline | `majordomus bench baseline update` on the platform it measures | the baseline JSON by hand |
| what the executable is, owns, refuses; discovery; transports; side effects | `apps/majordomus-cli/README.md` | `/registry/executable/` |
| a shell command's category, stage, reads, writes, syntax, exit codes | `share/commands.yaml` (semantics) and `docs/CLI.md` (narrative) | `commands.json`, `/commands/<name>/` |
| a use case or an application | `share/use-cases.yaml`, `share/applications.yaml` | `catalogue.json`, its route |
| a claim | `docs/CLAIMS.yaml` and `docs/claims/<id>.md` | `capabilities.json`, `docs/SITE_CLAIMS.md`, `/guarantees/` |
| a doctrine | the rule file under `.ai/repo/rules/` or the vendored package | `doctrines.json`, `/doctrines/` |
| a milestone or an issue | `.ai/repo/project/**` | `plan.json`, `docs/PLAN_STATUS.md`, `/plan/` |
| a profile, the policy's schema | `share/skeleton/**` | `profiles.json`, `policy.json` |
| a kind or a schema | `share/kinds.yaml`, `share/schemas/`, or `.ai/repo/knowledge/` for this repository | `share/allow/*.txt`, the kinds table |
| the provider bootstraps | `.ai/repo/policy.yaml`, the provider templates | `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` |
| how a page looks | `site/templates/**`, `site/tailwind.css` | `site/data/generated/**` to make a template's life easier |
| which pages the executable's section has, or how claims attach to its surfaces | `scripts/generate-site-data`, `scripts/lib/executable-site.jq` | a route list in a template or in `nav.toml` |
| the editorial navigation | `site/data/nav.toml` — intents and their fixed pages | an entry per module or capability: those routes are enumerated by the generator, and `site-check` refuses one typed here |
| the homepage's words | `site/data/marketing.toml` | a number, a claim word |

</div>


After any edit in the left column: `just derive`, review the diff of the derived files,
`just derive-check`, commit both together.

## Markdown rendering

Canonical Markdown must stay readable on GitHub, so the site uses only syntax GitHub renders
natively. `scripts/lib/project-markdown.awk` projects three GitHub-native constructs into site
components, on the derived copy only:

<div class="overflow-x-auto">

| in the canonical file | on the site |
|---|---|
| ` ```mermaid ` fence | `<pre class="mermaid">`, rendered client-side; the source stays visible without JavaScript |
| `> [!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]`, `[!CAUTION]` | Flowbite alert |
| a pipe table | wrapped in `overflow-x-auto` so wide tables scroll inside their container |

</div>


Everything else is Zola's Markdown renderer inside a Flowbite Typography container
(`format dark:format-invert`): headings with anchors, lists, task lists, footnotes,
blockquotes, and class-based syntax highlighting (`[markdown.highlighting] style = "class"`,
themes `ayu-light` and `ayu-dark`). Zola writes the two theme stylesheets into `site/public/`;
`scripts/site-build` scopes the dark one under `.dark` so class-based dark mode selects it.
`site/content-src/render-test.md` exercises every construct and is linked from nowhere.

## Mermaid

Client-side, from the pinned npm package, vendored to `static/js/mermaid.min.js` at build.
Chosen over build-time SVG because the Mermaid CLI needs a headless browser in CI and the
site must stay reproducible from bash, Node and Zola alone. `site/diagrams.js` initialises
with `securityLevel: 'strict'`, a restrained palette for light and dark, and re-renders on
the theme toggle. Four diagrams are projected from data, never drawn by hand: the task
lifecycle from the outcome vocabulary, the policy-to-instruction-file graph from the policy's
projections, the site pipeline from the generator's input list, and the core model from the
README. A failed diagram leaves its source visible and never breaks the page.

## Zola structure

```
site/
  config.toml          base_url, class-based highlighting, heading anchors, footnotes
  content/             _index and one stub per page; docs/ and render-test.md are generated
  content-src/         hand-written pages that get the same projection as docs
  templates/
    base.html          head, theme-before-paint, navbar, footer, Flowbite init, Alpine
    components.html    Tera 2 components: badge, code, card, section_head, diagram
    index.html         homepage: the business hero, then the sections, all from generated data
    page.html          standalone page with breadcrumb and Typography body
    docs-section.html  /docs/ index from the section's pages
    docs-page.html     document with sidebar navigation
    profiles.html policy.html guarantees.html getting-started.html architecture.html
    404.html
    partials/          navbar, footer, theme-toggle, breadcrumb, profile-cards, mermaid
  data/marketing.toml  the only hand-written prose
  data/generated/      written by generate-site-data, committed, checked
  tailwind.css theme.js diagrams.js
  static/              images copied from assets/; app.css and js/ written at build
  public/              output
```

Zola 0.23 runs Tera 2: components replace macros, `import` no longer exists, array indexing
is `x[1]`, undefined variables are errors, `filter` and `slice` are gone in favour of list
comprehensions and Python-style slicing.

## Tailwind, Flowbite, Alpine

`site/tailwind.css` follows the Flowbite quickstart for Tailwind v4: `@import "tailwindcss"`,
`@import "flowbite/src/themes/default"`, `@plugin "flowbite/plugin"`,
`@plugin "flowbite-typography"`, `@source` for `node_modules/flowbite` and the templates, and
`@custom-variant dark (&:where(.dark, .dark *))` from the dark-mode guide. Flowbite v4's
semantic tokens (`bg-neutral-primary-soft`, `text-heading`, `border-default`, `rounded-base`)
are used throughout. The theme names Inter first; it is not loaded, so the stack falls to the
system font on purpose.

Flowbite JS is vendored from the pinned package. Its bundle exposes `window.initFlowbite` and
does not call it (verified in `node_modules/flowbite/dist/flowbite.js`), so `base.html` calls
it once on `DOMContentLoaded`. The navbar uses `data-collapse-toggle`; nothing else needs
Flowbite JS yet.

The homepage hero holds three business propositions (`[hero].propositions` in
`marketing.toml`): one complete message per outcome, the first being the page's `h1` and the
default. The selector is a radio group rather than Flowbite's tabs or carousel, because both of
those hide the other panels with a class until JavaScript runs: arrow keys move between the
outcomes natively, every message is in the HTML, and the checked input drives the visible
message through Tailwind's `peer-checked` (the labels) and `group-has-[#id:checked]` (the
messages) variants. The three messages share one grid cell and the inactive ones are
`invisible`, not removed, so the block keeps the height of the longest message and choosing
another never moves the buttons below it. The first message is visible unless another input
is checked, so a browser without `:has()` still shows the `h1`. The ids are positional
(`hero-p-1..3`) because Tailwind compiles only class names it can read in the template.
`scripts/site-check` proves every proposition is on the page, the first is the `h1` and the
checked default, and the hero's in-page link lands on a section.

Alpine.js is vendored and used for two things: the copy button on code snippets (`x-data`,
`x-on:click`, `x-text`) and the raw-policy disclosure on `/policy/`. Both degrade: the code
and the policy are in the HTML; only the control disappears without JavaScript.

Custom CSS: one rule, `[x-cloak]{display:none!important}`, inline in `base.html`, required by
Alpine's documentation. Inline `style=""` attributes are forbidden and `scripts/site-check`
fails on one.

## Flowbite guidance consulted

The Flowbite LLM entry point (`flowbite.com/docs/getting-started/llm/`) points at `llms.txt`,
which is an index. The pages read and followed, on 2026-09-04: getting-started/quickstart
(Tailwind v4 directives), getting-started/javascript (`initFlowbite`), customize/dark-mode
(`@custom-variant`, toggle markup and script, reproduced in `theme.js`), components/navbar,
components/card, components/badge, components/footer, components/typography. The repository's
installed `flowbite` is 4.0.2, matching the documented token vocabulary.

## Routes

Every tile on the homepage is a link to a page with its own title, description, canonical URL
and Open Graph metadata. The route classes and their sources:

<div class="overflow-x-auto">

| route | source | template |
|---|---|---|
| `/` | `readme.json`, `marketing.toml`, `lifecycle.json`, `capabilities.json`, `diagrams.json`, the `why` section | `index.html` |
| `/why/`, `/why/<slug>/` | `site/content-src/why/*.md` | `why-section.html`, `why.html` |
| `/outcomes/`, `/outcomes/<slug>/` | the hero's propositions in `marketing.toml`, one hand-written page each in `site/content-src/outcomes/*.md`; moments, commands and claims from the front matter | `outcomes-section.html`, `outcome.html` |
| `/getting-started/` | `project.json`, `policy.json`, `lifecycle.json` | `getting-started.html` |
| `/supervises/`, `/supervises/<slug>/` | `readme.json` (What it does rows, cross-linked to commands and claims by keyword) | `supervises-section.html`, `responsibility.html` |
| `/commands/`, `/commands/<name>/` | `commands.json` (each command's section of `docs/CLI.md`) | `commands-section.html`, `command.html` |
| `/profiles/`, `/profiles/<slug>/` | `profiles.json` | `profiles.html`, `profile.html` |
| `/policy/` | `policy.json` including the raw file | `policy.html` |
| `/guarantees/`, `/guarantees/<status>/`, `/guarantees/<id>/` | `capabilities.json`, `docs/claims/<id>.md` | `guarantees.html`, `status.html`, `claim.html` |
| `/limitations/`, `/roadmap/` | `readme.json` sections by heading | `readme-section.html` |
| `/architecture/` | `site/content-src/architecture.md`, `source.json`, `diagrams.json` | `architecture.html` |
| `/docs/`, `/docs/<doc>/` | `docs/*.md` listed in `docs/README.md` | `docs-section.html`, `docs-page.html` |
| `/docs/api/`, `/openapi.json` | `openapi.json` (from `docs/generated/openapi.json`) | `api.html` |
| `/docs/cli/`, `/docs/cli/<command path>/` | `cli.json` (from `docs/generated/cli.json`, itself the clap declaration and `cli::EXAMPLES`) | `docs-cli.html`, `docs-cli-group.html`, `docs-cli-command.html` |
| `/doctrines/`, `/doctrines/<slug>/` | `doctrines.json` | `doctrines-section.html`, `doctrine.html` |
| `/use-cases/`, `/use-cases/<id>/`, `/applications/`, `/applications/<id>/` | `catalogue.json` | `use-cases-section.html`, `use-case.html`, `applications-section.html`, `application.html` |
| `/plan/`, `/plan/dag/`, `/plan/<id>/` | `plan.json` | `plan-section.html`, `dag.html`, `milestone.html`, `issue.html` |
| `/evidence/`, `/map/` | `claims-graph.json`, `architecture.json` | `evidence.html`, `map.html` |
| `/registry/` | `site/data/registry/registry.json`, `executable.json` | `registry.html` |
| `/registry/executable/` | `apps/majordomus-cli/README.md`, `executable.json` (claims) | `registry-executable.html` |
| `/registry/modules/`, `/registry/modules/<id>/` | the dataset's modules, `executable.json` | `registry-modules.html`, `registry-module.html` |
| `/registry/capabilities/`, `/registry/capabilities/<slug>/` | the dataset's descriptors, benchmarks and coverage, `executable.json` (module, API anchor, source, claims, tests) | `registry-capabilities.html`, `registry-capability.html` |
| `/registry/cli/` | the dataset's `cli` (the clap declaration) | `registry-cli.html` |
| `/registry/mcp/` | the dataset's `mcp` (tools, resources, protocol) | `registry-mcp.html` |
| `/registry/benchmarks/` | the dataset's `benchmarks` (targets, coverage, policy, baselines) | `registry-benchmarks.html` |
| `/build.json` | `scripts/site-build` | — (served raw) |
| `/render-test/` | `site/content-src/render-test.md`, `noindex` | `docs-page.html` |

</div>


A capability's route slug is its id with `.` and `_` replaced by `-` (`objects.search` →
`/registry/capabilities/objects-search/`, `repository.scope_classify` →
`/registry/capabilities/repository-scope-classify/`): Zola slugifies every path that way,
so the dataset names the route Zola actually serves. The API reference's operation anchors
are HTML ids Zola never touches, so they replace only the `.` (`#op-objects-search`,
`#op-repository-scope_classify`); `executable.json` carries both spellings (`slug`,
`api_anchor`), so the two link each other without a table.

A native command's route is its path under `/docs/cli/`, one segment per word, with a
trailing slash: `["majordomus", "bench", "baseline", "update"]` is
`/docs/cli/bench/baseline/update/`. The rule is one function in the executable
(`cli::route`), the route it produced is carried in `docs/generated/cli.json`, and every
consumer downstream — the site generator, the templates, `site-check`, `site-probe` — reads
that field rather than deriving one of its own. A command that has commands under it is a
Zola section so that its children have routes beneath it, and a command that has none is a
page; which it is comes from the data, so a command that grows a subcommand changes shape by
itself. `site-check` compares the declared routes and the built ones as sets, both ways: a
command with no page fails, and a page whose command the executable no longer has fails as an
orphan.

`/docs/cli/` belongs to the *native* command line, the Rust executable's. `docs/CLI.md`
specifies the **shell** tool `bin/majordomus` — a different program, whose structured
reference is `/commands/` — and renders at `/docs/cli-specification/`. The rule that turns a
document into a route is one function in the site generator (`doc_slug`), and the route it
produced is recorded in `docs.json`, so the link rewriting, the pages and `site-check` read
it instead of each computing a slug. Do not describe `share/commands.yaml` or `docs/CLI.md`
as sources for the Rust executable: they are the shell tool's, and the two command surfaces
are documented apart on purpose.

Navigation is `site/data/nav.toml`: a handful of intents (Why, Get started, Concepts, Plan,
Trust, Executable, Reference), rendered as Flowbite dropdowns on desktop and as labelled flat
lists inside the collapsed menu on phones. It holds editorial intent — which fixed pages an
intent leads to — and never an enumeration: the modules and capabilities are listed by the
generator, and `scripts/site-check` refuses a module or capability route typed into it.
`scripts/site-check` verifies every navigation and homepage link resolves, every tile class
has its page, for the executable's section that every module and capability of the dataset
has its page and no page exists without its entry, and for the command line that the routes
under `/docs/cli/` are exactly the commands the executable declares, that each page renders
its own usage, arguments and example anchors and links up and down, and that no command
route is typed by hand into a template, the generator or the navigation. `scripts/site-probe`
then fetches every declared command route from a running server and reads the deepest command
page back.

## Responsive strategy

Mobile-first: base classes describe the phone layout, `sm:`/`md:`/`lg:`/`xl:` add columns.
Every `<pre>` and `<table>` sits in a scroll container, either an explicit `overflow-x-auto`
wrapper or a Typography container carrying `[&_pre]:overflow-x-auto` and
`[&_table]:overflow-x-auto`. Grid items that hold code carry `min-w-0`. Validation is
measured, not eyeballed: `scripts/site-probe` builds a copy under the deployment prefix, serves
it locally, loads every route in headless Chrome inside iframes of 320, 390 and 1280 px and
asserts `document.scrollWidth == clientWidth`, then exercises the mobile menu, a navigation
dropdown and the theme toggle and counts rendered Mermaid diagrams. It runs in the Pages
workflow and skips with a notice where no Chrome exists. `test/cases/09_site_mobile_first.sh`
lints the built HTML for the structural causes of overflow without a browser.

## Accessibility

One `h1` per page, `<main>`, `<nav aria-label>`, a skip link, `aria-current` on active
navigation, `aria-expanded`/`aria-controls` on the menu toggle, `aria-label` on icon-only
controls, `role="note"` on callouts, visible focus rings (`focus:ring-*`, never removed),
Flowbite's contrast tokens in both themes. Everything except the copy button and the
disclosure works without JavaScript.

## GitHub Pages deployment

The site is published by `.github/workflows/pages.yml`, which triggers directly on a push to
master whose paths can change the site and runs beside `validate.yml` rather than after it.
Its one job proves the committed `site/data/generated` is current for the tree by comparing
the tree's canonical input hash with the one `source.json` carries
(`scripts/generate-site-data --fingerprint`), renders it (`scripts/pages build`), runs every
static check over the output (`scripts/pages check`), and pushes `gh-pages`. A tree whose
committed derived data is stale is refused rather than regenerated.

Everything that decides whether a change may *merge* — the behavioural suite, the crate's
gates, coverage, the macOS suite, the benchmark check and the browser probe — runs on the same
commit in `validate.yml` and gates the `ci` status. The publication path carries what can make
the published bytes wrong and nothing else; the reasoning, the budgets and the measurements
are [`GITHUB_PAGES_PERFORMANCE.md`](@/docs/github-pages-performance.md), and [`CI.md`](@/docs/ci.md) has the
validation shape. The `rust` job runs `majordomus generate --check` and the `site` job
`scripts/generate-site-data --check`; locally `scripts/derive-check` (`just derive-check`)
runs both, and `scripts/derive` (`just derive`) regenerates every derived artifact in
dependency order.

GitHub Pages serves the `gh-pages` branch (source: branch, path `/`), and whoever pushes
that branch deploys. One script does it, `scripts/site-deploy`: it refuses a dirty tree and a
commit `origin/master` does not contain, runs the gate (`generate-site-data --check`,
`majordomus generate --check`, `site-build`, `site-check`, `--probe` on request), copies
`site/public` into a worktree of `gh-pages` with `.nojekyll`, commits it with the source
commit in the message, and pushes. GitHub's own "pages build and deployment" then serves it
in under a minute; the footer of every page names the commit it was built from, so the
deploy is verifiable with `curl`. Unchanged output is not pushed twice, and a `--dry-run`
shows the commit it would push. `test/cases/96_site_deploy.sh` runs the whole path against a
local bare remote: every refusal, the first deploy, the no-op redeploy, and the check that
`site/public` was built from HEAD.

The job of `.github/workflows/pages.yml` is the gated form of the same command: it builds and
checks the commit and then runs `scripts/site-deploy --skip-build`. A person runs
`scripts/site-deploy` (or `just site-deploy`) when the operator wants the site live without
waiting for the Actions queue, and that path — no queue in front of it at all — is why the
branch source is kept instead of the native Pages artifact flow, which would make a workflow
run the only way to publish. The procedure is `.ai/repo/skills/deploy-site/SKILL.md`. Pointing
Pages at the branch is a one-time `scripts/site-deploy --configure-pages`.

## Sync guarantee

`scripts/derive-check` is the one read-only gate; it composes the generators' own checks
and names every stale artifact before it exits:

- `majordomus generate --check` derives every projection of the registry, the schemas, the
  policy and the index again and compares byte for byte: `docs/generated/**`,
  `share/allow/**`, the provider bootstraps, `site/data/registry/registry.json`.
- `scripts/generate-site-data --check` regenerates into a temporary directory and diffs every
  generated file, `docs/SITE_CLAIMS.md` and `docs/PLAN_STATUS.md` against the committed ones;
  `source.json` is compared by its input hash, which covers every canonical file the
  generator reads, `docs/generated/registry.json` and `docs/generated/openapi.json` among
  them, so a change to the executable's code moves the site's hash.

The chain is tested edge by edge. `apps/majordomus-cli/tests/projections.rs` proves that a
descriptor changed in code moves the OpenAPI document, the reference, the manifest and the
site dataset, and only the sections that project it (an exposure removed moves the routes
and the coverage, not the tools; a description moves the descriptor everywhere), and that
the dataset's tools are the MCP surface's and its routes the document's.
`test/cases/95_executable_reference.sh` proves that a capability added to the manifest gains
its route, its index entries and its links from the site generator alone, that one removed
loses them with no orphan left, and that a manifest naming an unknown module, a missing
source file or another schema is refused. `scripts/site-check` proves the rendered side:
every module and capability of the dataset has its page, no page exists without its entry,
the CLI, MCP and benchmark pages render every command, tool, target and baseline, every
capability page links its module and its operation, and no template, generator or
navigation file names a capability by hand. `test/cases/11_site_derivation.sh` changes the
version, a profile, a principle, a policy value, a claim and a document heading in a
scratch copy and asserts each change appears in the derived data; `test/cases/10_site_data.sh`
proves a missing path or an untested guaranteed claim fails the generator;
`test/cases/51_derived_artifacts_committed.sh` proves the committed derived files match
their sources and that none embeds the commit it lands in.

The commit a page was built from is not a derived file's business: `scripts/site-build`
writes `site/data/build.json` (for the footer and the architecture page) and
`site/static/build.json`, served at `/build.json`, with the commit, whether the tree was
dirty, the site's input hash and the registry and index fingerprints; `scripts/site-check`
proves the served file names HEAD and the data the site was built from, so a deployment can
be verified from outside against the commit that was meant to deploy.

## Local development

```
npm ci                      # Tailwind, Flowbite, Alpine, Mermaid — pinned
brew install zola           # or the release binary; CI pins 0.23.4
just derive                 # every committed derived artifact, in order (scripts/derive)
just derive-check           # is every committed derived artifact current? writes nothing
just test                   # the shell suite, the Rust gates, derive-check
scripts/site-serve          # generate, build, serve at http://127.0.0.1:1111/prismatic-majordomus/
scripts/site-build          # production build into site/public/
scripts/site-check          # the static checks CI runs
scripts/site-probe          # the browser-measured checks (needs Chrome; --quick for one page per section)
SITE_PROBE_JOBS=4 scripts/site-probe   # as CI runs it: four routes at a time
```

The one workflow after a change to anything canonical — a Rust declaration, a document, a
claim, a command's semantics — is: `just derive`, review the diff of the derived files (it
is the change, seen from every projection), `just derive-check`, commit both. Forgetting
the first step is caught by the third, and by CI.

## Adding new canonical data

Add the field to the file that owns it, read it in `scripts/generate-site-data`, render it in
a template, and extend `test/cases/11_site_derivation.sh` with the change-and-assert pair.
Commit the regenerated `site/data/generated/`. Do not add a field to the JSON by hand.

## Adding new site components

Define a Tera 2 component in `site/templates/components.html`, using Flowbite markup from the
component's documentation page, with mobile classes first. Call it as `{{ <name attr="…" /> }}`.
If it needs a class Tailwind cannot see in the templates, it does not belong here.

## What must never be edited manually

`docs/generated/**`, `share/allow/**`, `site/data/registry/**`, `site/data/generated/**`,
`site/content/{docs,profiles,guarantees,supervises,commands,doctrines,use-cases,applications,plan,why,registry}/**`,
`site/content/render-test.md`, `docs/SITE_CLAIMS.md`, `docs/PLAN_STATUS.md`,
`site/static/app.css`, `site/static/js/**`, `site/static/images/**`, `site/static/openapi.json`,
`site/static/build.json`, `site/data/build.json`, `site/public/**`, `CLAUDE.md`, `AGENTS.md`.
Change the canonical file; `just derive`; rebuild.
{% endraw %}

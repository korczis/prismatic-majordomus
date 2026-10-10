# Product — the features, and the landing page as a projection of them

The product's public face — the homepage, the `/features/` pages, the capability matrix —
states what Majordomus does. Every fact on it is derived from the repository it describes:
which capabilities exist, which interfaces expose them, how many objects of each kind the
layer holds, which operational moments a feature answers, what is guaranteed and what is
only advisory. What a person writes is which parts of the product form one chapter, and in
what order. Behaviour as implemented and tested; where this document and the executable
disagree, the document is wrong and changes in the same commit. The decision is
[ADR 23](../.ai/repo/adrs/0023-product-features-are-objects-of-the-layer-and-the-landing-page-is-a-projection.md);
the rule is `project.product-surface-derived`; the directory's own contract is
[`.ai/repo/features/README.md`](../.ai/repo/features/README.md).

## The kind

```text
.ai/repo/features/<id>.md    one product feature    schema: feature/v1
```

The file name is the `id` is the slug is the route: `features/worktrees.md` is
`majordomus product show worktrees`, `majordomus://feature/worktrees`,
`GET /api/v1/product/feature?id=worktrees` and `/features/worktrees/`. Nothing registers a
feature. The source class `feature` in `.ai/repo/knowledge/sources.yaml` discovers every
file under that directory through the version-control index, the kind `feature` in
`share/kinds.yaml` says how it is read, and
`share/schemas/majordomus/feature/feature.v1.schema.json` decides whether it is valid.

## What is authored and what is derived

A feature file holds **references** and **editorial decisions**, and nothing else. The
schema sets `additionalProperties: false`, so a derived key such as `surfaces:`, `route:`
or a count is refused at validation rather than silently believed.

| In the file | Derived from it |
|---|---|
| `modules` — capability modules of the executable | the capabilities, MCP tools, HTTP routes and command-line paths behind the feature |
| `commands` — public commands of the shell tool | the command-line surface, and the stage and read-only flag of each |
| `kinds` — object kinds of the layer | how many objects of each kind this repository holds |
| `rules`, `claims` | the class of each rule, whether the tool enforces it, the status of each claim |
| `docs`, `adrs`, `use_cases` | titles, statuses and the routes the site gives them |
| `cockpit`, `web` | the Cockpit areas and the mounts of the web topology |
| `areas`, `audiences` — the why catalogue's own taxonomies | the operational moments that name any of the feature's mechanisms |
| `headline`, `summary`, `short_title`, `status`, `weight`, `featured`, `tags`, `related` | nothing: these are the editorial decisions |

The interfaces a feature is exposed through are derived and never declared: `cli` when a
module it names has a command-line path or it names a shell command, `api` when a module
has an HTTP route, `mcp` when a module has a tool or a resource or the feature names a
kind, `cockpit` when it names an area or a module with one, `docs` when it names a
document. A feature therefore cannot claim a surface the registry does not have.

## One model, every interface

`apps/majordomus-cli/src/product.rs` resolves every reference against the index, the
capability registry, the command registry, the why catalogue and the web topology, and is
the only place any of this is decided.
`apps/majordomus-cli/src/capability/builtin/product.rs` declares six capabilities over it
with `capability!`, so the command line, the HTTP routes, the OpenAPI operations, the MCP
tools and resources and the generated reference are projections of one declaration
([ADR 2](../.ai/repo/adrs/0002-canonical-capability-registry.md), `docs/CAPABILITIES.md`).

| Answer | Command line | HTTP | MCP |
|---|---|---|---|
| every feature | `majordomus product list` | `GET /api/v1/product/features` | `majordomus_features`, `majordomus://product` |
| one feature | `majordomus product show <id>` | `GET /api/v1/product/feature` | `majordomus_feature`, `majordomus://feature/<id>` |
| the domains | `majordomus product domains` | `GET /api/v1/product/domains` | `majordomus_product_domains`, `majordomus://product/domains` |
| the matrix | `majordomus product matrix` | `GET /api/v1/product/matrix` | `majordomus_product_matrix` |
| the providers | `majordomus product providers` | `GET /api/v1/product/providers` | `majordomus_providers` |
| the refusals | `majordomus product validate` | `GET /api/v1/product/validate` | `majordomus_product_validate` |

## Domains: the product's shape above the feature

A **domain** is one of the few things the product controls — context, coordination,
governance, evidence, completion and surfaces, as this repository declares them — stated once
in `.ai/repo/features/domains/<id>.md` (kind `domain`, schema `domain/v1`) with its title, a
one-sentence `headline`, a one-sentence `problem` and a `weight` that orders every listing
of domains ([ADR 104](../.ai/repo/adrs/0104-the-product-is-filed-under-a-few-domains-and-a-feature-names-its-one.md)).

A feature names exactly one domain in its own `domain` field, and a domain lists nothing:
its members are the stable features that name it, and their union of surfaces, the distinct
claims, use cases, rules and capabilities behind them and the moments they answer are
derived, never written. `product validate` refuses a domain that does not exist (with the
nearest candidate), a stable feature that names none while any is declared, and a stable
feature under a domain that is not stable; a domain no stable feature names is a warning
and is shown nowhere.

The areas of the why catalogue are not domains and are not replaced by them. An area is
where operations hurt, a feature may serve several, and moments, the diagnosis and the
Cockpit's sidebar are filed by area. A domain is where the product answers, and a feature
has exactly one.

## The public projection boundary

`majordomus generate site` writes `site/data/registry/product.json` and
`product-graph.json`. The dataset is not the model serialised: `PUBLIC_FEATURE_FIELDS` and
`PUBLIC_DOMAIN_FIELDS` in `apps/majordomus-cli/src/site.rs` are allow-lists, and each field is copied by name. A
field added to the model does not reach a published page until somebody adds it to that
list, which is the point — accidental exclusion is not a boundary, and a field nobody
allowed is not published.

What that keeps out is asserted, not assumed: the feature's Markdown body, absolute paths,
anything outside the repository. The test is `apps/majordomus-cli/tests/product.rs`, which
builds the dataset over a fixture and refuses a key nobody allow-listed, a body, a fixture
root path and a temporary directory; `scripts/generate-site-data` refuses the artifact
again before it renders anything from it.

## What the site does with it

`scripts/generate-site-data` validates the dataset — the schema is
`majordomus-site-product/v1`, published as
`share/schemas/generated/site-product.schema.json` — and writes one page per non-draft
feature under `site/content/features/`, with the feature's own Markdown as the body. The
templates read the dataset and name nothing:

| Route | Template | What it renders |
|---|---|---|
| `/` | `site/templates/index.html` | the chapters (features that declare themselves featured, in weight order), the interfaces, the matrix, the graph, the providers, the kinds and the doctrine |
| `/features/` | `features-section.html` | every non-draft feature as a card, grouped by the domain it names in the domains' weight order, with any feature no shown domain holds listed apart; the interfaces, the graph, the providers |
| `/features/<id>/` | `feature.html` | a breadcrumb through its domain, its promise, its own prose, then everything derived from its references: why it exists, how it works, where it sits, its interfaces, its evidence, how to use it, where the page came from and the features related to it |
| `/features/matrix/` | `features-matrix.html` | features against interfaces, and modules, commands and kinds against the features that name them, and how much of each feature is proven |

A feature in a list is drawn by one partial, `site/templates/partials/feature-card.html`:
its name linked to its page, its status, its one-sentence headline, the interfaces it reaches,
how many claims it names and how many of those name the test that settles them, and how many
use cases show it in use. It takes a feature of the dataset or a domain's member alike; a
count the data does not carry is left out, and a zero it does carry is said in words. A
feature page's related features come from relationships the dataset holds — the features it
names and that name it, its domain's other members, and the features that share a use case
or a claim with it — each card saying which. `scripts/site-check` (check `features`) holds
the index and the pages to the dataset: every public feature is one card on `/features/`,
in its domain's group; every card on the site names a public feature and links its page; a
feature with a domain carries it in its breadcrumb; and the templates that draw features
select none by hand and print no number of their own.
`test/cases/810_a_feature_is_explored_under_its_domain.sh` adds one feature file to a copy of
this repository, builds the site the way the build does, finds the feature's card under its
domain and its page under the domain's trail, then removes the file and finds nothing left.

`site/data/marketing.toml` is the one hand-written file: positioning sentences and button
labels, held to a line budget, carrying no number and no capability claim. Routes that
moved are declared once in `site/data/nav.toml` under `[[redirects]]` and become Zola
aliases on the page they point at.

## The terminal on the homepage

The page shows the tool running, and every transcript on it is a run that happened.

`scripts/generate-site-data` writes `site/data/generated/terminal.json` by joining two
datasets it has already written: `lifecycle.json`, the ordered lifecycle the tool declares,
and `catalogue.json`, whose `use_cases[].evidence.steps[]` carry the command line, the exit
status and the stdout of every use-case scenario the same run executed. The output was
redacted when it was captured — `lib/usecase.sh` replaces the repository path, the clock,
the identifiers and the tool versions with placeholders — so a transcript names no machine
and reproduces byte for byte on another one.

Nothing in the dataset, the generator or the template names a command:

| Field | How it is decided |
|---|---|
| the order of the tabs | the lifecycle's own order |
| which run illustrates a step | among that command's passing scenario steps: one that succeeded before one that was refused, then the longest body, then use case and step id |
| which panel opens first | the one with the most output |
| what is in the refusals | every recorded step of a lifecycle command that exited non-zero, deduplicated by command, status and body, ordered by the lifecycle |
| the sentence beside a refusal | the first `FAIL` line the run printed, else its last line, up to the reproduce command |

Two refusals are built into the generation. A lifecycle step no scenario ran fails it,
because a page that quietly drops a stage claims a lifecycle nobody walks; and a corpus with
no recorded refusal fails it, because the section that says the tool refuses may not be
empty. `scripts/site-check`'s `terminal` check then reads the built HTML in both directions:
every panel the dataset holds is on the page, every panel on the page is a run, each matched
by its command line, its exit status and its verdict line, and every panel links the scenario
it was recorded in.

The consequence worth stating: the homepage cannot show the tool doing something the tool
does not do, and it cannot keep showing something that stopped happening. A command that
stops refusing disappears from the page on the next generation, and a transcript edited by
hand fails the check rather than the reader.

## The homepage's story, and what search engines are shown

The homepage is an argument with an order, and the order is data: `site/data/homepage.toml`
lists the ids of the sections the page renders — hero, recognise, map, how, refuses, surfaces,
proof, install. It is told overview first: what this is, what goes wrong without it, what it
controls, how one task moves through it, the proof that "done" can be refused, how one
declaration reaches every interface, the evidence, and how to start. Reference-level material —
the lifecycle transcripts, the full catalogue of failure modes, every recorded refusal, a
capability on all its surfaces — stays on the page behind a disclosure rather than in the first
reading of it. Nothing about the order is decided in `site/templates/index.html`.

| Section | What it shows | Where it comes from |
|---|---|---|
| `hero` | the positioning, one install command, and the verdict a worker that said it was done received, line by line | `marketing.toml`, `distribution.json`, `challenge.json` |
| `recognise` | three failures, each a domain's `problem`; the featured moments behind a disclosure | `homepage.toml` `problems`, `product.json` domains, the Why catalogue |
| `map` | the product's domains as disclosures: the failure each answers, its features, its counts | `product.json` domains (ADR 0104), via `partials/domain-map.html` |
| `how` | the task lifecycle in its order; the recorded run of each step behind a disclosure | `terminal.json` |
| `refuses` | the challenge stepped through, and every other recorded refusal behind a disclosure | `challenge.json`, `terminal.json` |
| `surfaces` | declaration to interfaces, the model's size, the providers, one capability on every surface | `product.json`, `registry.json` |
| `proof` | every claim counted by its declared status, what a recorded run supports, where the page came from, what is not built | `docs/CLAIMS.yaml`, `product.json` `evidence`, `manifesto.toml` |
| `install` | the install, next-step and verification commands | `distribution.json` |

The long-form argument the homepage used to carry section by section is `/method/argument/`,
moved verbatim, so no sentence of `manifesto.toml` stopped being rendered.

The site is indexed by one declared policy. `[indexing] unlisted` in `site/data/nav.toml` names
the sections whose detail pages are receipts — the plan's issues and milestones, the registry's
capabilities and modules, the closed sessions. Those pages stay published and linked; they carry
a robots `noindex` (`templates/base.html`) and are left out of `sitemap.xml`
(`templates/sitemap.xml`), while each section's own index page stays indexed.

`scripts/ci/homepage-check` holds all of it, in both directions, under the blocking rule
`project.homepage-tells-a-declared-story`, and `scripts/site-check` reports its findings, so a
homepage or a sitemap that drifts from its declaration is refused before the Pages build
publishes it. `test/cases/333_homepage_narrative.sh` proves each check by breaking it.

The homepage's weight is declared in the same file, as `[budget]`: the scripts and stylesheets
it loads from the site and the graph data it carries inline. The homepage loads no graph
runtime: its map is the domains, as native disclosures, and the composed product graph is on
`/features/`, published at `/graphs/product.json` and fetched when the drawing first comes into
view; no page loads the Mermaid runtime unless it carries a diagram. The same check reports the current weight on
every build, so raising a budget is a reviewed change to `homepage.toml`, never a silent one.
The words are held too: `homepage.toml` `[promises]` declares the capability words the
hand-written `marketing.toml` and `manifesto.toml` may use only with a claim (unattended,
autonomous, automatic merge or release, and the like), each with the claim id that permits it,
and the check fails a word whose claim is absent, not guaranteed or not supported by a recorded
run, and the name of a feature the product model does not call stable.

To add a homepage section: write it in `index.html` with an `id`, put that id where it belongs
in `homepage.toml`'s `order`, and give it a link or a derived figure. To unlist a section's
detail pages: add its name to `[indexing] unlisted`.

## What is refused

- A reference that resolves to nothing. The message names the file, the key and the
  nearest candidate, which makes it a repair rather than an investigation.
- A derived key in a feature file — `surfaces`, `route`, a count, a list of moments.
- A `featured` feature that is not `stable`; a `stable` feature that names no mechanism, or
  no document, or whose body lacks `## What it does` and `## What it does not do`.
- A stale dataset: `majordomus generate --check` fails a tree whose `product.json` does not
  match the repository, so the homepage cannot describe an executable this repository does
  not have.
- A template, a generator or a navigation file that names a feature, a module, a command,
  a provider or a count by hand. `scripts/site-check` decides that.

A capability module, a public command or an object kind that no stable feature names is
**not** refused. It is reported as a gap by `majordomus product validate` and shown on the
A surface column says where a feature is reachable. It does not say what of it holds, and a
compliance table that answers only the first question invites the reader to assume the second.
So each row also carries `proven` over `claims`: how many of the claims that feature names are
guaranteed *and* name the case that settles them, out of how many it names at all. Both numbers
are derived — the claims from the feature file, the case from each claim's `test:` in
`docs/CLAIMS.yaml` — and a claim whose test is `-`, meaning planned or rejected, carries none
rather than a dash a reader would mistake for evidence. The same field reaches each feature page,
beside the claim it settles, so the evidence is one read away rather than three.

matrix, because a thing the product does that the product page does not mention is exactly
what this model exists to make visible. Closing it is one reference in one file.

## Adding a feature

```bash
cp .ai/repo/features/worktrees.md .ai/repo/features/<id>.md
$EDITOR .ai/repo/features/<id>.md       # id, headline, summary, the references, the body
git add .ai/repo/features/<id>.md       # discovery reads the version-control index
majordomus product validate             # schema, references, floors
just derive                             # the dataset, the pages and the generated docs follow
```

`test/cases/97_product_features.sh` does exactly that and undoes it, proving the feature is
answered by the command line, HTTP, MCP, the site dataset and the graph while it exists,
and by none of them once it is gone.

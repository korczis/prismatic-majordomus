+++
title = "How this site is derived"
description = "The website is a projection of the repository, built the way the tool asks projects to build their own instructions."
template = "architecture.html"
+++
{% raw %}
{% raw %}
## Ownership

<div class="overflow-x-auto" tabindex="0">

<div class="overflow-x-auto" tabindex="0">

| layer | owns | lives in |
|---|---|---|
| product truth | policy schema, profiles, worker instructions, the CLI | `share/skeleton/**`, `bin/`, `lib/` |
| public narrative | what it is, why, how, what it refuses | `README.md`, `docs/**` |
| claims | every capability, with status, source, implementation, test | `docs/CLAIMS.yaml` |
| marketing copy | headline, leads, button labels; no claims, no numbers, a line budget | `site/data/marketing.toml` |
| derived data | stable JSON the templates read | `site/data/generated/**` (committed, checked) |
| derived content | canonical Markdown with generated front matter | `site/content/docs/**` (gitignored, rebuilt) |
| presentation | Zola templates, Flowbite components, Tailwind utilities, Alpine enhancements | `site/templates/**`, `site/tailwind.css` |
| output | the static site | `site/public/**` |

</div>


</div>


## Pipeline

The pipeline is drawn at the foot of this page rather than typed here. It is rendered from
`site/data/generated/diagrams.json`, which the generator writes from the inputs it actually
read, so a canonical file added to the derivation appears in the picture without anyone
redrawing it. The block that used to sit here was a second, hand-maintained copy of that
same diagram, and the two already named different things.

## Sync guarantee

`scripts/generate-site-data --check` regenerates into a temporary directory and diffs it against the committed `site/data/generated/`. A difference fails the build. The input hash in `source.json` covers every canonical file the generator reads; it appears in the footer of every page. A regression test changes one canonical value in a scratch copy and asserts the derived data changes with it, so the derivation is proven rather than assumed.

## What is never edited by hand

`site/data/generated/**`, `site/content/docs/**`, `site/static/app.css`, `site/static/js/**`, `site/public/**`. Change the canonical file; rebuild.
{% endraw %}
{% endraw %}

+++
title = "How this site is derived"
description = "The website is a projection of the repository, built the way the tool asks projects to build their own instructions."
template = "architecture.html"
+++
## Ownership

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

## Pipeline

```
share/skeleton + docs + README + CLAIMS.yaml
        │  scripts/generate-site-data
        ▼
site/data/generated/*.json  +  site/content/docs/*.md
        │  zola build
        ▼
site/public/          ← Tailwind + Flowbite CSS, Flowbite JS, Alpine
        │  GitHub Actions (pages.yml)
        ▼
GitHub Pages
```

## Sync guarantee

`scripts/generate-site-data --check` regenerates into a temporary directory and diffs it against the committed `site/data/generated/`. A difference fails the build. The input hash in `source.json` covers every canonical file the generator reads; it appears in the footer of every page. A regression test changes one canonical value in a scratch copy and asserts the derived data changes with it, so the derivation is proven rather than assumed.

## What is never edited by hand

`site/data/generated/**`, `site/content/docs/**`, `site/static/app.css`, `site/static/js/**`, `site/public/**`. Change the canonical file; rebuild.

+++
title = "The website's skills section is rendered from the skills catalogue, one page per skill carrying the skill's own body and examples, and a removed or renamed skill leaves no page, entry or link behind"
description = "The site's skills section lists every skill, and each skill has a page of its own that renders its front matter as metadata, its own Markdown body as the page, its examples, its related skills and the path of the file it was read from. Nothing on those pages is typed into the site. When a skill is added, renamed or removed and the site data regenerated, the index, the page and every link follow, and scripts/generate-site-data --check, which CI runs before the site builds, exits 10 while the committed data is behind the files."
weight = 132
[extra]
claim_id = "skill-site-projection"
status = "guaranteed"
source = "docs/claims/skill-site-projection.md"
+++
{% raw %}

## What it means

The site's skills section lists every skill, and each skill has a page of its own that renders its front matter as metadata, its own Markdown body as the page, its examples, its related skills and the path of the file it was read from. Nothing on those pages is typed into the site. When a skill is added, renamed or removed and the site data regenerated, the index, the page and every link follow, and `scripts/generate-site-data --check`, which CI runs before the site builds, exits 10 while the committed data is behind the files.

## How it works

`scripts/generate-site-data` sources `lib/skills.sh`, runs the same examination `skills check` runs (a skill that fails it fails the build), and writes `site/data/generated/skills.json` from the catalogue — each skill with its route, its examples read from `examples/*.md` (title from the first heading, body below it), and an index by tag — then one Zola page per skill under `site/content/skills/`, front matter from the catalogue and body copied from `SKILL.md`. The content directory is replaced whole on every run, so a page whose skill is gone is gone. `site/templates/skills-section.html` and `skill.html` render from those two sources and nothing else; the sidebar of every skill page lists the section's pages, so navigation between skills is derived too.

## How to see it

```bash
scripts/generate-site-data && jq '.count, [.skills[].id]' site/data/generated/skills.json
ls site/content/skills/
scripts/site-build && ls site/public/skills/
git mv .ai/repo/skills/repo-review .ai/repo/skills/review-repo   # then change id: in SKILL.md
scripts/generate-site-data --check    # exit 10: stale
scripts/generate-site-data && ls site/content/skills/            # review-repo.md, no repo-review.md
```

## What it does not cover

The site's navigation bar names the section, not the skills; individual skills are reached from the section index and from each page's sidebar. The site has no search of its own, so skills are not indexed for search.

## Why it exists

A documentation page that describes a skill is the second copy of it, and the second copy is the one that is wrong six weeks later. Rendering the skill's own file, from the same catalogue every other surface reads, means the page can only be stale in a way the build refuses.
{% endraw %}

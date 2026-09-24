# The site build composes every published static surface from the committed topology, and refuses one whose artifact is absent

## What it means

The published site is more than what Zola renders. Every static surface the topology publishes,
other than the site itself, is copied into the build at its own mount — today that is the crate's
rustdoc at `/rustdoc` — and nothing in the build names which surfaces those are. A surface added
to the topology tomorrow is composed tomorrow. A surface the topology publishes whose producer has
not run refuses the build, names the producer, and publishes nothing, because a site that
silently lost a surface is a stale deployment nobody sees.

## How it works

`scripts/site-build` asks `mj_web_composed` in `lib/common.sh` for the composed surfaces: every
surface of `docs/generated/web.json` of kind static directory that the published site carries,
other than the application. It asks before any generation or rendering is spent, and exits `12`
naming the surface, its mount, its artifact and its producer when an artifact is absent. After
Zola has written its output and the site's accessibility pass has run, it copies each artifact to
its mount. A mount the site itself already filled is refused with exit `10` rather than
overwritten, because one path has one owner and which one a reader got would otherwise depend on
the order of two lines.

The same selection tells every check which pages are the site's own and which belong to a
composed surface, so the site's page contract judges the site's pages and `majordomus quality
rustdoc` judges the reference's. The build also writes each composed surface, with the commit its
producer declared, into `/build.json`.

## How to see it

```bash
jq -r '.surfaces[] | select(.kind == "static-directory") | [.id, .mount, .availability] | @tsv' docs/generated/web.json
scripts/site-build                          # refuses, naming the producer, until the reference exists
scripts/rust-check --doc && scripts/site-build   # "== compose rustdoc at /rustdoc", then the route count
jq .surfaces site/data/build.json
bash test/run.sh 486_rustdoc_composition
```

## What it does not cover

Whether the composed tree is complete, current or free of broken links is decided by the
surface's own check and by `scripts/site-check` over the composed output, not by the build.
Publishing the composed tree is `rustdoc-published`. The served documentation build at `/docs` is
composed the same way, and nothing here makes a running server read `site/public`.

## Why it exists

The alternative is a copy step per surface, which is the registration ADR 0013 refused: correct
the day it is written and wrong the first time a surface is added or renamed without it. Reading
the committed topology means the publication carries exactly what the topology says it publishes,
and the refusal means the only way to publish without a surface is to remove it from the
topology, which is a visible change. `test/cases/486_rustdoc_composition.sh` builds without the
artifact and with it.

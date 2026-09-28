+++
title = "The crate's rustdoc is published at /rustdoc with the site, built from the commit the site names, and is never committed"
description = "https://majordomus.dev/rustdoc/ serves the crate's reference as rustdoc rendered it, from the"
weight = 166
[extra]
claim_id = "rustdoc-published"
status = "guaranteed"
source = "docs/claims/rustdoc-published.md"
+++
{% raw %}

## What it means

`https://majordomus.dev/rustdoc/` serves the crate's reference as rustdoc rendered it, from the
same deployment as the rest of the site, and the commit it was built from is the commit the site
was built from. That is readable from outside: `/build.json` names the site's commit and, under
`surfaces`, the commit each composed surface's producer declared. The tree is generated for every
publication and is never committed to the repository.

## How it works

The rustdoc job of the pages workflow, a job of its own that the deploy job needs, produces the
tree through the composite action `.github/actions/rustdoc` from the commit being deployed and
hands it over as an artifact of the same run. The deploy job fetches it,
`scripts/site-build` composes it at `/rustdoc` from the committed
topology, `scripts/site-check` judges the composed output, and `scripts/site-deploy` pushes
`site/public` to `gh-pages` exactly as it always has — the deploy path did not change; what it
carries did.

The tree is not committed because it is large, because it embeds the commit it was built from on
the crate's `COMMIT` constant page, and because a committed copy would be stale on every commit.
Its freshness is proven by the deployed commit instead.

## How to see it

```bash
curl -fsS https://majordomus.dev/build.json | jq '{commit, surfaces}'
curl -fsS https://majordomus.dev/rustdoc/ | grep -o '<code>[0-9a-f]\{40\}</code>'
git ls-files target/web/rustdoc | wc -l        # 0: never committed
bash test/run.sh 489_rustdoc_deploy
```

## What it does not cover

The case runs the deploy against a local bare remote and reads the published branch back; it does
not reach GitHub, whose own build of `gh-pages` and whose CDN are verified by
`rustdoc-verified-live` and the `pages-live` gate. Whether every item has its page is
`rustdoc-complete`. A local `scripts/site-deploy` publishes the reference the same way, provided
the tree was produced from the commit being deployed — which `quality rustdoc` and the site's own
checks refuse otherwise.

## Why it exists

The reference existed for as long as the crate did, and nobody could read it: the gate rendered
it and discarded it, the publication job had no Rust toolchain, and every route a reader might
have tried answered 404. Publishing it is the point; publishing it from the commit the site names
is what makes the published page a statement about the code that is live, rather than about some
earlier build. `test/cases/489_rustdoc_deploy.sh` deploys a composed site and reads the reference
and its commit from the published branch.
{% endraw %}

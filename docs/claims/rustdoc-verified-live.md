# A rustdoc tree built from another commit is refused, before publication and at the public URL

## What it means

The reference is stale when it was built from a commit other than the one it is presented as. That
is refused twice. Before publication, the integrity check refuses a tree whose producer did not
build it from `HEAD`. After publication, the public verification refuses a live reference that is
absent, broken or built from another commit than the one deployed — and in the pages workflow
that verification is a hard step, so the deployment run fails.

## How it works

Two witnesses name the commit, and they do not share a writer. `built_from` in the tree's
`surface.json` is the producer's statement, carried into `/build.json` under `surfaces` by the
site build. The crate's own `COMMIT` constant page is the compiler's: rustdoc renders the
constant's value, which `build.rs` read when it last ran — `MAJORDOMUS_BUILD_COMMIT` when that
is set, the checkout's `HEAD` otherwise. `build.rs` does not rerun when only `HEAD` moves, so
the producer hands the build the commit it records, and both witnesses come from one build.

`majordomus quality rustdoc` compares `built_from` with `HEAD`. `scripts/pages verify-rustdoc
--commit SHA` reads the public site once it serves the deployed commit: the `rustdoc` surface in
`/build.json` and `/rustdoc/surface.json` built from that commit, the landing page naming it, the
library's index answering as rustdoc's with its assets, and the constant page naming that
commit. It is its own function beside `verify`,
which still checks `/build.json` alone. `scripts/ci/pages-check` runs the same verification as a
section of the `pages-live` gate, on every full validation and at `finish`.

## How to see it

```bash
majordomus quality rustdoc                      # built_from against HEAD
curl -fsS https://majordomus.dev/build.json | jq '.surfaces'
scripts/pages verify-rustdoc --commit "$(git rev-parse origin/master)" --timeout 0
scripts/ci/pages-check
bash test/run.sh 487_rustdoc_staleness
```

## What it does not cover

It does not wait on GitHub: whether the site serves the deployed commit at all is `scripts/pages
verify`, whose wait is external latency and is reported rather than enforced. It does not judge
whether every item has its page — `rustdoc-complete` does, before publication. A run cancelled
before its verification step reports nothing; `pages-live` asks again on the next validation.

## Why it exists

The site's only public verification read `/build.json`, so a deployment that had lost the whole
reference, or carried one from an older build, would have verified. Generation succeeding is not
evidence that the right thing was deployed, and a reference that is wrong at its public URL is a
failed deployment in the same sense as GitHub's own build of `gh-pages` erroring, which the
workflow already makes red. `test/cases/487_rustdoc_staleness.sh` builds a tree from one commit and
presents it as another.

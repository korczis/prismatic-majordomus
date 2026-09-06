# Every platform, artifact name and installation URL is derived from one model, and a projection that disagrees with it is refused

## What it means

`share/distribution.yaml` declares the binary, the repository releases come from, the
installer's canonical URL and defaults, the archive naming function, and every target with
its operating system, architecture, C library, Rust target triple, status and the runner a
release builds it on. A target it does not declare does not exist — not for the release
build, not for the installer, not for the documentation, not for the website.

Everything else is a projection: the build matrix in `docs/generated/distribution-matrix.json`,
the platform table inside the published `install.sh`, the platform table in
`docs/INSTALL.md`, the dataset the website renders, and the public metadata of every
recorded release. None of them states a platform of its own, and none of them is written by
hand.

## How it works

`majordomus generate --target distribution` writes all of them from the model and the
release records under `.ai/repo/releases/`. `--check` compares each with what is committed
and exits 10 naming any that differs, which is reached by `just derive-check`, by the CI
gate the `site` job runs, and by the release pipeline before a single artifact is built.

The naming function exists once, as `Target::artifact_name`, and every caller reaches it
through `majordomus distribution artifact` — including the packaging script, which asks for
the name rather than composing one.

## How to see it

```bash
majordomus distribution                     # the install command, the defaults, the tally
majordomus distribution targets             # every declared platform and its status
majordomus distribution validate            # every invariant, exit 10 with each violation
majordomus generate distribution --check    # every projection, exit 10 if any is stale
```

## What it does not cover

That a declared platform *works* is proved by building it, not by declaring it: the release
builds every supported target and verifies each archive on the runner it was built on. The
model is what makes the set of platforms one fact rather than five.

## Why it exists

A supported platform has to be true in the build matrix, in the installer's table, in the
documentation, on the website and in what the tool says about itself. Written down five
times it is wrong within a release, and the person who discovers it is the person who
cannot install the tool. Adding a platform here is one edit and a regeneration; the
behavioural case proves that by adding one to a copy of the model and requiring it to
appear everywhere.

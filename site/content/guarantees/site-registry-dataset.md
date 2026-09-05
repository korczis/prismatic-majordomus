+++
title = "The site's Registry page is rendered from site/data/registry/registry.json, a dataset majordomus generate site derives from the registry and the index with their fingerprints, and generate --check refuses a stale one in CI"
description = "GitHub Pages has no content model of its own for the registry. /registry/ reads site/data/registry/registry.json; the Rust executable writes that file from the capability registry and the index (majordomus generate site), and majordomus generate --check, which CI runs on every push, derives it again and exits 10 when the committed file differs. A number on that page that disagrees with the executable is a stale generated file, and the build refuses it."
weight = 116
[extra]
claim_id = "site-registry-dataset"
status = "guaranteed"
source = "docs/claims/site-registry-dataset.md"
+++
{% raw %}

## What it means

GitHub Pages has no content model of its own for the registry. `/registry/` reads `site/data/registry/registry.json`; the Rust executable writes that file from the capability registry and the index (`majordomus generate site`), and `majordomus generate --check`, which CI runs on every push, derives it again and exits 10 when the committed file differs. A number on that page that disagrees with the executable is a stale generated file, and the build refuses it.

## How it works

`apps/majordomus-cli/src/site.rs` builds one dataset (`majordomus-site-registry/v1`): the generator's identity and version; the registry's fingerprint and counts with every builtin capability summarised (id, kind, exposures, stability, benchmark and cache policy, provenance) and every module with what it composes; the index's fingerprint, state, sections, counts by kind and every object without its content (URI, kind, identity, title, path, section, source class, size, tags); every kind the executable reads with its format, front-matter rule, schema and identity fields; and the provider projections the policy declares. Everything is sorted, nothing carries a timestamp, an absolute path or the commit (the site's own `source.json` carries the commit and is written at build time).

The directory is the Rust executable's alone. The shell site generator (`scripts/generate-site-data`) replaces `site/data/generated/` wholesale and reports a file it did not write as an orphan, so the registry dataset lives beside it, not inside it: no directory has two writers.

## How to see it

```bash
apps/majordomus-cli/target/debug/majordomus generate site --check      # OK generated site/data/registry/registry.json
jq '.registry.fingerprint, .registry.summary.total, .index.by_kind' site/data/registry/registry.json
printf '\n' >> site/data/registry/registry.json
apps/majordomus-cli/target/debug/majordomus generate --check           # exit 10: site/data/registry/registry.json (differs)
git checkout site/data/registry/registry.json
```

## What it does not cover

The dataset lists the builtin capabilities in full and the declarative ones as the objects they are resources of; it does not carry contents, schemas or benchmark results. The rest of the site (claims, plan, roadmap, documents) is still derived by `scripts/generate-site-data` from the files that own those facts; this dataset is the registry's slice, not a replacement of that generator.

## Why it exists

The repository's rule that interfaces are projections of the registry (`project.interfaces-are-projections`) applied to the site: before this the pages showed nothing of the registry, and a page that did would have had to hand-maintain a second list of operations and kinds. `apps/majordomus-cli/tests/projections.rs` builds the whole plan twice over a fixture and compares, refuses the checkout path in any artifact, adds one rule to the layer and sees the dataset's fingerprint move and the bootstraps stay, changes the policy and sees the opposite, and sees `check` name exactly the artifact each change concerns.
{% endraw %}

# The public dataset carries only fields somebody allowed

## What it means

`site/data/registry/product.json` is not the product model serialised. It is a field-by-field
copy through an allow-list, so a feature's Markdown body, an absolute path, a local username
or a field nobody named cannot reach a published page. A field added to the model is absent
from the website until somebody adds it to that list.

## How it works

`PUBLIC_FEATURE_FIELDS` in `apps/majordomus-cli/src/site.rs` names every field a public
feature may carry, and the generator copies by name rather than excluding by attribute:
accidental exclusion is not a boundary, and a `skip_serializing` that somebody forgets is the
failure mode this shape removes. The feature's `body` is deliberately absent — the prose the
site renders is read from the feature's own file by the site generator, at the path the
dataset names, not carried through the dataset.

`scripts/generate-site-data` refuses the artifact again before it renders anything from it: a
dataset that is not `majordomus-site-product/v1`, that does not validate, that carries a
feature body, or that carries a machine path fails the generation.

## How to see it

```bash
majordomus generate site
jq '.features[0] | keys' site/data/registry/product.json
grep -c '"body"' site/data/registry/product.json          # 0
cargo test -p majordomus-cli --test product
```

The test builds the dataset over a fixture repository whose root is a temporary directory, and
asserts that every key was allow-listed, that no feature carries a body, that the fixture root
and `/tmp/` appear nowhere, that the telemetry equals the registry's and the index's own
counts, that the published schema validates the document, and that two runs are byte-identical.

## What it does not cover

The allow-list decides what *shape* may be published, not whether a particular value is
sensitive. A field named in the list carries whatever the model put in it, so a field that
could hold a secret must not be added — which is what makes the list the review point.

## Why it exists

A public page derived from a repository's own internals is a projection boundary, and a
boundary that works by leaving things out is a boundary that fails the first time somebody
adds a field. Naming what may cross it inverts the default: the safe outcome is what happens
when nobody thinks about it.

+++
title = "Every reference a feature makes resolves against the registry that owns it, and one that does not is an error naming the file, the key and the nearest candidate rather than a page linking to nothing"
description = "Every reference a feature makes is typed and resolves against the registry that owns it: a"
weight = 165
[extra]
claim_id = "product-references-resolve"
status = "guaranteed"
source = "docs/claims/product-references-resolve.md"
+++
{% raw %}

## What it means

Every reference a feature makes is typed and resolves against the registry that owns it: a
capability module of the executable, a public command of the shell tool, an object kind of
the layer, a rule, a document, a decision, a claim, a use case, a Cockpit area, a surface of
the web topology. A name that resolves to nothing fails `majordomus product validate` with
the file, the front-matter key and the nearest candidate; a page that linked to nothing is
what that refusal prevents.

The other direction is reported rather than refused. A capability module, a public command or
an object kind that no stable feature names is a gap: `product validate` warns and the matrix
shows the row empty. A thing the product does that the product page does not mention is what
the model exists to make visible, so it is never quietly absent.

## How it works

`apps/majordomus-cli/src/product.rs` resolves each reference against the index, the capability
registry, the command registry, the Why catalogue and the web topology, and computes the
nearest candidate by edit distance over the ids that registry actually holds. The floors a
stable feature is held to are checked in the same pass: it names at least one mechanism and
at least one document, its body carries `## What it does` and `## What it does not do`, and
only a stable feature may be featured.

## How to see it

```bash
majordomus product validate                     # every finding with its file, key and correction
majordomus product matrix                       # the coverage gaps, shown rather than hidden
cargo test -p majordomus-cli --test product
bash test/run.sh 97_product_features            # a typo'd module is refused with `did you mean`
```

## What it does not cover

Nothing here decides whether a reference is the *right* one — only that it names something
this repository has. A feature naming a rule that exists but does not hold it is a review
question, not a validation error.

## Why it exists

The failure this refuses is silent. A card on a page that named a command nobody had written
was a link that went nowhere until a reader found it, and the reader was the first check.
Resolving the reference at validation time moves that check to the person who made the
change, with the correction already in the message.
{% endraw %}

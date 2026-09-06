+++
title = "A decision states what it put in force as typed references, each one validated, and the reverse direction — what a rule, a document, an implementation or a case was decided by — is read from the knowledge graph rather than written down a second time"
description = "A decision that changed the repository is only useful if a reader can get from it to what"
weight = 140
[extra]
claim_id = "adr-traceability"
status = "guaranteed"
source = "docs/claims/adr-traceability.md"
+++
{% raw %}

## What it means

A decision that changed the repository is only useful if a reader can get from it to what
it changed, and back. The record states the forward direction once, in `related`:

```yaml
related:
  - rule:majordomus.context-integrity
  - claim:context-coverage
  - file:lib/context_docs.sh
  - test:test/cases/69_context_documents.sh
```

The reverse — which decision put this rule in force, which decision this case proves — is
not written anywhere. It is the same graph read backwards. Two hand-maintained edges are
one edge and one thing to forget; the tool keeps the pair honest by deriving the second.

## How it works

`related` is a list of typed references, and the type says where the target lives:
`rule:<id>` in the effective rule set, `claim:<id>` in `docs/CLAIMS.yaml`, `file:<path>`
and `test:<path>` in the repository. `majordomus adr check` and the `adr` doctrine refuse a
rule the set does not have, a claim the matrix does not have and a path that does not
exist, naming the record and the reference.

The extractor turns each reference into an edge of the knowledge graph, with the
front-matter key that stated it as provenance: a rule is `declares`, a claim is `supports`,
a case is `tested_by`, a file is `references`. Those are the graph's own relation names,
not a second vocabulary invented for decisions. An edge whose target no node matches is a
finding, so a rename that breaks a reference is reported rather than quietly dropped.

`related` is what the decision put in force. Where it came from is `provenance.derived_from`
— a different question, with its own reference vocabulary, and the two are not merged.

## How to see it

```bash
majordomus adr show adr-0011                        # the record, with what it names
majordomus adr check                                # every reference resolves
majordomus knowledge edges --type declares          # decisions to the rules they put in force
majordomus knowledge edges | grep adr:adr-0011      # one decision's edges, with provenance
```

## What it does not cover

Nothing asserts that a decision names everything it touched: a reference is a claim a
person made, and its absence is not a finding. A decision may also be recorded before
anything implements it, which is the ordinary case for a proposal, so an empty `related` is
valid. What the tool guarantees is that what a record does name exists, and that the
reverse direction never disagrees with the forward one — because there is only one.

## Why it exists

The repository had decisions and it had rules, claims, code and cases, and the only link
between them was prose. Following it meant grepping. Traceability that lives in a reader's
memory is traceability nobody audits, and the first rename breaks it silently.
{% endraw %}

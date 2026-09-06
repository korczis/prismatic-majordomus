+++
title = "Every reference a moment makes resolves against the thing it names, and one that does not is an error carrying the nearest candidate"
description = "A moment names things: the audiences that recognise it, the operational areas it falls"
weight = 137
[extra]
claim_id = "why-references-resolve"
status = "guaranteed"
source = "docs/claims/why-references-resolve.md"
+++
{% raw %}

## What it means

A moment names things: the audiences that recognise it, the operational areas it falls
under, other moments, the commands and capabilities that answer it, the claims that say what
is guaranteed, the rules that govern it and the use cases that show the way out. Every one
of those names is checked against the thing it names, and a name that resolves to nothing is
an error — with the nearest existing name offered when the value looks like a typo of one.

```text
.ai/repo/why/moments/two-agents-one-bug.md:audiences
unknown audience reference: "ai-nativ-team"
did you mean: ai-native-team
```

## How it works

`majordomus why validate` resolves each reference against its own registry: audiences, areas
and related moments against the catalogue; commands against the command registry; capability
ids against the executable's registry; claims against `docs/CLAIMS.yaml`; rules against the
effective rule set; use cases against the layer's use-case section. The candidate is the
nearest by edit distance, within a bound that scales with the length of the value.

The same command reports two other classes of error: a file whose name disagrees with the
`id` in it, naming the file it should be; and two files claiming one identity, which the
index excludes and the catalogue reports rather than reporting itself valid over the
objects that were removed from under it.

## What it does not cover

It checks that a name resolves, not that naming it was right. A moment may name a claim that
has nothing to do with it and validate.

## Evidence

`apps/majordomus-cli/tests/why.rs` writes a moment naming an audience that does not exist
and asserts the finding, its field and the candidate offered; it does the same for a related
moment that does not exist, for a file whose name disagrees with its id, and for two files
claiming one identity. `test/cases/98_why_catalogue.sh` proves the same three from the
command line, with the exit code.
{% endraw %}

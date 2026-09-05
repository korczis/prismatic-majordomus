+++
title = "Provider instruction files are generated from the one policy, deterministically"
description = "CLAUDE.md, AGENTS.md, GEMINI.md and any other target the policy names are not written by hand. majordomus update generates them from one canonical policy, the profiles, and one shared body text, through a small template per provider. Running it twice with the same inputs produces byte-identical files."
weight = 19
[extra]
claim_id = "projection-generation"
status = "guaranteed"
source = "docs/claims/projection-generation.md"
+++
{% raw %}

## What it means

`CLAUDE.md`, `AGENTS.md`, `GEMINI.md` and any other target the policy names are not written by hand. `majordomus update` generates them from one canonical policy, the profiles, and one shared body text, through a small template per provider. Running it twice with the same inputs produces byte-identical files.

## How it works

`lib/update.sh` hashes the policy and profiles, renders each target from its adapter — the templates under the distribution's `share/providers/`, or the repository's own override under its providers section — and writes each target atomically. Every generated file is a bootstrap: it names `README.md`, points at `.ai/README.md`, names the default profile and the lifecycle workflow, and carries no rule of its own. Its first line is a stamp naming the command that produced it, the policy hash it came from and the hash of its own content. Adapters translate; they do not add rules — a rule that exists for one provider and not another is a policy bug by definition, and `doctor` fails a generated file that has grown a rule corpus.

## How to see it

```bash
majordomus update && shasum CLAUDE.md
majordomus update && shasum CLAUDE.md   # identical hash; output says "unchanged CLAUDE.md"
```

## What it does not cover

v0.1 projects a deliberately narrow subset of the policy — the budget, the profile table, state locations, the finish contract, the identity-fields rule — and lists what is not projected. It does not merge an existing hand-written instruction file; you decide what moves into the body on first use.

## Why it exists

In one workspace of about twenty repositories, the instruction files for two AI tools in the same repository shared between none and a tenth of their content. They were not duplicates; they were disjoint rulebooks, and which one applied depended on which tool was open.
{% endraw %}

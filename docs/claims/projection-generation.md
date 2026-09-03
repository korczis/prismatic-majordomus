# Provider instruction files are generated from the one policy, deterministically

## What it means

`CLAUDE.md`, `AGENTS.md`, `GEMINI.md` and any other target the policy names are not written by hand. `majordomus update` generates them from one canonical policy, the profiles, and one shared body text, through a small template per provider. Running it twice with the same inputs produces byte-identical files.

## How it works

`lib/update.sh` hashes the policy and profiles, builds the fragments every projection shares (the profile table, the finish contract, the required handover sections), expands the shared body `providers/body.md`, wraps it in the provider template, and writes each target atomically. Every generated file begins with a header naming the command that produced it and the policy hash it came from. Adapters translate; they do not add rules — a rule that exists for one provider and not another is a policy bug by definition.

## How to see it

```bash
majordomus update && shasum CLAUDE.md
majordomus update && shasum CLAUDE.md   # identical hash; output says "unchanged CLAUDE.md"
```

## What it does not cover

v0.1 projects a deliberately narrow subset of the policy — the budget, the profile table, state locations, the finish contract, the identity-fields rule — and lists what is not projected. It does not merge an existing hand-written instruction file; you decide what moves into the body on first use.

## Why it exists

In one workspace of about twenty repositories, the instruction files for two AI tools in the same repository shared between none and a tenth of their content. They were not duplicates; they were disjoint rulebooks, and which one applied depended on which tool was open.

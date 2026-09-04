# Provider instruction files are thin bootstraps that point at the AI layer and carry no rule of their own

## What it means

`CLAUDE.md`, `AGENTS.md`, `GEMINI.md` and any other target the policy names are generated, and each says the same short thing: read `README.md`, then `.ai/README.md`, follow its discovery protocol, run `majordomus context`, and find the lifecycle under the layer's workflows. No rule lives in a provider file. A person reaches the layer through `README.md` and `AGENTS.md`; a worker reaches it through its own bootstrap; both arrive at the same `.ai/`.

## How it works

`lib/update.sh` renders each target from an adapter under the distribution's `share/providers/` (or the repository's own override under its providers section), stamps it with the policy hash and its content hash, and writes it atomically. The `majordomus.bootstrap-integrity` doctrine, dispatched from `doctor` and `watch`, proves the chain on every run: `README.md` names `AGENTS.md`, every projection names `.ai/README.md`, and a generated file that has grown a rule corpus of its own fails. The always-loaded projection also stays within the policy's line budget, which is what keeps a bootstrap a bootstrap.

## How to see it

```bash
majordomus update
grep -l '\.ai/README\.md' CLAUDE.md AGENTS.md GEMINI.md     # all three
majordomus doctor | grep bootstrap                            # OK bootstrap README.md — names AGENTS.md; OK bootstrap 3 projection(s) — each points at .ai/README.md
printf '\n### Rules\n\n- **English only** in code.\n' >> AGENTS.md
majordomus doctor   # FAIL bootstrap AGENTS.md — carries a rule corpus of its own (a profile table, rule bullets or a rules section); rules live under .ai/repo/rules/
```

## What it does not cover

A bootstrap can only point. Whether the worker follows the pointer, reads the rules and runs `context` is not observable from outside the worker, which is why the rules themselves and the finish contract, not the bootstrap, are what enforcement rests on.

## Why it exists

Two copies of a rulebook drift, and the copy in the provider file is the one nobody reviews. When the provider body carried the rules, every rule change meant regenerating every projection and trusting that no provider had been hand-edited in the meantime. With the rules in the layer and the projections reduced to pointers, there is one place a rule can be, and `test/cases/03_update.sh` proves the projections say where it is.

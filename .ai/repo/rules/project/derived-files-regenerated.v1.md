---
id: project.derived-files-regenerated
version: 1
kind: rule
title: Derived files are regenerated, never edited
description: A generated file is changed by changing its canonical source and rerunning the generator, and both are committed together; a commit that leaves the derived data behind its inputs is refused before it exists.
statement: A generated file is changed by changing its canonical source and rerunning the generator, and both are committed together; the fingerprint gate refuses the commit that does not.
status: active
class: blocking
depends_on: []
tags: [derived]
---

# Rationale

A hand edit to a generated file is the two-rulebooks failure in miniature; docs/GITHUB_PAGES_ARCHITECTURE.md lists what is generated and CI refuses a tree whose derived files are stale.

Refusing it in CI turned out not to be enough. The publication path refuses a tree whose committed site data does not hash to its canonical inputs, so a merge that skipped the regeneration does not break the person who made it — it breaks whoever lands next, who has to run a generation they did not cause and cannot shorten. That happened three times in one hour on 2026-09-06, and each repair was overtaken by the next landing. A gate that only speaks after the merge is a gate that bills the wrong person.

The check itself was never the difficulty: `scripts/pages current` compares the recorded input hash with this tree's in a fifth of a second, because the generator writes that hash into `site/data/generated/source.json` and the same list can be hashed again without regenerating anything. What was missing was the moment: it now runs where the commit is made.

# Required behaviour

A generated file is changed by changing its canonical source and rerunning the generator, and both are committed together.

# Failure behaviour

`scripts/pages current` exits 10 and the commit does not happen, naming both hashes and the command that repairs it. The gate is declared in the policy's `enforcement` list as `derived-current`, wired by `git-hook:pre-commit`, so `doctor` reports it as a wiring finding and exits 10 when the hook stops invoking it or swallows its exit code — the rule is therefore enforced twice over: the gate refuses the stale commit, and the doctrine refuses the repository that unwires the gate.

The validator stays out of `lib/`: which files are canonical inputs, and where their hash is recorded, are facts about this repository rather than about every installation of the tool. Declaring the gate as an enforcement entry is what lets `doctor` hold it without the shipped executable learning this repository's layout.

`scripts/derive-check` remains the exhaustive form — `majordomus generate --check` and `scripts/generate-site-data --check` together, naming every stale artifact of either generator — and CI and the Pages workflow still run it. The fingerprint gate is the cheap one that can afford to run on every commit.

# Verification

`test/cases/56_derived_current_gate.sh` proves it by mutation: a canonical input moved without a regeneration makes `scripts/pages current` exit 10 and name both hashes, the same tree regenerated passes, and a hook that stops invoking the gate makes `doctor` exit 10 with the wiring finding. `scripts/derive-check` — `majordomus generate --check` and `scripts/generate-site-data --check` together — runs in CI and in front of every Pages deploy, and `scripts/derive` regenerates every derived file in dependency order; test/cases/51_derived_artifacts_committed.sh proves the committed derived files match their sources and that none embeds the commit it lands in; test/cases/95_executable_reference.sh proves a capability that joins or leaves the registry manifest gains or loses its site route from the generator alone; the majordomus.projection-integrity rule covers the generated instruction files.

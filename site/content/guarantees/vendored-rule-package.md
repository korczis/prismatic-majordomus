+++
title = "The rule baseline is vendored into the repository with a manifest naming every file and its hash, and a hand edit is detected and refused"
description = "The rules Majordomus ships are not read from the executable at run time. init copies the package from share/standard/majordomus/ into .ai/repo/rules/vendor/majordomus/, manifest included, and from then on the repository's copy is the one that applies. The manifest names every rule file with its identity and its content hash, so an edit under vendor/, a listed file that is missing, or a stray file beside the manifest is detected by name. A newer executable reports that it ships a different package and never applies it; the baseline changes only when majordomus rules vendor update is asked for."
weight = 70
[extra]
claim_id = "vendored-rule-package"
status = "guaranteed"
source = "docs/claims/vendored-rule-package.md"
+++
{% raw %}

## What it means

The rules Majordomus ships are not read from the executable at run time. `init` copies the package from `share/standard/majordomus/` into `.ai/repo/rules/vendor/majordomus/`, manifest included, and from then on the repository's copy is the one that applies. The manifest names every rule file with its identity and its content hash, so an edit under `vendor/`, a listed file that is missing, or a stray file beside the manifest is detected by name. A newer executable reports that it ships a different package and never applies it; the baseline changes only when `majordomus rules vendor update` is asked for.

## How it works

`mj_rules_manifest_check` in `lib/rules.sh` compares each listed file's hash and identity against the manifest and lists what is present but unlisted. `rules vendor status` prints the vendored and shipped revisions, the integrity of the vendored copy (exit `10` on any problem) and whether the two packages are the same (exit `11` when the executable ships a different one). `rules vendor update` refuses (`15`) over a vendored copy that fails its manifest unless `--force`, stages the new package beside the target and swaps it in atomically, writes a `rules.vendored` ledger event, and never touches `rules/project/`. `update`, `doctor` and `check` do not write under `vendor/` at all.

## How to see it

```bash
echo "a sentence added by hand" >> .ai/repo/rules/vendor/majordomus/rules/scope-integrity.v1.md
majordomus rules vendor status   # integrity: rules/scope-integrity.v1.md differs from its manifest hash (hand-edited?)   exit 10
majordomus rules vendor update   # refused: the vendored package was hand-edited …; review with: majordomus rules vendor diff, then --force   exit 15
majordomus rules vendor update --force
majordomus rules vendor status   # state: current
```

## What it does not cover

The manifest proves the vendored files are the ones the package shipped; it says nothing about whether the package is the right version for the repository, which is a decision `rules vendor diff` informs and a person makes. A project rule under `rules/project/` is the repository's own and is not under any manifest.

## Why it exists

A baseline that lives in the executable changes whenever the executable does, silently, on every machine at a different time. Vendoring makes the rule set a fact of the repository, reviewable in a diff and changed on purpose. `test/cases/67_rule_dag.sh` edits, removes and adds files under `vendor/`, hands the repository to a newer distribution, and proves that nothing changes until `update` is asked for and that `rules/project/` survives it byte for byte.
{% endraw %}

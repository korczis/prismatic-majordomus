+++
title = "A new enforced rule is a doctrine, not an inline check"
description = "A new enforced rule is a doctrine, not an inline check"
weight = 115
[extra]
kind = "rule"
slug = "project-rule-is-a-doctrine-1"
identity = "project.rule-is-a-doctrine@1"
status = "active"
source = ".ai/repo/rules/project/rule-is-a-doctrine.v1.md"
+++
{% raw %}

## Rationale

A check that a command calls by name is enforcement nobody declared; the dispatcher reads the declared rules, so a rule added there runs from that moment and one whose validator is missing fails doctor. The same argument applies one step out: a rule proved by a behavioural case, with the case named only in English in its Verification section, is a relation nothing holds — the case can be renamed and the rule goes on reading as proven.

## Required behaviour

A rule the tool enforces declares an x-majordomus block, in the mode that matches how it is enforced. A rule a command runs is dispatched: it names a validator `mj_validate_<name>`, with the category, exit code and enforcing commands the dispatcher needs. A rule a gate proves is gated: it names the tests and no validator, and nothing dispatches it. Either way it names at least one test. No command selects checks by hand, and no rule states its proof only in prose.

## Failure behaviour

The loader refuses a block that is neither mode, naming the missing half, and the set does not resolve. A dispatched rule whose validator function is absent fails doctor. A blocking rule that names no proof at all is new debt that `scripts/ci/rule-proof-check` refuses, and a rule naming a test that is not in the tree fails it outright, because such a rule reads as proven and is not.

## Verification

`majordomus doctor` walks the dispatched chain; `test/cases/18_doctrine_wiring.sh` mutation-tests it by breaking each link. `scripts/ci/rule-proof-check` holds the gated relation, and `test/cases/125_rule_proof.sh` mutation-tests both modes, the loader's refusal of a half-declaration, and the ratchet.
{% endraw %}

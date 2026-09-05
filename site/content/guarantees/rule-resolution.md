+++
title = "The effective rule set is resolved as a dependency graph in a deterministic order, and a set that does not resolve is applied by nothing"
description = "A rule is a Markdown file with YAML front matter: identity (id and version, never the file name), class, status, depends_on as exact id@version references, and for the rules the tool enforces an x-majordomus block naming the validator, the commands and the tests. The effective set is every active vendored rule plus every active project rule, ordered so that each dependency precedes the rule that depends on it. Any defect in that graph stops the command that needed the set, with the reason, and no rule is applied partially."
weight = 71
[extra]
claim_id = "rule-resolution"
status = "guaranteed"
source = "docs/claims/rule-resolution.md"
+++
{% raw %}

## What it means

A rule is a Markdown file with YAML front matter: identity (`id` and `version`, never the file name), class, status, `depends_on` as exact `id@version` references, and for the rules the tool enforces an `x-majordomus` block naming the validator, the commands and the tests. The effective set is every active vendored rule plus every active project rule, ordered so that each dependency precedes the rule that depends on it. Any defect in that graph stops the command that needed the set, with the reason, and no rule is applied partially.

## How it works

`mj_rules_load` in `lib/rules.sh` reads the vendored package in manifest order and the project rules in file-name order, validates each file (required fields, `kind: rule`, an integer version, a known status and class, a dotted id, only the keys `share/allow/rule.txt` allows, well-formed references, a complete `x-majordomus` block when one is present), then resolves the graph with a topological sort that walks the declared order wherever the graph leaves a choice, so two runs produce the same order. A dependency no rule provides, a dependency on a deprecated rule, a cycle, one `id@version` claimed by two files and a project rule whose id is in the `majordomus.` namespace are each refused by name; there is no override. Every command that reads rules — `rules list`, `rules show`, and the dispatcher behind `check`, `doctor`, `finish` and `watch` — goes through this one loader and stops with exit `10` when it fails.

## How to see it

```bash
majordomus rules list                                  # one line per rule, dependencies first
cat > .ai/repo/rules/project/cyclic.md <<'EOF'
---
id: project.cyclic
version: 1
kind: rule
title: Cyclic
description: Depends on itself.
statement: Nothing.
status: active
class: advisory
depends_on: [project.cyclic@1]
---
EOF
majordomus rules list                                  # majordomus: rules do not resolve: dependency cycle among project.cyclic@1   exit 10
```

## What it does not cover

Resolution proves the graph is sound, not that the rules are good. A rule without `x-majordomus` is normative for whoever reads it and enforced by nobody, and `rules list` says so in words rather than implying otherwise. Whether each enforced rule is actually wired to its validator, its commands, its test and CI is the separate chain `doctor` walks.

## Why it exists

A rule set with a hole in it — a dependency that never loaded, two files disagreeing about one identity, an order that changes between machines — enforces something, and nobody can say what. Failing closed on the whole set is the only way the phrase "the effective rules" means one thing. `test/cases/67_rule_dag.sh` breaks the graph one fact at a time and proves each break is refused by name and undone cleanly.
{% endraw %}

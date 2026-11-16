+++
title = "Every domain of the registry is a skill, derived and shown wherever skills are"
description = "Every domain of the registry is a skill, derived and shown wherever skills are"
weight = 110
[extra]
kind = "rule"
slug = "project-every-domain-is-a-skill-1"
identity = "project.every-domain-is-a-skill@1"
status = "active"
source = ".ai/repo/rules/project/every-domain-is-a-skill.v1.md"
+++
{% raw %}

## Rationale

A skill listing that shows some of what the repository offers teaches a person that the rest
does not exist. ADR 0165 derives one skill per domain of the registry so that the listing is
complete without being a copy of every tool, and so that adding a capability to a module is
the only act needed for it to be reachable by name.

## Required behaviour

The editors plan (`apps/majordomus-cli/src/editors/`) derives `.claude/skills/mj-<module>/SKILL.md`
for every module with a public capability that has an MCP tool, beside the command skills of
ADR 0146 and the workflow skills of the layer. No file under `.claude/skills/` is written by
hand: each carries the generated-file banner. The site's skills section and the UI list the
skills from `editors.status`, the projection the files are written from, and type no skill
name of their own.

## Failure behaviour

`majordomus generate --check` fails on a domain skill that is missing, stale or orphaned, and
the pre-commit hook runs it. The crate's tests in `editors/claude.rs` fail when a domain skill
loses a capability row, its effect, or the rule that a change runs only on the person's
request. `scripts/site-check` fails when the skills section lists a skill the projection does
not.

## Verification

`majordomus generate --check`, `majordomus editors status`, `cargo test --lib editors::`, and
`scripts/site-check`.
{% endraw %}

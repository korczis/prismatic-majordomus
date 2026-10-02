# Agent bootstrap

Read [`README.md`](README.md) for human-facing repository context.

Canonical provider-neutral AI instructions and repository context live under
[`.ai/`](.ai/).

Before substantive planning, implementation, review, or repository mutation:

1. read [`.ai/README.md`](.ai/README.md),
2. follow its discovery protocol,
3. load mandatory effective rules and resolve their dependencies,
4. load only task-relevant skills and knowledge,
5. never implicitly load `.ai/local/**`.

If `.majordomus/` exists, treat it as an optional external/read-only Majordomus
tool installation unless the task explicitly concerns Majordomus itself.

Do not duplicate shared rules in this file.

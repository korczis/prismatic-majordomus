+++
title = "Platform and developer productivity team"
description = "Owns the rules, the tooling and the execution environment that everybody else works inside."
weight = 40
[extra]
id = "platform-team"
taxonomy = "audiences"
source = ".ai/repo/why/audiences/platform-team.md"
+++
{% raw %}

## Platform and developer productivity team

### Who this is

The team that sets up the paved road: the linting, the CI, the templates, the agent
configuration, the conventions that make one repository legible to somebody who has never
seen it. Their output is other people's defaults.

### How they work

They write a rule once and expect it to apply everywhere. With assistants in the loop, the
rule has to be loaded by a machine that did not attend the meeting, in a repository the
author has never opened, under a provider whose configuration file has its own opinions.

### What goes wrong

Rules that exist and are never loaded. Provider files that disagree with each other because
each was edited separately. A policy that changed in one place and stayed stale in the
four projections of it. Enforcement that everyone believes is running and that nothing
actually invokes.
{% endraw %}

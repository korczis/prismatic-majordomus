+++
title = "One policy, projected into every tool's instruction file"
description = "A provider-neutral policy and four execution profiles are the one source; majordomus update renders each provider bootstrap from its template, stamps it with the policy hash and the hash of its own content, and doctor fails a bootstrap that was hand-edited, that carries a rule corpus of its own, or that exceeds the always-loaded budget."
weight = 90
[extra]
id = "policy"
status = "stable"
source = ".ai/repo/features/policy.md"
+++
{% raw %}

## What it does

`.ai/repo/policy.yaml` names the projections: a provider and a target file for each. The
templates ship with the tool and a repository may override one under its own layer; the
Rust executable and the shell tool render the same bytes, stamp included, so both agree on
every stamp. A bootstrap says how to find the policy and never what the policy is: it points
at the layer and the rules, stays inside the line budget the policy sets, and carries no
rule of its own, so a rule that exists for one provider and not another cannot happen.

Profiles set capability class, effort, verbosity, presentation, context toggles and
verification as independent fields, and a task carries the profile it started with.
Unknown keys anywhere are errors, so a typo fails loudly.

## What it does not do

It never names a vendor model: capability classes are what the projection asks a worker to
map onto the closest its environment offers. It does not edit a provider's own settings,
and a hand edit of a generated file is detected and refused rather than merged.
{% endraw %}

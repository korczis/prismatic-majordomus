+++
title = "a-provider-hook-refuses-an-out-of-scope-edit — A provider that can refuse a tool call refuses an edit outside the task's scope before it is written"
description = "share/providers.yaml states for each provider, with a dated source, whether it can refuse a tool call before it runs; for each that can, the tool installs a hook that asks the task's scope and refuses an out-of-scope write, and a Stop-class hook that asks finish --check; for each that cannot, the declaration says so and nothing claims enforcement. The required CI check remains the gate; hooks are defence in depth."
weight = 43
template = "milestone.html"
[extra]
plan_id = "a-provider-hook-refuses-an-out-of-scope-edit"
source = ".ai/repo/project/milestones/a-provider-hook-refuses-an-out-of-scope-edit.yaml"
+++

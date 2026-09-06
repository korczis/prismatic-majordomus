+++
title = "Keep the plan as milestones and issues the tool can validate"
description = "Declare milestones as outcomes and issues as execution contracts, and let the tool say what is ready and what is blocked."
weight = 31
[extra]
id = "plan-the-work-as-data"
source = ".ai/repo/use-cases/plan-the-work-as-data.md"
category = "knowledge"
maturity = "guaranteed"
+++

## Situation

The plan lives in a tracker nobody reads from the repository, or in a document whose status column is a matter of opinion. What is ready to start and what is blocked is an argument, not a query.

## Outcome

Milestones and issues are files under `.ai/repo/project/`; `plan validate` refuses a dangling dependency or a cycle, and `plan ready` and `plan blocked` are computed from the same records every time.

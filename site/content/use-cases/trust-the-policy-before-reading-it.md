+++
title = "Trust the policy and the profiles only after they are validated"
description = "Prove that the canonical policy parses with every key it needs declared, that every profile parses and the default exists, and that what the policy declares as enforced is what actually runs."
weight = 14
[extra]
id = "trust-the-policy-before-reading-it"
source = ".ai/repo/use-cases/trust-the-policy-before-reading-it.md"
category = "policy"
maturity = "guaranteed"
+++

## Situation

The policy is the one file everything else is derived from. If it carries a key nobody reads, a typo becomes a silent default; if a profile is missing, the worker runs under nothing; if the policy says a hook enforces something and the hook does not exist, the enforcement is a sentence. None of that is visible by reading the files.

## What you run

- `doctor`: the policy and every profile parsed with unknown keys refused, every value the code reads declared, the default profile present, and each declared enforcement reconciled against what actually runs
- `check`: outside a task, exit 12 rather than a run that checked nothing

## Outcome

What the policy says is what the tool does, and both are proven before any worker reads them. The exit code is the answer: 0 clean, 10 a failing finding, 12 a missing precondition.

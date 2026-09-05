---
id: ci-gated-project
kind: application
title: 'A project where CI decides what is acceptable'
summary: 'Integration is gated, and a claim about the code is expected to have a test and a pipeline behind it.'
weight: 3
status: active
fits_when:
  - 'A completion claim should require verification that actually ran, with its exit code recorded'
  - 'Public capability claims are expected to name their implementation and their test'
  - 'The pipeline must fail when a declared rule stops being invoked'
does_not_fit_when:
  - 'Checks are advisory by policy and a red pipeline is routinely merged past'
  - 'There is no CI, in which case the lifecycle commands still work but the last link of every chain is missing'
use_cases: [accept-or-refuse-finished-work, prove-a-rule-is-enforced, prove-performance-with-benchmarks, add-a-use-case-and-prove-it, know-which-tool-is-running, plan-the-work-as-data, read-the-rules-the-tool-applies, block-acceptance-on-an-open-question, complete-an-issue-only-with-its-evidence, gate-ci-on-the-tool-itself, trust-the-policy-before-reading-it]
doctrines: [majordomus.verification-integrity, majordomus.doctrine-wiring-integrity]
responsibilities: [finish, doctor]
---

# Context

The team already believes that a rule without a blocking check is a suggestion. What is usually missing is the same standard applied to the rules governing the work itself, rather than only to the code.

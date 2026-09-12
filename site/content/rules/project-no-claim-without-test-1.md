+++
title = "No claim without a test"
description = "No claim without a test"
weight = 98
[extra]
kind = "rule"
slug = "project-no-claim-without-test-1"
identity = "project.no-claim-without-test@1"
status = "active"
source = ".ai/repo/rules/project/no-claim-without-test.v1.md"
+++
{% raw %}

## Rationale

A claim nobody can run is a promise; docs/CLAIMS.yaml exists so that every guaranteed claim names the test that proves it and the site refuses to render one that does not.

## Required behaviour

A capability sentence in README.md or docs/ is backed by a case in test/cases/ or is phrased as a target.

## Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

## Verification

Review. scripts/generate-site-data refuses a guaranteed claim with no test, and test/cases/28_no_hardcoded_values.sh checks every claim's detail page and test path.
{% endraw %}

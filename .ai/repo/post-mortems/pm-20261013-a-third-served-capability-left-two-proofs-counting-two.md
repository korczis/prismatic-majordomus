---
schema: post-mortem/v1
id: pm-20261013-a-third-served-capability-left-two-proofs-counting-two
kind: post-mortem
title: Adding served.forge left a doctest and a test asserting the module as it was
description: A capability added to a module breaks every example and test that enumerates the module; run the doctests and the HTTP tests before landing.
status: resolved
date: 2026-10-13
runs:
  - workflow: validate
    id: "38328889515"
    commit: 56625981f8
    jobs: ["rust (1)", "rust (3)", "coverage", "suite-shard (3)", "suite-shard (4)", "suite"]
cause:
  class: defect
  summary: Pull request 1059 added capability served.forge; the module's doctest still listed two capabilities and tests/http_serve.rs still expected the old reason text.
components:
  - file:apps/majordomus-cli/src/capability/builtin/served.rs
  - file:apps/majordomus-cli/tests/http_serve.rs
signals:
  - id: doctest
    text: "capability::builtin::served::module (line 450)"
  - id: test
    text: "what_was_deployed_answers_over_mcp_as_over_http"
  - id: panic
    text: "tests/http_serve.rs:242"
fixed_by: ["commit:c03a5ff769", "commit:e6d0371cfb", "pr:1061"]
prevention: ["file:apps/majordomus-cli/tests/http_serve.rs"]
tags: [rust, capabilities]
related: ["issue:I2649"]
---

# Adding served.forge left a doctest and a test asserting the module as it was

## What failed

In `validate` 38328889515, `rust (1)` failed one doctest of 2045, `src/capability/builtin/served.rs
- capability::builtin::served::module (line 450)`. `rust (3)`, `coverage` and case
`76_capabilities_projections` (shard 4) failed on
`what_was_deployed_answers_over_mcp_as_over_http` at `tests/http_serve.rs:242`; case
`77_rust_evidence` (shard 3) failed on the same doctest.

## Root cause

Pull request 1059 added `served.forge` beside `served.observe` and `served.show`. The
module's example asserted the list of two, and the HTTP test asserted a reason that the new
capability's presence had changed (`the forge has not been observed from this checkout;
majordomus served forge asks it`). Neither the doctests nor `http_serve` were run on the
branch before it was merged as the release candidate.

## Fix

`c03a5ff769` and `e6d0371cfb` (pull request 1061) name the third capability in the example
and in the test.

## Prevention

The two tests are the guard. They are in the full `cargo test`, which the pre-push hook
runs; the candidate was pushed with its branch run cancelled, so the trunk ran them first.

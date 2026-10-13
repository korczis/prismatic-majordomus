+++
title = "Adding served.forge left a doctest and a test asserting the module as it was"
description = "Adding served.forge left a doctest and a test asserting the module as it was"
weight = 3
[extra]
kind = "post-mortem"
slug = "pm-20261013-a-third-served-capability-left-two-proofs-counting-two"
identity = "pm-20261013-a-third-served-capability-left-two-proofs-counting-two"
status = "resolved"
source = ".ai/repo/post-mortems/pm-20261013-a-third-served-capability-left-two-proofs-counting-two.md"
+++
{% raw %}

## Adding served.forge left a doctest and a test asserting the module as it was

### What failed

In `validate` 38328889515, `rust (1)` failed one doctest of 2045, `src/capability/builtin/served.rs
- capability::builtin::served::module (line 450)`. `rust (3)`, `coverage` and case
`76_capabilities_projections` (shard 4) failed on
`what_was_deployed_answers_over_mcp_as_over_http` at `tests/http_serve.rs:242`; case
`77_rust_evidence` (shard 3) failed on the same doctest.

### Root cause

Pull request 1059 added `served.forge` beside `served.observe` and `served.show`. The
module's example asserted the list of two, and the HTTP test asserted a reason that the new
capability's presence had changed (`the forge has not been observed from this checkout;
majordomus served forge asks it`). Neither the doctests nor `http_serve` were run on the
branch before it was merged as the release candidate.

### Fix

`c03a5ff769` and `e6d0371cfb` (pull request 1061) name the third capability in the example
and in the test.

### Prevention

The two tests are the guard. They are in the full `cargo test`, which the pre-push hook
runs; the candidate was pushed with its branch run cancelled, so the trunk ran them first.
{% endraw %}

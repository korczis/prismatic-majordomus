+++
title = "Every surface a change reaches — the published site, the release metadata, each active deployment — is asked for the identity it states at its own address and compared with the trunk's revision; a stale, unreachable or unreadable surface refuses, and neither a deploy command's exit code nor an HTTP 200 counts as verification"
description = "A deploy command that exited 0 with the old revision still live is indistinguishable from"
weight = 146
[extra]
claim_id = "deployment-verified-live"
status = "guaranteed"
source = "docs/claims/deployment-verified-live.md"
+++
{% raw %}

## What it means

A deploy command that exited 0 with the old revision still live is indistinguishable from
a deployment unless something asks. `deploy.verify` asks. Which surfaces a change reaches is
derived from what the repository declares — the CI model's path classes say what the site
is built from and what the public surface is taken over, `site/config.toml` says where the
site is published, the release records say what was last published, the deployment objects
say whether anything is running — and each applicable surface is asked at the address that
states its own identity: `/build.json` for the site, `releases/latest.json` for the release
metadata, `/api/v1/distribution/build` for a running executable.

A surface stating an older commit, version or tag than expected is **stale**; one that does
not answer is **unreachable**; a reachable one with no identity this executable can read is
**unreadable**. None is a pass, and a report that asked nothing is not ok. A target the
change does not reach, or a deployment object that is declared rather than active, is in the
plan with the reason and is never asked.

## How it works

The `deploy` and `verify` obligations are established live by `deploy.verify` once the
trunk reaches the task's commit, against the trunk's head; before that, the finding says
the trunk does not reach the commit rather than asking a surface about a revision it was
never sent. The request carries no header and the evidence carries the address asked, the
identity expected, the identity stated and one sentence — never a body beyond the fields
compared. The network is behind one trait, and the default fetcher is `curl` with a bounded
timeout; the crate's suites drive the same comparison with answers of their own.

## What proves it

`test/cases/281_deploy_verify_live.sh` serves a site identity from a local origin and proves
the same target verified, stale, unreadable and unreachable in turn, that a target the change
does not reach is not asked, and that the `verify` obligation reads the trunk before asking
anything. `deploy::verify`'s suite holds the comparison, the redaction of the evidence and
that nothing asked is not ok. The rule is `project.deployment-is-verified-live`; the decision
is ADR 0057.
{% endraw %}

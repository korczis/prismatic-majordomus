# Every surface a change reaches is asked what it serves, and a stale or silent one refuses

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

## Why it asks rather than trusts

A deploy command's exit code says the command ran; a green pipeline says the tree was fine;
an HTTP 200 says something answered. None of them says which revision is live, and the
repository paid for that difference twice. The only fact that settles it is the identity the
surface itself states, so that is the only fact this reads.

## What it does not cover

A deployment object that is `declared` rather than `active` is not asked and not pretended
about. A surface the repository does not declare — a mirror, a CDN edge, a fork — is not a
target. Smoke tests beyond the identity comparison belong to the surfaces' own probes
(`scripts/pages verify`, `scripts/ci/pages-check`, the release smoke phase); this claim is
about the revision, not the behaviour behind it.

## How to see it

`test/cases/281_deploy_verify_live.sh` serves a site identity from a local origin and proves
the same target verified, stale, unreadable and unreachable in turn, that a target the change
does not reach is not asked, and that the `verify` obligation reads the trunk before asking
anything. `deploy::verify`'s suite holds the comparison, the redaction of the evidence and
that nothing asked is not ok. The rule is `project.deployment-is-verified-live`; the decision
is ADR 0057.

---
id: project.deployment-is-verified-live
version: 1
kind: rule
title: A deployment is verified by asking the deployed surface what it serves
description: Every surface a change reaches — the published site, the published release metadata, each active deployment object — is asked for the identity it states at its own address and compared with the revision the trunk expects. A surface stating an older identity is stale, one that does not answer is unreachable, a reachable one with no identity is unreadable, and none of those is a pass. Which surfaces a change reaches is derived from what the repository declares, never from a list of deploy commands, and a target that does not apply is in the plan with the reason.
statement: Derive the deployment targets from the CI model's path classes, the site's declared origin, the release records and the deployment objects; ask each applicable target for the identity it states and compare it with the expected revision; refuse a stale, unreachable or unreadable surface; and never take a deploy command's exit code, a green pipeline or an HTTP 200 as evidence that a revision is live.
status: active
class: blocking
depends_on: [project.land-and-publish@1, project.completion-is-proved@1]
tags: [deployment, verification, evidence, release]

x-majordomus:
  tests: [test/cases/281_deploy_verify_live.sh]
---

# Rationale

On 2026-09-09 the public site served a commit from an unmerged branch for half an hour;
on 2026-09-10 it sat hours behind the trunk. Every tree-level gate was green throughout,
the deploy workflow had exited 0, and a person noticed by eye both times. The site had
carried its own identity (`/build.json`) the whole while, and the executable had stated
its commit at `/api/v1/distribution/build` since the distribution model existed. Nothing
asked.

The `deploy` and `verify` obligations were declared with nothing to establish them, so a
worker recorded what they saw — and a hand-recorded remote fact is exactly the thing a
worker can be wrong about in the direction that flatters them.

# Required behaviour

- `deploy.verify` derives the plan (`deploy::targets`): the published site applies when
  the change touches what the site is built from, the release when it touches what the
  public surface is taken over, an active deployment object when it touches the object's
  build inputs; a verifier asked after a deployment asks every target that exists.
- Each applicable target is asked at the address that states its identity — `/build.json`,
  `releases/latest.json`, `/api/v1/distribution/build` — and the identity is compared
  field by field with what is expected. The request carries no header; the evidence
  carries the address asked, the identity expected, the identity stated and one sentence,
  and never a body beyond the fields compared.
- The obligations `deploy` and `verify` are established live by it once the trunk reaches
  the task's commit, against the trunk's head; before that, the finding says the trunk
  does not reach the commit rather than asking a surface about a revision it was never
  sent. A repository with no deployment object cannot establish `deploy`, and says so.
- A deployment object that is `declared` rather than `active` is not asked and not
  pretended about; the plan names it with the reason.

# Failure behaviour

A surface stating an older commit, version or tag than expected refuses (`stale`); one
that does not answer refuses (`unreachable`); one that answers with nothing this executable
can read as an identity refuses (`unreadable`). `finish --outcome completed` is refused
while the `verify` obligation stands unmet or the deployment-verified question refuses. A
target list written by hand, or a verification that reads a status code and nothing else,
is a violation.

# Verification

`test/cases/281_deploy_verify_live.sh` serves a site identity from a local origin and
proves the same target verified, stale and unreachable in turn, that a target the change
does not reach is not asked, and that the `verify` obligation reads the trunk before
asking. The crate's `deploy::verify` suite holds the comparison, the redaction of the
evidence, and that nothing asked is not ok.

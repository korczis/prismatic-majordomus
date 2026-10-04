+++
title = "Completed work does not leave the trunk on its version"
description = "Completed work does not leave the trunk on its version"
weight = 60
[extra]
kind = "rule"
slug = "majordomus-version-obligation-1"
identity = "majordomus.version-obligation@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/version-obligation.v1.md"
+++
{% raw %}

## Rationale

A version is a statement about what is being served. The public contract already decides
the least a *release* owes — a capability removed is breaking whatever the commit that
removed it called itself — and that analysis is silent about most work: a fix, a test, a
document and a refactor behind the boundary move no contract. A trunk that takes dozens of
them in a day stays on one version, and the version stops naming anything.

The cadence is the repository's answer to that: accepted work advances the version by at
least this much. It is measured against the version the **trunk** declares, because the trunk
is what the work is integrated into — not the last release, which can be many integrations
behind. Two branches that both started from one trunk would otherwise both land claiming the
same next version.

**Why it is a predicate and not an event.** A provider ends its session, a person runs
`finish` twice after a network failure, a workflow is rerun, a deployment is retried: a
counter incremented at any of those boundaries advances the version once per boundary that
saw the work. The obligation is instead decided from the tree and the trunk — the declared
version reaches the minimum, or it does not — so asking again finds it satisfied and writes
nothing. It becomes owed again only when the trunk itself moves past it, which is exactly
when a new advance is due.

**Why a provider's end is not a completion.** The session-end hook writes a handover. A
handover says the work is unfinished, so it owes no version; only an accepted `completed`
outcome does.

## Required behaviour

**The policy selects it.** `version_advanced` in `verification.finish_requires` turns the
requirement on, and `release.cadence` names the cadence. A repository that declares no
cadence is judged by the contract alone, as before.

**`finish --outcome completed` judges first and advances last.** The doctrine reads the
obligation with the rest of the contract: `satisfied` and `not-owed` hold, `owed` holds as
*to be paid*, and `behind` and `unverified` — a tree behind its trunk, a trunk or a reader
nobody could reach — refuse. Only when every line of the contract holds does `finish` run
`release advance`, which raises the version to the minimum with the one writer — the
manifest's version line and the lock's record of it — regenerate the version's projection
with `majordomus generate distribution`, and read the obligation again: it must now hold. The
advance is recorded as `release.advanced`, with the obligation's identity, the trunk and both
versions, and `task.finished` follows it. A completion refused for any reason advances
nothing and records no advance, so neither the tree nor the ledger can be read as accepted
work that was not.

**`check` reports it and refuses nothing.** A check is asked many times while work is in
progress, and an advance owed by unfinished work is not a defect yet.

**Only the outcome `completed` is refused.** `partial`, `blocked`, `failed` and `no_match`
are honest about unfinished work and owe no version.

## Failure behaviour

A violation is a `FAIL` finding under the category `version`, naming the obligation's
identity, the declared version, the minimum and why each input requires what it does.
`finish` exits 10 and writes no completion and no advance. An advance that could not be made,
or that did not leave the obligation satisfied, refuses the same way, named under this
doctrine.

## Verification

`mj_validate_version_obligation` decides it, dispatched from `check` and `finish`, by asking
the executable's `release obligation`. `test/cases/801_completed_work_advances_the_version.sh`
proves it against fixture repositories with a trunk: a completed finish advances the version
once and a repeated finish does not advance it again; a completion refused by another line
advances nothing; `check` reports the obligation and refuses nothing; an outcome other than completed owes
nothing; a change set of projections only owes nothing; a tree whose trunk moved past it is
refused; and a trunk that cannot be read is unverified and refused.
{% endraw %}

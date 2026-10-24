+++
title = "A published UI shows the evidence CI recorded, never \"not run\" for what CI ran"
description = "A published UI shows the evidence CI recorded, never \"not run\" for what CI ran"
weight = 149
[extra]
kind = "rule"
slug = "project-published-evidence-is-what-ci-recorded-1"
identity = "project.published-evidence-is-what-ci-recorded@1"
status = "active"
source = ".ai/repo/rules/project/published-evidence-is-what-ci-recorded.v1.md"
+++
{% raw %}

## Rationale

On 2026-10-16 the owner opened https://majordomus.dev/ui/kiosk/ and saw a cloud of "not run".
The snapshot published there had counted 154 test claims as never run. The validation of the
same commit had run every one of them, and its `evidence` artifact held 606 recorded
executions.

The Pages runner read the tracked ledger, `.ai/repo/evidence/ledger.json`. That ledger holds
fourteen local runs, last changed on 2026-10-03, because the recording branch that would carry
CI's runs into it (ADR 0080, I1954) is not built. Every test without a row there reads
`not run`.

"Not run" is the one evidence state that is a statement about the past. It says nobody ran
this. Publishing it for a test CI ran is a false statement on the page the project shows the
world, not merely a stale one.

Measured over the artifact of master d116266803:

<div class="overflow-x-auto" tabindex="0">

| ledger | proven | not run |
|---|---|---|
| tracked only | 0 | 214 |
| the run's | 235 | 0 |

</div>


## Required behaviour

- The Pages workflow's UI snapshot job runs `scripts/pages evidence --ledger-out
  .ai/repo/evidence/ledger.json` before the UI is read.
  - It uses the artifact of the published commit or its nearest validated ancestor. Its
    manifest must name a commit in the published history, or nothing is copied.
  - A row of an ancestor is judged by the one freshness function at the published commit, so
    it reads stale or inputs unchanged, never as a run of this commit.
- It then runs `scripts/pages published-evidence --bin <the executable that reads the UI>`.
  This asks evidence.report, the capability the screens ask, in the same checkout. It refuses
  with exit 10 when `not run` exceeds the declared baseline.
- The baseline, `.ai/repo/evidence/published-not-run.txt`, is a single count. It may only fall.
- When the recording branch lands (I1954), the tracked ledger carries CI's runs itself. The
  overlay then has nothing to add, and the check keeps holding the same line.

## Failure behaviour

- An artifact outside the published history, or one with no ledger, copies nothing and says
  why. The check then refuses what the tracked ledger alone would publish.
- A report that cannot be read, or a baseline that holds no count, exits 12. It is never read
  as a pass.

## Verification

`test/cases/3178_the_published_ui_reads_the_evidence_ci_recorded.sh` drives both commands
through the real script against this repository's history. It covers:

- the ledger put in place;
- the ledger refused for a commit outside the history;
- the ledger refused when the artifact carries none;
- the check passing within the baseline and refusing over it;
- the check exiting 12 on an unreadable report and on an empty or absent baseline.
{% endraw %}

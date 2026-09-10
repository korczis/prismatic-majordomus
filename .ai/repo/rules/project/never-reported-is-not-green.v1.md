---
id: project.never-reported-is-not-green
version: 1
kind: rule
title: A verdict that never arrived is not a verdict
description: A change is judged against the trunk's own failing set, established by reading a run that finished, and never against the absence of a report; a check that has not spoken is unknown, not passing.
statement: A check that has not reported is unknown, never passing; a change is merged against the trunk's measured failing set, not against silence.
status: active
class: advisory
depends_on: [project.land-and-publish@1]
tags: [process, integration, evidence]
---

# Rationale

For a whole night this repository's trunk carried three branch-breaking defects while every
surface a person looks at showed nothing wrong. No validation run had completed: the runs were
queued behind scarce runners, and each new push cancelled the one before it. Nobody was
deceived by a green tick, because there was no green tick — they were deceived by an empty
space where a verdict goes, which reads exactly like one.

The same shape appeared twice more the next day, in different clothes:

- A publication that had **succeeded** reported `cancelled`, because the job's own wait for the
  content delivery network expired before the network caught up. The site was serving the new
  commit a minute later. A worker reading the job's status would have re-run a deploy that had
  already happened; the answer was to ask what the site was actually serving.
- Every pull request open that day showed failing checks, and every one of those failures was
  the trunk's own. A worker comparing against zero would have concluded that all of them were
  broken and that none could land, and the day would have ended with nothing merged and a trunk
  still red.

What these have in common is that the cheap reading — a colour, a status word, a count of
failures — is not the reading that decides. The expensive reading is what a run actually said,
and about which tree.

# Required behaviour

**A check that has not reported is unknown.** Absence is recorded and pursued as absence; it is
never written down, spoken, or merged against as though it had passed. When a run does not
complete, the honest report is that the change is unverified, and it says which check is
missing.

**A change is judged against the trunk's own failing set.** Before merging, establish what the
trunk itself fails — from a run that finished, or by running the gate locally on the trunk —
and compare. Identical or smaller is a change that leaves the trunk no worse, and that is the
bar; strictly zero is not the bar while the trunk is red, because it would freeze every repair
behind every other repair. A failure the change introduces is named and owned before it lands.

**A job's status is not the fact it was measuring.** When a job reports a failure whose subject
is observable — a published site, a running server, a file on disk — ask the subject before
believing the job. A bounded wait that expired is a measurement that did not finish, not a
measurement that came back negative.

**A verdict is arranged to arrive.** A gate whose runner is so scarce that its answer never
comes is documentation, not enforcement; either it is planned so it can complete, or the fact
that it cannot is stated where a reader will meet it.

# Failure behaviour

Decided by review and by the record, not by a gate: no check can tell that a person believed
silence. The nearest mechanical support is that the planner refuses a model whose verdict
cannot be reached, and that the published-site gate asks the site rather than the workflow.

A claim in a report, a pull request or a commit message that something "passes" when no run
finished is the failure this rule names, and it is a correctness failure rather than a matter
of tone: it puts a fact into the record that nothing measured.

# Verification

The planner's own case holds that every gate the model declares is reachable in some plan, so a
gate cannot be declared into a job that never runs. Everything else here is held by review.

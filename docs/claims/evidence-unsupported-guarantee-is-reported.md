# A claim that declares a guarantee no recorded run supports is named by a gate rather than displayed as guaranteed

## What it means

`majordomus evidence show --check` exits 10 when any claim declaring `status: guaranteed`
is in a state the recorded evidence does not support, and names each one with the reason
and the command that would settle it. `scripts/evidence-check` renders that same answer and
ratchets it against `.ai/repo/evidence-baseline.txt`, so a guarantee that loses its
evidence is a failure while the debt that already existed is known rather than blocking.

Only `guaranteed` claims are judged. `advisory` states that enforcement is not observable
from outside, `planned` that nothing implements the behaviour yet and `rejected` that
nothing will; demanding a current proof of those would be demanding proof of a thing the
claim already says is not there.

## How it works

Of the seven proof states, the three that carry a passing run behind them — `proven`,
`inputs_unchanged` and `stale` — are accepted as support, and are never printed as the same
thing. The other four are findings: `failing`, because the claim guarantees a behaviour
whose test most recently failed; `not_run`, because it names a test no run of which has
ever been recorded; `unrunnable`, because it names a path no runner drives, so no execution
of it can ever exist; and `no_test`, because it names no test at all. Each finding carries
the claim, the status it declares, the state the evidence supports, the reason in words,
and the reproduce command when running something would settle it.

`scripts/evidence-check` derives nothing of its own. It asks the executable for
`evidence show --format json` and reads the findings out of that answer, because a gate
with a second opinion about what is proven would be a second model of proof. It prints the
tallies for the whole matrix, not only the failures, so a reader sees the shape of the
evidence and when it was last recorded. Without a build it exits 12 and says so rather than
passing quietly.

The ratchet is not membership of a list of names. It was, and that was the defect rather
than the strictness: the baseline names the guarantees that were unsupported on the day it
was written, so a claim written after that day is absent from it for the only reason a
claim can be absent from a snapshot of the past — it did not exist yet. Under membership
every newly merged guarantee was reported as having lost evidence it had never had, and
reddened the trunk for every branch that merged afterwards. A ratchet whose alarm fires on
growth rather than on regression measures the wrong thing.

So the baseline records the state per claim — a `+` line for a guarantee a run supported, a
bare line for one no run supported — and two things fail the gate. **Lost**: a claim the
baseline recorded as supported, and the evidence no longer supports; it had a proof and
does not now, and membership could never see this, because a claim losing its proof joins
the unsupported set exactly as a new claim does. **Withdrawn**: a supported guarantee that
is no longer a guaranteed claim of the matrix at all; nothing became unproven, the
denominator simply shrank, which is how a ratchet rots without ever going red.

A guaranteed claim the baseline never knew, arriving with no recorded run, is reported and
counted as new debt and does not fail — refusing it is the broken behaviour above. It is
admitted by `scripts/evidence-check --baseline`, in its own commit, with the reason. A
baseline line that is now supported is reported so the list tightens instead of rotting.
The file is written by `--baseline` and never by hand, and its unsupported half shrinks in
one direction only — by recording a run that proves a claim, never by adding a line.

The gate is declared in `.ai/repo/ci/gates.yaml` under the `structure` job with
`always: true`, and the `evidence` change class routes the ledger, the baseline, the gate,
the matrix, the claim pages and `docs/EVIDENCE.md` to it.

## How to see it

```bash
majordomus evidence show --check                  # exit 10 with the unsupported guarantees
majordomus evidence show --findings --format json # the findings alone, with reasons
scripts/evidence-check                            # the ratcheted verdict, with the tallies
scripts/evidence-check --strict                   # every unsupported guarantee fails
scripts/evidence-check --baseline                 # rewrite the baseline from what is found now
scripts/evidence-check --repo PATH                # judge that repository instead of this one
```

Filtering never narrows the tallies: an answer showing one claim still reports how much of
the matrix was examined, so a filtered read cannot misreport coverage.

## What it does not cover

It is advisory today, and saying otherwise would be the kind of claim this subsystem
exists to refuse. The ledger starts empty, so on the day the gate arrived every guaranteed
claim was `not_run` — true, and as a blocking gate it would have meant nothing could be
merged by anybody until a full suite run had been recorded. Every one of those claims is in
the baseline, so what the gate refuses right now is a guarantee that lost its proof or left
the matrix, not an unproven guarantee. New unproven guarantees are named and counted and do
not fail. `--strict` ignores the baseline and fails on all of them; it is the end state,
reachable when the baseline's unsupported half is empty, and CI switches to it then.

The gate does not run anything. It reads what was recorded, so a claim proven by a run
nobody recorded is indistinguishable from one nobody ran, and the remedy for a finding is
to run the named command and record it — or to fix what broke.

It judges the declared status against the evidence, not the claim against reality. That the
test named actually exercises the sentence claimed is a question for review, held by
`project.no-claim-without-test`; a `stale` guarantee passes the gate, because a proof older
than its subject is still a proof that was made.

And it is not a tamper check. The ledger is a tracked file, a hand-edited outcome is a line
in a diff, and the gate believes what the ledger says.

## Why it exists

A guarantee is the strongest thing this repository says about itself, and it was the one
thing nothing checked: the matrix could declare a behaviour guaranteed, name a test, and be
published as a guarantee with no run behind it anywhere. Making that visible had to come
before making it fatal, because a gate that blocks on the day it arrives is a gate that
gets disabled. So the debt was measured, written down once, and made to shrink in one
direction — which is how a check that nobody can honour becomes a check nobody can regress.

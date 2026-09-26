# A claim reads proven only when its run is contained in the presented revision, nothing but the ledger changed since, and both trees were their commits

## What it means

Every verdict is a verdict at something. `majordomus evidence show` judges the working
tree in front of you; `majordomus evidence show --presented HEAD` judges the checked-out
commit as committed, which is what a site built from that commit shows its readers. In
either case a claim reads `proven` only when all of these hold: its latest run passed; the
commit the run was recorded on is one the presented revision contains; nothing but the
evidence ledger changed between that commit and the presented revision; the run measured a
tree that was its commit; and the presented tree is its commit too.

A pass recorded on a commit the presented revision does not contain — an unmerged branch, a
rewritten history — reads `stale`, and the detail names the commit. A named revision is
judged by the ledger committed in it, not by the ledger in the working tree, and a clean
failing run of the same test that the checkout holds uncommitted, recorded between the
evidence and the presented commit, caps the verdict at `stale`.

## How it works

One function, `evidence::freshness` in `apps/majordomus-cli/src/evidence/freshness.rs`,
decides every state from the recorded run and a comparison with the presented revision,
and the first matching row of its truth table wins (docs/EVIDENCE.md, "The seven states").
The comparison is `freshness::compare`: whether the presented revision contains the
evidence commit (`git::contains`, which answers "unknown" for a commit this clone does not
have rather than "no"), and which paths changed between them — for the working tree every
tracked, staged and untracked change, for a presented commit the committed difference.

`freshness::presented_commit` refuses any revision that is not the checked-out commit,
because the claims and the tests' sources are read from the checkout. It measures the tree
without the ledger's working copy, and a tree state the caller gives can only weaken that
measurement. `freshness::ledger_at` reads the ledger as the commit holds it.

The executions the working ledger holds and the committed ledger does not are
`freshness::uncommitted`, listed in the report's `presented.uncommitted`, and read only
through `freshness::weakened_by`: a record may withhold `proven` and never grant it. A
failing, timed-out or errored run on a clean tree, at a commit that contains the evidence
and that the presented commit contains, moves `proven` or `inputs_unchanged` to `stale` with
a detail naming it. The cap is `stale`, not `failing`: such a record never decides a
verdict.

## How to see it

```bash
majordomus evidence show --presented HEAD                        # what a build of HEAD shows
majordomus evidence show --presented HEAD --presented-tree dirty # a build from a dirty tree
majordomus evidence show --presented HEAD --format json | jq .presented
bash test/run.sh 502_evidence_is_judged_at_the_presented_revision
```

The case builds a fixture repository and walks every half: a pass on a side branch read
from the trunk, a pass committed with its ledger and read at the next commit, a failing run
held uncommitted, an uncommitted pass that must not strengthen anything, a dirty tree
measured and declared, and the refusals.

## What it does not cover

It judges only the checked-out commit. A verdict at an arbitrary commit would need that
commit's claims and test sources, which are read from the checkout; `--presented` refuses
any other revision rather than mixing two trees.

The working ledger's uncommitted executions are the only supplementary records read here.
Run records kept elsewhere, such as a CI run's own artifact, are later sources of the same
rule and are not read by this report. The entry preflight keeps its own per-run filter until
it is moved onto this function.

`inputs_unchanged` remains the absence of a known invalidation. A change to a file a claim
does not name is not caught; the answer is to record a run against the presented commit.

## Why it exists

Before this, every verdict was taken against the reader's working tree, so nothing could
say whether a claim was proven at the commit a site showed, and a pass recorded on a branch
that never merged could read `proven` on a trunk that never contained it. A report judged
from the committed ledger alone would have the opposite defect: it would read `proven` over
a failing run the same checkout holds. The presented revision, containment and the
monotone rule close all three, and they are one function, so no surface can decide
differently from another.

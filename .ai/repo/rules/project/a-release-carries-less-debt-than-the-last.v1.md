---
id: project.a-release-carries-less-debt-than-the-last
version: 1
kind: rule
title: A release carries less debt than the one before it
description: Before every release the documents say what is true, every claim is validated and covered by a test, and the repository's own accepted violations are fewer than they were at the previous release, by at least a declared minimum. The debt is counted by `majordomus release debt` from the baselines `.ai/repo/ci/debt.yaml` declares, written into the release's record, and held against the next release; no baseline may grow in any change.
statement: Do not release a tree whose accepted violations did not fall. Pay at least the declared minimum of the debt before the tag, never add an entry to a baseline, and never let a baseline exist that the declaration does not count.
status: active
class: blocking
depends_on: [project.no-claim-without-test@1, project.release-is-a-projection@2]
tags: [release, debt, baselines, dogfooding]

x-majordomus:
  tests: [test/cases/997_a_release_carries_less_debt_than_the_last.sh]
---

# Rationale

A gate that could not be made to pass on the day it was written keeps a baseline: a file of
violations it accepts, and refuses anything beyond. Eighteen such files stood under
`.ai/repo/` on 2026-10-08, holding 2788 accepted violations between them — 1861 public Rust
items without a tested example, 178 documented commands nothing runs, 158 claims without
evidence, 99 MCP tools nothing invokes, and the rest.

Each of those gates is a ratchet inside one change: its list may not grow. None of them
said the list must ever get shorter, and nothing added them up. The repository shipped
several releases a day, each of them green, over a debt that only had to stand still. A
baseline file is a place for a number to be quietly kept; "no new debt" is satisfied for
ever by doing nothing.

The owner's rule of 2026-10-08 is that it does not stand still: before every release the
Markdown is brought to the truth, every claim is validated and covered with tests, the
repository's own violations converge to zero, and every release reduces them by at least a
minimum. And that this is enforced on the repository by its own tool, not kept as a habit.

# Required behaviour

1. **Before a release, four things hold on the commit to be tagged.**
   - The documents say what is true: the derived ones are current, the documented commands
     are run or say why not, the diagrams are drawn, the links resolve.
   - Every guaranteed claim is validated by evidence.
   - What the release changes is covered by tests.
   - The accepted violations are fewer than at the previous release (below).
   The first three are held by the gates that already exist — the commit's `ci` verdict,
   which `scripts/ci/release-verdict` requires. This rule adds the fourth and states that a
   release owes all four; a release announcement names what stood for each and says what no
   gate states rather than claiming it.
2. **The debt is declared once.** `.ai/repo/ci/debt.yaml` names every baseline that is debt,
   the gate that reads it and the form its entries take: one violation per line, or numbers
   the file states. A file under `.ai/repo/` named like a baseline is listed there or
   excluded there with its reason.
3. **No baseline grows.** In any change, a baseline larger than the previous release
   recorded it is refused, whatever the total did.
4. **A release carries less.** The debt a change can pay is lower than the previous
   release's by at least `minimum_reduction`. The minimum is never below 1.
5. **Every release records what it carried.** `scripts/release-record` writes the counts
   into the release record, from `majordomus release debt --record`, so that the next
   release is held against a number that was measured and not remembered.
6. **A first measurement says it is one.** A previous release that recorded no counts is
   compared with nothing, and the report says so; it is neither a pass by silence nor a
   refusal.
7. **Paying is done by fixing.** An entry leaves a baseline because what it names was
   fixed. Deleting an entry whose violation remains makes its own gate fail, which is the
   check that the payment was real.
8. **Debt that cannot be paid says so, and why.** A baseline no change can pay is declared
   `payable: false` with its reason. It is still counted, still recorded and still refused
   when it grows; it is left out only of what a release must reduce and of "nothing is
   owed". One baseline is so declared, `commit-policy`: the commits it counts are published
   and the history is append-only. A second needs its own reason and the same scrutiny.

9. **The evidence is not older than the release before.** The declaration names the
   tracked evidence ledger; a release is refused when the newest execution CI recorded
   there predates the previous release's publication, when the ledger holds no CI
   execution, or when it cannot be read. Measured on 2026-10-08: the ledger was recorded
   on 2026-09-17 and supported none of 237 claims through two releases, while the check
   that reads it passed, because it refuses only evidence that was lost. A change is never
   refused for it.

10. **A baseline is not above the truth at a release.** A baseline whose own gate says it
    can be tightened is refused at a release until it is written again: the record would
    otherwise carry a debt the tree does not have. Measured on 2026-10-08: the pipefail
    baseline declared 29 while its gate measured 28, through a release. The declaration
    names, per baseline, the command to ask and the words it prints; a gate that cannot be
    run is a refusal, and a baseline whose gate cannot state slack is named as not measured
    in every release, never passed over.

11. **What could not be read is never a zero.** A previous release whose debt block cannot
    be read is a refusal, not a release that recorded none. A baseline the previous release
    counted above zero and the declaration no longer names is a refusal: debt leaves by
    reaching zero, not by leaving the declaration. A record is not written over counts that
    could not be taken. A gate asked for slack answers with exit 0 or 10; any other exit is
    a gate that could not judge, and refuses. A minimum below 1 is refused.
12. **Each finding says what it does.** The report marks a finding `FAIL` only when it
    refuses the verdict that was asked for, a change or a release; a statement printed
    beside a refusal is not marked as one.

# Failure behaviour

- `majordomus release debt` exits 10 when a baseline grew, when a file named like a baseline
  is neither counted nor excluded, when an exclusion or an unpayable baseline gives no
  reason, when a counter states no number or a declared file cannot be read. It names each.
- `majordomus release debt --release` also exits 10 when the total did not fall by the
  minimum, and says the largest total a release may carry.
- `majordomus release debt --release` exits 10 as well when the declared evidence ledger is
  older than the previous release, holds no CI execution or cannot be read, and names both
  times. `majordomus release debt` without `--release` does not.
- `majordomus release debt --release` exits 10 when a baseline's own gate says the baseline
  can be tightened, or when that gate could not be run, naming the baseline, what it
  declares and the command asked.
- A declaration that cannot be read is exit 12: nothing is known to be debt, which is not a
  tree with none.
- `scripts/ci/release-verdict` refuses the tag for the same reasons, before it is pushed;
  the release workflow's `debt` phase refuses it before an artifact is built; and
  `scripts/release-record` writes no record for a tree whose debt it could not count.

# Verification

- `test/cases/997_a_release_carries_less_debt_than_the_last.sh`: a repository with one
  release on record. Standing still is allowed in a change and refused at a release; one
  entry paid makes the release pass; one line added to a baseline is refused in a change;
  a counter that grew is seen though the total fell; an undeclared baseline is refused; a
  first measurement is recorded and said to be compared with nothing; an unpayable baseline
  is counted and not owed, refused when it grows and refused without its reason. A second
  release over byte-identical baselines is refused against the one before it. A declared
  evidence ledger that is missing, or whose newest CI execution predates the previous
  release, refuses a release and not a change. The MCP
  tool answers with the same counts, and this repository's own declaration names
  `commit-policy` as its only unpayable baseline.
- `majordomus release debt` on this repository prints the eighteen baselines and their
  total; the gate `release-debt` runs it on every change that touches one.
- The unit tests of `apps/majordomus-cli/src/release/debt.rs` hold each form of count and
  each standing.

# What this does not decide

- Which baseline is paid. Any entry counts.
- That `commit-policy` is retired. It is frozen, not gone: counted in every report and
  record, and refused if it grows. Retiring the baseline is the owner's act.
- That a document which is consistent and wrong is true. No gate states that; a release
  says so.

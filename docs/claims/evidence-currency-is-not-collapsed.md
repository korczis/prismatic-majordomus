# A run recorded against this commit and a run whose inputs merely have not changed since are reported as different states, never as one

## What it means

Two passing runs can stand in very different relations to the tree in front of you, and
the model refuses to print them the same way.

`proven` means the diff between the execution's own commit and the working tree is empty:
the tree in front of you is byte for byte the tree the run measured. That is not the same
statement as "recorded at HEAD with a clean tree", which sounds equivalent and is not.

`inputs_unchanged` means a passing run exists and nothing the claim itself names — its
`source`, its `implementation`, and the test's own source — differs between the recorded
commit and the working tree. That is the absence of a known invalidation, not proof at
HEAD: something the claim does not name may have broken the behaviour since. It is
labelled as the weaker thing everywhere it is shown, and the command line prints the
sentence that explains it beside the state.

A surface that rendered both as one tick would have reintroduced the defect whatever the
underlying data said, so the distinction is in the model, in every projection derived from
it, and in the gate.

## How it works

The comparison is `git diff --name-only <the execution's own commit> --`, run once per
distinct commit the ledger names and shared by every claim recorded against it, which in
practice is one subprocess. It catches a path committed since, staged, or merely edited in
the working tree. The proof state is decided by that diff against each execution's own
commit and never by HEAD; HEAD and the tree state are reported so a reader knows what the
report was derived against.

The ledger's own row is excluded from the diff, and this is the least obvious part of the
design. The evidence is *about* the tree rather than part of what the tests measure, so its
own row must not age the proof it records. Without the exclusion `proven` is unreachable by
construction: recording writes the ledger and therefore dirties the tree, and committing
the record moves HEAD past the commit the record names, so the strongest state could never
be reached by the act that produces the evidence for it. This was found in review, and
`test/cases/124_evidence.sh` now asserts both halves — the tree is dirty immediately after
a recording, every claim is still `proven`, and recording touched nothing but the ledger —
so a regression that made `proven` depend on a clean tree again would fail rather than pass
unnoticed.

`stale` is the third passing state: the run passed, but something the claim names has
changed since, and the changed paths are named so a reader is told which of the three
invalidated the run rather than told to go and look. It also covers two cases a diff alone
would miss. When the test's own source no longer hashes to the digest that was recorded,
the claim is `stale` and the test is named, because an edit made and reverted around a run
leaves no diff and is still not the thing that was measured. And when git cannot answer the
comparison at all — no work tree, a commit this checkout does not have — the state is
`stale` rather than `inputs_unchanged`, because not knowing is not proof.

The seven states are ordered strongest to weakest in the model itself, so a summary that
sorts by state reads as a ranking, and each carries its own one-sentence meaning there
rather than letting each surface invent a gloss.

## How to see it

```bash
majordomus evidence show --state proven
majordomus evidence show --state inputs_unchanged
majordomus evidence claim distribution-canonical-model   # state, meaning, execution, changed
bash test/run.sh 124_evidence                            # both sides of the distinction, in a fixture
```

The fixture is a repository of its own, because the behaviours that matter are behaviours
about commits: it records a run, asserts `proven` with a dirty tree, makes one commit that
changes nothing any claim names, and asserts that every claim is then `inputs_unchanged`
and none is `proven`.

## What it does not cover

The dependency model is deliberately shallow and non-transitive. It is the three paths the
claim writes down and nothing computed from them, so `inputs_unchanged` does not catch a
change to a file the claim does not name that breaks the behaviour anyway, a change to the
runner, the harness, the toolchain or the environment, a claim whose `implementation` names
one file where the behaviour lives in several, or a behaviour that depends on something
outside the repository entirely. Each is a real hole, and the answer to all of them is the
same and is not a cleverer dependency graph: record a run against this tree, and the state
becomes `proven`.

The digest is reported by `majordomus evidence proves` as `digest_matches` and is an input
to the state only in the one direction described above — a mismatch makes a claim `stale`.
It cannot make a claim stronger than the diff says it is.

## Why it exists

Both alternatives to keeping the two states apart are worse. Treating a passing run as
valid until something anywhere changes is the badge whose derivation cannot be inspected;
declaring every result stale the instant anything anywhere changes is sound and useless,
since on a repository that lands to master every few minutes no claim would ever be proven
except in the seconds after a full run. So the model says which of the two it found, and
says it in the same words on every surface. Collapsing them is precisely how a green mark
stops meaning anything, and the collapse is easy to reintroduce by accident — which is why
it is a claim with a case behind it rather than a convention.

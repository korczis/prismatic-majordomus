# A pull request that edits a projected plan record is not refused because GitHub does not show the edit yet, and the trunk is refused for it once a declared window has passed

## What it means

GitHub's issues and milestones are a projection of the plan on the trunk. A pull request
that edits a plan record, closes one, or moves one to another milestone changes what GitHub
should say, and GitHub cannot say it before the pull request has merged. The gate that
compares the two does not hold that against the pull request. It reports the records the
pull request changed as `PENDING` and refuses only drift the pull request did not cause.

After the merge the same records are `PENDING` on the trunk for a declared window, which is
the time the executor has to apply the projection. Past the window the trunk is refused and
the refusal names the command to run.

## How it works

`scripts/ci/github-check` runs `scripts/github-sync --check` and reads its drift lines. Four
states are what an apply writes: `behind`, `state`, `milestone`, and the `closed` the
adapter reports beside `state` when GitHub is the closed side. For those four the gate asks
which records changed since a reference commit.

| run | reference commit | words on the `PENDING` line |
|---|---|---|
| a pull request, or a tree given `MJ_GH_BASE=<ref>` | the merge base with the base branch | projected after merge |
| a tree the default branch does not contain | the merge base with the remote's default branch | projected after merge |
| the default branch | the youngest first-parent commit older than the window | the executor applies after landing |

A record has changed when its file differs from the reference, or when its rendering does.
The second half matters because a status is derived: closing one issue moves the body of
every issue that waited for it, and can close its milestone, without touching their files.
The gate renders the plan at the reference and at the head with `scripts/github-sync --plan`
and compares each record's body hash and status line. The reference is rendered by its own
adapter when it carries one, so a change to the renderer counts as the head's change too.

The window is `apply.window` in `.ai/repo/ci/github.yaml`, in seconds. The gate reads it from
the tree under test and carries no copy. A landing's age is the committer date of its
first-parent commit on the trunk, so a commit written last week and merged a minute ago
landed a minute ago.

`PENDING` lines are counted apart from findings. The last line reads
`github-check: 0 finding(s), 2 pending`, and the exit status follows the findings alone.

## How to see it

```bash
scripts/ci/github-check                            # this tree, judged as CI would judge it
MJ_GH_BASE=origin/master scripts/ci/github-check   # as a head that merges into origin/master
scripts/github-sync --plan                         # offline: the rendering the gate compares
bash test/run.sh 1038_a_head_is_judged_against_what_it_changes
```

## What it does not cover

Nothing applies the projection. After a landing that changes plan records the executor runs
`scripts/github-sync --apply` from master ([Pull-request integration](../INTEGRATION.md)),
and the gate only bounds how late that may be. A job that applies on every push to master is
the owner's open choice and does not exist.

Only the four states an apply writes can be pending. A hand-edited region (`edited`,
`conflict`) and a remote issue claiming an unknown record (`unmanaged`) are refused on every
run, whoever changed the record.

A pull request is not excused for drift the trunk left behind. If a landing is still inside
its window and unapplied, another pull request that did not change those records is refused
for them until somebody applies.

A new record is `missing`, not `behind`, and `missing` is ratcheted against
`.ai/repo/ci/github-drift-baseline.txt` and is never pending
([the neighbouring claim](github-projection-gated.md)).

What cannot be measured is not assumed. With no base that resolves, no merge base, a shallow
clone or no declared window, the gate prints a `NOTE` line saying which and refuses
everything it refused before this claim existed.

## Why it exists

On 2026-10-10 pull request #855 changed one line of issue I1900 and failed with `DRIFT
behind issue I1900 (#753) body is behind the canonical record`. Pull request #820 closed
eight records and failed with `8 record(s) state`. Neither could go green on its own head:
the only way to apply the change was to merge it, and the gate refused the merge for the
change not being applied. The way out was to not edit the record, which meant the plan could
not be corrected by a pull request. Had either change landed anyway, the trunk's own run
would have been red until somebody applied by hand, and a release tag waits for that run.

## Evidence

`test/cases/1038_a_head_is_judged_against_what_it_changes.sh` drives the gate over a fixture
repository with a base, a head and two landings, against a scripted `gh` that serves the
remote and fails on any call that is not a read. It proves that a record the head changed
is pending and the run exits 0, that a record it did not change is refused beside it, that
a record whose rendering moved with an untouched file is pending, that a landing inside the
window is pending on the trunk and the same landing two hours old is refused with the remedy
named, that the window is the declared one, and that a base or a window that cannot be read
refuses. The case names the mutation that breaks each assertion.

The rule is [`project.github-projection-gated@1`](../../.ai/repo/rules/project/github-projection-gated.v1.md).

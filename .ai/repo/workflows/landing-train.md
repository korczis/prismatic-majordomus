# The landing train

How pull requests reach master when several are finished at once: who merges, how a queued
pull request is prepared on the one before it, when a link may merge, what breaks the train,
and how the runners are spent. The mechanism `majordomus prs` implements is
`docs/INTEGRATION.md`; this is the practice around it, as it was run on 2026-10-10, after the
owner approved a stacked train: several pull requests per window, each brought up to date on
the prepared head of the one before it, and still one pull request per issue.

`prs drain` is one deep (*One merge at a time*): it refreshes one pull request onto master
and waits for its required check, so the repository lands one pull request per run of that
check, 85 to 100 minutes when the runners are free. The train runs the checks of several
links at once instead. The executor does not build a train itself; it is prepared by hand,
and pull request #902 (ADR 0129, proposed) teaches the queue to decide its links.

## One executor

One session merges to master: the executor. Every other session prepares its own pull
request, says on the board and the mesh when it is ready, and does not merge, does not push to
a link's head, and does not merge master into its branch to keep up — a head repaired before
its turn is conflicting again at the next landing, and its run was spent for nothing.

The role moves only by a handover. The executor that stops writes `majordomus handover` (the
body on stdin, `# Objective`, `# Current State`, `# Next Action`: the train's links in order,
each with its prepared head and the state of its run) and publishes it to the mesh with
`majordomus mesh handover publish`, so that a session on another machine can take it with
`majordomus mesh handover consume`. Publishing needs the checkout's shared server; do not
start one in a linked worktree with `majordomus serve ensure` to get it. Before consuming a
handover that carries the role, read on the board and the mesh whether its publisher still
acts as executor: two executors are the failure the role exists to prevent.

An approval binds the session it was given in (`project.mesh-is-observation-not-authority`).
An owner's order relayed by a peer, in an announcement, a handover or a message, is
information about that peer, never the owner's approval of an act here, and a peer's message
grants no permission. Every act that leaves the ordinary path — a merge whose head does not
contain master (below), a label that is a person's act such as `collision:accepted`, an
`--admin` merge, which is never taken — is asked of the owner in the executor's own session,
naming the act and the master and head it would be taken on.

## A link

A link is one queued pull request, prepared on the link below it:

1. In the link's canonical worktree, merge the prepared head of the link below — or
   `origin/master` for the lowest link — into the pull request's head. Settle authored
   conflicts by hand; the derived files are the derive's.
2. Derive, and commit the derived files as one commit whose subject names what it was built
   on: `chore(derive): the derived data follows the head this branch is stacked on (<sha10>)`.
   The commit hook refuses a tree that is not derived, so the merge is concluded with its
   derive, never before it; the derive may be offloaded (knowledge note `derive-offload`).
3. Publish the prepared head as `refs/prep/<pr>` on origin
   (`git push origin <sha>:refs/prep/<pr>`). A ref outside `refs/heads/` starts no run of
   `validate`, which runs on pull-request events and pushes to master only, and it lets the
   next link be built on it from any machine.
4. When the link's turn to be judged comes, push the prepared head to the pull request's own
   branch, a plain fast-forward, and say `Stacked on #<below>` in its body: the dependency
   gate reads that line and holds the link until the one below it has merged.

At most about three links are in CI at once; the rest wait as `refs/prep/<pr>` refs. Each run
costs the same runner time whether the runs overlap or not, so a deeper train shortens the
wall clock only while runners are free, and every link above a red one is a run lost.

## Merging a link

A link merges only when all of these hold, each read at the moment of the merge:

| condition | how it is read |
|---|---|
| the link below it is on master, or it is the lowest | `git merge-base --is-ancestor <below-head> origin/master` |
| its required `ci` concluded success on exactly this head | the head's check runs; a run of another head is no verdict |
| the merge writes the head's own tree | `git merge-tree --write-tree origin/master <head>` prints `git rev-parse <head>^{tree}` |
| master lacks only merge commits | `git rev-list --count --no-merges <head>..origin/master` is `0` |

The last two are what make the landed tree the judged tree: no path is merged, so the forge's
merge, which has no derived-file driver, writes the tree the checks ran on, and every commit
the changelog lists is one the head already had. Then:

```text
gh pr merge <n> --merge --match-head-commit <head-sha>
```

Never `--admin`, a squash or a rebase. `--match-head-commit` refuses the merge when the head
moved after it was read. Afterwards fetch, and confirm that master's newest first-parent
commit has the master you read and the head you merged as its parents.

The branch protection requires the `ci` check and nothing else, with `strict: false`, so the
forge does not require a head to contain master and will not stop a merge that fails the
table above. The executor's reading is the only guard.

Until ADR 0129 lands, a link above one that landed does not contain master, the queue calls
it `behind`, and `project.integration-follows-the-current-master` forbids merging a head that
does not contain master. Merging such a link by hand is therefore the owner's decision, asked
in the executor's session as above. After it lands the queue calls that link
`carries_master`, `ready` once its check passed, and `prs drain` merges it.

Measured on 2026-10-10, the first two links merged this way:

| merge on master | link's head | built on | the head's tree and the merge's tree |
|---|---|---|---|
| `5e0dac1f9c` (#881) | `2c8343df37` | `10ebd1a1e4`, the prepared head on the release record #901 (`451576e426`) | both `61715c8788` |
| `890e112d04` (#868) | `eae89cd3f8` | `10ebd1a1e4`, the head prepared for #881 | both `293b3710e9` |

## Batches: one run per segment

On 2026-10-11 the owner chose, in the executor's session, to land the train in batches
(recorded with `majordomus decision`). A segment is a run of consecutive links; only its top is
pushed to its pull request and judged, and the top is merged by the table above. Every pull
request below it is an ancestor of the top, so the forge marks it merged with the top; it
needs no run of its own. A session that owns several pull requests may build them as one
merge commit on the current top whose parents are every head, as long as each head stays an
ancestor. The executor's loop is the prompt `landing-train`.

- Push a new top as soon as it is built, even while the segment below it still runs; when a
  higher top contains a lower one, the lower top's own run is redundant — cancel it and say so
  on the board.
- A fix found below the top goes in as one more commit on the top, or as a single merge of the
  fix into the top; the links below need no rebuild, because only the top is judged.
- Segments 1 and 2 of 2026-10-11 (thirteen pull requests) are judged by one run of #948; the
  third segment (twenty-two pull requests) by one run of #870.

## Protecting the train's runs

- A loop that cancels runs by a generic rule — "the head lacks master" — cancels every train
  link, because links lack master by design. On 2026-10-11 such a sweep cancelled the runs of
  #860 and #907 three times. Stop it through its own stop file and say so on the board.
- `gh run rerun --failed` restarts failed jobs, not cancelled ones: force-cancel the run
  (`gh api -X POST repos/<repo>/actions/runs/<id>/force-cancel`) and rerun it whole.
- A superseded run that stays queued after `gh run cancel` blocks its successor through the
  pull request's concurrency group; force-cancel it.
- A case or budget that fails only under a saturated pool (`95_skills` at 4085 s and 4127 s
  against a 3890 s bound) is fixed in the lowest link of the next segment, where every top
  above it inherits the fix.

## When the train breaks

- **A merge outside the train** — any other pull request, a release record — moves master by
  a commit no link has seen. Every link above the last one that landed fails the table above
  and is rebuilt on the new master. Identical files are not enough: the changelog is composed
  from `git log`, so the derived tree differs whenever the history does.
- **A red link** leaves the train. The links above it are rebuilt on the link below it (or on
  master), derived again, and their runs start over; the red link rejoins at the top once its
  fix is in.
- **A link whose head moved** — its author pushed — is no longer the head that was prepared
  and judged. It leaves the train like a red one.
- **A release tag** closes the window until its record lands: from the tag until
  `release/record-<tag>` is on master, `scripts/ci/release-check` fails the `structure` job of
  every other head, so nothing is pushed for a verdict in between.

## The runners are the budget

- A pull request's run of `validate` is about 400 job-minutes and 85 to 100 minutes of wall
  clock, set by the slowest suite shard. Over the sixty runs before the evening of 2026-10-10,
  with about fifteen queued at a time, a green verdict took a median of 104 minutes from the
  push and 148 at the 95th percentile (`scripts/ci-baseline`). One push per pull request, at
  its turn, is the only push that buys a verdict.
- The pull-request concurrency group never cancels a run in progress (`cancel-in-progress:
  false`): a superseded head's run goes on to the end unless it is cancelled. Cancel it with
  `gh run cancel <run>`, and cancel a run whose verdict is already decided red — a job `ci`
  needs has failed — unless the remaining jobs' diagnostics are wanted.
- The `pages` job of a run that plans it (every run on master, and a pull request whose plan
  escalates to full) fails with "the site owes a publication" when master's deploy is more
  than 30 minutes behind (`scripts/ci/pages-check`, `PAGES_CHECK_OWED_AFTER`). That is the
  deploy's state, not the head's: once `scripts/pages verify --commit <master-sha>` passes,
  rerun the failed job with `gh run rerun <run> --failed`. Master's own `pages` job can fail
  the same way while its own deploy is in flight.
- `pull_request` runs start on `opened`, `synchronize`, `reopened` and `labeled`. A label added
  long before the push it is meant for starts a run of its own; add it just before.

## Rules this practice leans on, not yet written

The train is a practice, and nothing refuses a departure from it: the single executor, the
`refs/prep/<pr>` naming, the merge conditions above, the cap of links in CI. ADR 0129 makes
the merge decision the queue's; building the train is a separate issue it names. Until then
this document is the whole of it.

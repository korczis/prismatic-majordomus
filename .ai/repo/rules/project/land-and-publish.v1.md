---
id: project.land-and-publish
version: 1
kind: rule
title: Work lands often and small, and what is published is master and only master
description: A branch merges to master as soon as it is green rather than waiting for a batch; master is merged into it locally first, so the derived merge driver resolves what GitHub cannot; the published site is built from master alone; the commit gh-pages names is an ancestor of master; and the person who merged is the person who checks that it reached the public.
statement: Merge to master as often as you can, one branch at a time, with master merged in locally and pushed before you ask; publish master and nothing else; and after every merge the merger confirms what the public is actually being served.
status: active
class: blocking
depends_on: [project.derived-files-regenerated@1, project.interfaces-are-projections@1]
tags: [process, integration, deployment, evidence]

x-majordomus:
  tests: [test/cases/104_published_site.sh, test/cases/97_pages_fast_path.sh, test/cases/57_derived_merge_driver.sh, test/cases/111_unblock.sh]
---

# Rationale

Eight sessions worked on this repository in one afternoon and almost nothing reached the
public. The site served version 0.3.1 with no changelog while master carried three releases'
worth of change, seven pull requests stood open, and an integration branch cut at 17:52 was
stale by 18:00. None of it was a disagreement about the work. Every part of it was a
mechanical failure that nobody could see from inside their own branch, and each was measured
rather than argued:

- **The conflicts were not conflicts.** Two pull requests were marked `CONFLICTING` on the
  same ten files, and ten of the ten were `merge=derived`. Not one was a source file. This
  repository has `scripts/merge-derived` and it resolves them; GitHub cannot run a custom
  merge driver, so the server reports conflicts that exist on nobody's machine. A local
  merge of master took one of those pull requests to zero conflicts with nothing resolved by
  hand. A previous integration here went from thirty-three conflicts to zero the same way.
- **The driver is per clone.** A worktree that never ran `just derive-merge-driver` fights
  those same files locally too, which is why they appeared to fight everyone.
- **A batch costs more than its conflicts.** Every conflicted merge needs a full `just
  derive` before the pre-commit hook accepts it, and any source fix made after that derive
  invalidates it. One session ran three derives for one merge.
- **The public site drifted from the repository.** At 17:46 UTC it served a commit from an
  unmerged branch — the result of `site-deploy --any-ref`, which documents itself as a
  preview, pointed at production by two different sessions, this one included.
- **A merge broke the publication and nothing said so.** A change to a document's shape that
  was never rendered locally shipped a page the site generator could not build, the pages
  run failed, and the public site silently stayed on the previous commit.
- **Green and not-yet-reported looked identical.** No validation run on master completed for
  over four hours while master carried two defects that broke every branch merging it.

# Required behaviour

1. **Land often and small.** A branch merges to master as soon as it is green, one at a
   time. An integration branch that batches many is a last resort for a backlog that already
   exists, is declared as such, and is not the pattern afterwards.
2. **Merge master locally, then push, before asking for review.** Run `just
   derive-merge-driver` once per worktree. Resolve source conflicts, fix the build, *then*
   derive once, then commit — a derive before the last source fix is a derive done twice. A
   derived file is never resolved by hand: it is regenerated, because a hand-resolved
   derived file passes the merge and fails `derive-check` one commit later. The server's
   view does not change until the local merge is pushed. `scripts/unblock <branch>` (`just
   unblock`, and it takes a pull request number) is that whole gesture as one command, in a
   detached scratch worktree that never touches the branch's own: it merges the trunk in
   where the driver exists, refuses and names any conflict outside the `merge=derived` set
   rather than deciding it, derives, commits and pushes the branch. It exists because a
   ten-minute manual gesture performed several times a day is one that gets skipped under
   pressure, and a branch nobody unblocks is a branch somebody merges without reading.
3. **A mechanical union is wrong across generations.** Taking both sides is right for a list
   that gained entries and wrong when one side is an older generation of the same code:
   folding one branch that way reintroduced a second match arm built against a previous
   shape, and seven compile errors with it.
4. **Publish master and only master.** `scripts/site-deploy --any-ref` is a local preview.
   The commit gh-pages names is an ancestor of master, always.
5. **The merger checks what the public is served.** Equality is not the test — a merge
   landing during a deploy window leaves the published commit an ancestor rather than the
   head, with nothing wrong. The merger confirms, within a bounded wait, that the site is
   serving the head they merged, using `scripts/pages verify --url … --commit "$(git
   rev-parse …)"` rather than a second comparison written by hand, and never an abbreviated
   SHA: that comparison is string equality against forty characters, so a short one burns the
   whole timeout and then reports a mismatch that is not one. A failure names both the
   commit expected and the commit served.
6. **The deploy belongs to the merge.** One owner per merge, so that two sessions do not
   publish over one another.
7. **A shape change is rendered before it is merged.** `scripts/site-build && scripts/site-check`
   on the head, before the merge, whenever a change touches a document's shape or the
   templates that render it. The pages workflow does not re-run those checks, so nothing
   between the merge and the public catches it.
8. **A merge that changes the public contract carries its version.** Landing without the
   bump is why the published site named a version three releases behind the code.

# Failure behaviour

`scripts/ci/pages-check`, registered as the `pages-live` gate, decides the published half.
Four things, because publication has failed in four ways here: the commit gh-pages names must
be an ancestor of master; master must not have moved past it, on a path that can change the
site, for longer than the deploy window; the commit the live pages name must be the one
gh-pages published — the failure a token-authored push causes, where no workflow starts at
all and every check on the tree stays green; and GitHub's own build of gh-pages must not have
errored, which is the half of publishing this repository does not own and which no workflow
of ours goes red for.

The second of those is clause 5 made mechanical, and it is the one that was missing. Ancestry
alone was a one-way check: a publication that is on master is on master however old it is, so
on 2026-09-10 the gate said `ok` all afternoon while the site sat hours behind. The window is
what separates the deploy in flight that clause 5 excuses from the deploy that is not coming;
it is thirty minutes, sized from a twenty-five-minute Actions queue observed that day, and a
commit that cannot change the site owes no publication at all — the paths that can are
`scripts/pages paths`, which is also the publication workflow's own trigger.

It runs in the full plan beside `installer-live`, never in a path-triggered one: a
live-network gate on every push fails collectively when an origin is slow, and a gate that
cries wolf is waived.

The rest is decided by review and by the record on the shared server's board. A gate cannot
tell that a branch waited for a batch, and inventing one would be a worse cure than the
disease.

# Verification

`test/cases/104_published_site.sh` holds `scripts/ci/pages-check` to what it claims, against
fixture repositories rather than the network: a publication from the trunk is accepted, one
from a branch that is not on it is refused with the cause and the remedy named, a trunk that
has moved past the publication is accepted while the deploy is young and refused once it is
older than the window — both over one fixture, so the judgement is the age and not the tree —
a deploy commit that does not say where it came from is refused as unauditable rather than
guessed at, a measurement the gate could not make is reported as a note and never as an `ok`,
and the gate carries neither a URL nor an identity path of its own — both are read from
the declarations that already hold them. The case was proved non-vacuous by removing the
refusal from the gate and watching it fail.

The live half is not simulated: a case standing up an HTTP server would be testing its own
fixture, and one reaching the real site would fail whenever the network did. `--offline` is
the seam, and the case asserts the seam announces itself rather than skipping silently.

`test/cases/97_pages_fast_path.sh` holds the publication path itself, including the two
halves of its concurrency: a push cancels the run it supersedes, a dispatch — the recovery
deploy — cancels nothing, and the run states whether it pushed gh-pages before it ended,
because a cancelled run is not a failure and fourteen of the seventeen cancelled runs of
2026-09-09/10 had already published. The
derived-merge-driver behaviour is `test/cases/57_derived_merge_driver.sh`, and
`test/cases/111_unblock.sh` holds `scripts/unblock` to clause 2: a conflict on an authored
file is refused with the files named and the branch left where it was, the scratch worktree
is gone on every path including the refusals, a dry run pushes nothing, and the branch is
never checked out. It was proved non-vacuous by making the script classify every conflicted
path as derived and watching the authored refusal disappear.

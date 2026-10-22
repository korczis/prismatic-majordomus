+++
title = "Claimed before it is built"
description = "Several AI sessions working on one repository will sooner or later build the same thing twice, or take the same number. Why announcing what you do was not enough, what a session asks of every branch before it builds, and why an identifier is surveyed rather than guessed."
template = "page.html"
[extra]
lang = "en"
related = ["seats-not-names/", "everything-followed/", "written-once-derived-everywhere/", "a-session-ends-the-work-does-not/", "one-branch-one-place/"]
[extra.translations]
cs = "zabrano-driv-nez-se-stavi/"
+++

On 9 September 2026 eight sessions worked in this repository, and two of them independently built a release subsystem. They used the same module path, the same command group and the same generated changelog, and they kept at it for several hours. Both had announced what they were doing. Neither announcement existed when the other session started.

The answer had been on record the whole time. A branch pushed before either session began already carried `apps/majordomus-cli/src/release/`. Nothing asked it. Two designs that arrive at one path are not a merge conflict, settled by combining two diffs. They are a choice between two finished pieces of work, and the one that loses was written for nothing.

## A board is a memory, a branch is a record

Every session announces what it is doing and which paths it expects to touch, and every other session can read those announcements on the peer board. The board answers who is working now. It cannot say what was built an hour before the process that serves it started, because a board is held in a process's memory.

[ADR 0044](@/adrs/adr-0044.md) (accepted) fixed the part of that which was simply wrong. A server serves the checkout it was started in, and a linked worktree has a server of its own, so one server's board was never the repository's. The board a session reads is now gathered: every checkout git registers is asked for its own board, and the answer says which checkouts it covered and whether it could read all of them. A short board is never presented as an empty repository.

A gathered board still sees only the sessions that are running. A branch pushed yesterday carries no announcement and no less of a claim.

## Paths are asked of every branch

So before a session creates a module, a command group, a document kind or a generated artifact, it asks git. `scripts/collision-check --new <paths>` reads every local and remote branch for a version of those paths that differs from the base, and names each branch that carries one, with its last commit and how long ago it landed. The rule is [project.work-is-claimed-before-it-is-built](@/rules/project-work-is-claimed-before-it-is-built-1.md).

The check separates a collision from related work. A rescue copy with the same bytes is listed and passes. So does a branch built on the other one, or a copy of the same work that has since diverged. Only a path that two branches added independently, with different content, collides. It also refuses to answer when it cannot see: a shallow clone, or a base that does not resolve, gets its own exit code and never reads as "nothing claims this".

Until 13 September nothing ran it. The provider bootstraps named it, and no hook, recipe or workflow called it. Now a gate asks it on every pull request, over every path the pull request adds, and refuses a collision. A person can accept one deliberately, with the label `collision:accepted`, because two implementations are sometimes worth comparing, and nothing mechanical can decide that. The collision is still printed for the reviewer.

## Numbers are surveyed, never guessed

Paths are half of it. The other half is identifiers: an ADR's number, a behavioural case's number, an issue id. On 11 September three branches claimed ADR 0044 and two claimed 0043, and three separate sessions each wrote the same survey to escape it.

That survey is now a command. `majordomus adr next` reads every ref, every tag, every sibling worktree's working tree and the peer board, reports what each of them claimed, and says so when one could not be reached. It allocates above the highest number any of them has seen, never into a hole, because a hole is a number that was taken and withdrawn or taken and not yet written, and no survey can tell those apart. `majordomus adr check` refuses a branch that adds a decision under a number another ref already gives to a different decision, and the pre-commit hook runs the same check through `doctor`. [ADR 0053](@/adrs/adr-0053.md) (proposed) is the reasoning.

Case numbers got the same treatment, for the same reason. On 12 September a survey of every ref found that nearly a third of the case numbers ever added had been carried by more than one file, and that three sessions had taken one number within minutes of each other. `scripts/case-numbers next` asks the same kind of survey: this working tree, the sibling worktrees, every ref and the peer board. A gate on every pull request refuses a number two files share, except for the collisions recorded when the gate was written, which sit on a baseline because renumbering them would break other sessions' branches.

## Announce the number, not only the path

The two halves are not interchangeable. A branch scan answers what has been pushed. Only the board sees a number that a session decided on a minute ago and has not yet written anywhere. So an announcement names the identifier being allocated as well as the paths, and a session reads the board before it takes one.

When work must not overlap across machines, an announcement is not enough either. On the mesh ([ADR 0067](@/adrs/adr-0067.md), proposed) a board announcement travels to every linked machine as an advisory claim. Work that must be exclusive is claimed exclusively, and a second session asking for it is refused with the name of the claim it meets. That is how the roles of integration are held, as [Seats, not names](@/seats-not-names.md) describes.

## What a claim is not

A claim on the board is not a lock. The overlap check reports, and nothing it says stops a commit. What refuses a change is the task's own scope, which `majordomus check`, `majordomus finish` and the pre-push hook hold the change to. The rule that asks for the branch scan stays advisory as a habit, while its parts that can be judged mechanically, a path two branches added differently or a number two decisions share, are gates.

The cost is a moment before each new path and a line on the board before each new number. The alternative was measured in afternoons: two sessions building one subsystem, and three sessions writing one survey to escape one collision. How sessions on several machines share their claims is described in [`docs/MESH.md`](@/docs/mesh.md), and where each branch is worked in by [`docs/WORKTREES.md`](@/docs/worktrees.md).

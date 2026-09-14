---
schema: adr/v1
id: adr-0054
kind: adr
title: A commit message is a judged value, not a convention and not a prompt
status: proposed
date: 2026-09-12
tags:
  - git
  - commit
  - governance
  - projections
related:
  - rule:project.conventional-commits
  - rule:project.interfaces-are-projections
  - rule:project.blocking-checks-cheap
  - rule:project.no-claim-without-test
  - claim:commit-message-is-judged
  - claim:commit-scopes-are-learned
  - claim:commit-plan-is-refusable
  - file:docs/COMMIT.md
  - file:.ai/repo/policy.yaml
  - file:.ai/repo/rules/project/conventional-commits.v2.md
  - file:.ai/repo/commit-policy-baseline.txt
  - file:.githooks/commit-msg
  - file:scripts/ci/commit-policy
  - file:apps/majordomus-cli/src/commit/mod.rs
  - file:apps/majordomus-cli/src/capability/builtin/commit.rs
  - file:apps/majordomus-cli/src/release/commits.rs
  - file:.ai/repo/adrs/0048-a-rule-declares-how-it-is-enforced-and-review-is-one-of-the-three.md
  - test:test/cases/274_commit_policy.sh
provenance:
  origin: authored
---

# 54. A commit message is a judged value, not a convention and not a prompt

## Context

This subject existed in three places and none of them knew about the other two.

The repository could **read** a commit. `release::commits` has parsed conventional subjects
out of the log to build the changelog since the changelog existed, resolving `I1305` and
`M000` against what the layer holds and dropping an id that resolves to nothing.

The repository could not **judge** one. `project.conventional-commits` at version 1 stated
the convention and its failure behaviour read *"No command decides this rule; a reviewer
does."* That sentence is the shape of defect the rest of this governance exists to remove —
a stated invariant with no verdict behind it — and it was measurably not working. Over 1161
non-merge commits the median subject is 73 characters, 48 exceed even the width this project
declares for a line of anything, and 38 carry a type word no vocabulary contains. Nothing
noticed, because nothing was looking. The rule was one of the review-enforced set ADR 0048
made countable: visible, stated on every run, and waiting for somebody to write the case.

The third place was outside the repository altogether. The *procedure* for writing a commit
— derive a scope from the path, link the work, do not invent an issue number, a fix needs a
test — lived in prose that an AI client read and carried out by hand, one file per client,
with a path-to-scope table maintained by a person. A workflow whose semantics live in a
prompt is unversioned, unenforceable, untestable, and true for exactly one tool. The table
is also derivable and nobody had said so: every commit in the history is a worked example of
which scope a set of paths belongs to, decided by whoever made the change and kept by
whoever reviewed it.

What was missing was never a grammar. It was a judge, a moment for the judge to speak, and a
declaration for it to speak from.

## Decision

**The commit message is a typed value of the executable, with one grammar.**
`commit::CommitHeader::parse` is total and never fails; the changelog and the judge read the
same parse and disagree only about what to do with a header that is not conventional. For
the changelog that is not a failure — a commit nobody spelled conventionally still happened,
and a changelog that dropped it would lie about what the release contains. For the judge it
is one finding, `commit.not_conventional`, and its consequences are deliberately not
reported as findings of their own: somebody fixing `update stuff` does not also need to be
told its scope is unknown. There is no second regular expression in a hook, a gate or a
script.

**The policy is data, declared once.** `commit:` in `.ai/repo/policy.yaml` carries
`subject_max_chars` and three levels; the schema `majordomus.policy/v1` owns its shape and
`share/allow/policy.txt` is generated from it. `subject_max_chars: 100` is measured, not
chosen: the git convention is 72, this history's median is 73 and its 95th percentile is 98,
and a limit half the history violates is not a limit but noise that teaches people to ignore
the check. The type vocabulary is deliberately **not** in the policy — it is `ChangeKind`,
which the changelog already renders headings from, and a second copy of those words is the
duplication this decision removes.

**The scope vocabulary is observed, not declared.** `commit.scopes` learns which scopes this
repository uses, how often, and about which directories, from at most 1500 commits of its
own log, counting each scope's association with directory prefixes four segments deep — four
because this repository's source lives at `apps/majordomus-cli/src/<subsystem>/` and three
stops one level above the subsystem, where a dozen scopes all have commits. A scope outside
the vocabulary is a **warning**, never a refusal: inference must not refuse the first commit
of a subsystem it has never seen. A table has no way to be wrong; a vocabulary read from the
history is right the day a subsystem is committed and sinks on its own when nobody touches
it for a year.

**The judgement happens where its evidence is.** `.githooks/commit-msg` asks
`commit.validate` with the staged paths — the one moment the message and the files both
exist, which is why `fix_requires_test` can be asked there and nowhere else. The gate
`scripts/ci/commit-policy` reads the history through `commit.history` and knows no paths, so
it does not make that finding at all: a judgement whose evidence is absent is not made rather
than guessed. The hook carries no logic; it asks.

**Four capabilities, declared once and projected.** `commit.scopes`, `commit.plan`,
`commit.validate` and `commit.history` are declared in
`apps/majordomus-cli/src/capability/builtin/commit.rs`, and the command line, the MCP tool
and resource, the HTTP route, the OpenAPI operation, the completion and the capability
reference are derived from that declaration, per `project.interfaces-are-projections`.
**Making the commit is deliberately not among them.** `git commit` is a repository mutation
and the exposure policy stops every machine surface at local mutation: an agent may ask what
a commit would be and whether a message passes; a person commits. The planner and the judge
are pure functions of a tree and a message, which is what lets them be asked from anywhere.

**The plan is fingerprinted and states its own reasoning.** The subject a plan proposes is
empty on purpose — what a change *did* is the one thing no evidence in the tree can state,
and a planner that wrote a confident sentence would be writing fiction into the history. The
grouping is a recommendation with its rationale attached, never an automatic split. The
fingerprint is the repository, the worktree, HEAD and a hash over every change with its stage
and status, because several workers share a checkout here and more share a repository, and a
plan computed at one HEAD and executed at another commits files somebody else staged under a
message about work that is no longer what changed. It is not a lock and it does not prevent
the race; it refuses to be the one that loses it silently.

**The rule becomes blocking at version 2, over a recorded baseline.** 54 commits of this
history do not satisfy the policy. Every one is published, so rewriting them is not on the
table: the count lives in `.ai/repo/commit-policy-baseline.txt`, the gate fails when it rises
and reports when it falls. Commits a branch *adds* are held to the policy outright with no
baseline, because the hook refused those messages while they were being written — a failure
there means the hook was bypassed or the policy moved.

**Git's own subjects are exempt, not wrong.** `Merge …`, `Revert "…"`, `fixup!` and
`squash!` are composed by git, and a validator that refused them would be refusing git. A
`merge:` subject a person wrote is *not* exempt: this repository writes those deliberately
for integrations, so `merge` is a declared type word and those commits are held to the same
width and the same references as every other.

This does not contradict ADR 0051, which demoted conventional commits from *authority* over
the release version to *evidence* for it. That decision was about what may decide a version
number: the public contract, measured, rather than a sentence somebody typed. This one is
about whether that sentence is legible at all. A commit subject is a poor authority and a
good record, and it is only a good record if something reads it.

## Alternatives rejected

**Keep the procedure a prompt in each AI client.** The status quo, and the most tempting
option because it already existed and cost nothing to leave alone. Rejected on four counts,
each fatal on its own: it is true for exactly one tool, so every client needs its own copy
and the copies drift; it is unversioned, so nobody can say what the procedure was on the day
a commit was written; it is unenforceable, so a client that ignores it is indistinguishable
from one that follows it; and it is untestable, so nothing can fail when it is wrong. The
path-to-scope table inside it was the concrete proof — a person maintained it, it was correct
on the day it was written, and a directory rename could make it name a scope the history had
stopped using with nothing to notice. A workflow whose semantics live in a prompt is a
workflow this repository has decided not to have.

**Keep the rule advisory and let a reviewer decide.** Also the status quo, and the honest
version of it is that a reviewer *was* deciding, and the measurement says what that produced:
48 over-width subjects and 38 unknown type words nobody remarked on. Rejected because the
rule's own failure behaviour — *"No command decides this rule; a reviewer does"* — was not a
declared exemption in ADR 0048's sense but a description of an absence. It named no
`reviewed_because`, and there was no reason it could have named: the grammar is mechanical,
the width is a number, and a record id either resolves or it does not. ADR 0048's rule
applies exactly — an executable proof always wins, and a rule that acquires a case stops
being review-enforced. Leaving it advisory to avoid the cost of the case would have been
choosing the fake green.

**A `types:` key in the policy.** The obvious shape, and it would have put the type words
beside the width where a reader expects them. Rejected because `ChangeKind` already holds
them for the changelog, and a second copy is precisely the duplication this decision exists
to remove: the two would disagree the first time either was extended, and the disagreement
would surface as a commit the judge refused and the changelog rendered.

**A path-to-scope table in the repository rather than in a prompt.** Moving the table from
prose into `.ai/` would have made it versioned and shared, which is two of the four
objections answered. Rejected because it leaves the one that matters: a table has no way to
be wrong. It is correct when written and silently stale thereafter, and the evidence for what
it should say — every commit ever made — is already in the repository and needs no
maintenance.

**A regular expression in the hook.** The cheapest possible commit-msg hook, and the reason
most repositories' commit conventions are enforced twice. Rejected because there would then
be two grammars: the one the changelog reads and the one the hook refuses with. They would
agree on the day the hook was written and not afterwards, and the failure mode — a commit
accepted at write time and dropped from the release notes — is invisible until somebody reads
a changelog looking for work they remember doing.

**Rewrite the 54 failing commits.** Every one is published, so rewriting them rewrites
history other clones and every pull request reference already hold. Rejected; the baseline is
the honest alternative, and it ratchets.

**Refuse an unknown scope.** Symmetric with the errors, and wrong. The vocabulary is
inference over a corpus, and the first commit of a new subsystem is exactly the case where
the corpus is empty — refusing it would make the mechanism hostile at the only moment it has
nothing to say. A warning is the finding that matches the confidence.

**Let the plan write the subject.** A planner that can see the diff can compose a sentence
about it. Rejected: it would be fiction with the authority of a tool behind it, and a history
of generated subjects is a history that says nothing a `git diff` does not already say.

**Expose `commit.apply` — let an agent commit.** Rejected on the exposure policy this
repository already has: every machine surface stops at local mutation. The value of this
subsystem is that it is a pure function of a tree and a message — it works offline, it cannot
half-succeed, and every answer can be recomputed and checked. A capability that ran `git
commit` would have none of those properties and would be the one operation in the registry
whose failure leaves the repository in a state the caller did not ask for.

## Consequences

**A commit can now be refused.** The `commit-msg` hook exits 10 and git aborts, printing
every finding with its code. That cost is ~0.8 s per commit and is paid by every worker in
this repository, human and agent. The gate costs 1720 ms over 1788 commits. Both satisfy
`project.blocking-checks-cheap`; the hook was sized to the moment it runs in, and neither
reaches a network.

**The hook is declared, not assumed.** It is registered under `enforcement:` in
`.ai/repo/policy.yaml` as `commit-policy-on-message`, so `majordomus doctor` reconciles it:
the path must resolve, be executable, and be invoked by `.githooks/commit-msg` without its
exit code being swallowed. A checkout that unwires the hook is a doctor failure rather than a
checkout that silently stopped judging.

**A rule left the review-enforced set.** `project.conventional-commits` moves from `class:
advisory` with no proof to `class: blocking` naming `test/cases/274_commit_policy.sh` and
`scripts/ci/commit-policy`. Under ADR 0048's ordering that makes it gated, and the
`review_only` count falls by one. The version bump to 2 is the mechanism that says the rule's
meaning changed rather than its wording.

**The policy schema grew a section.** `commit:` is a new key of `majordomus.policy/v1`, so
`share/allow/policy.txt` and the JSON schema beside it are regenerated, and a policy using
the key against an older executable is refused as unknown. That refusal is correct and is
what the generation guard is for; it is also why this change cannot be applied to a tree
without regenerating first.

**The baseline is debt that is now visible and bounded.** 54 is a number this repository can
watch. It can fall — every branch that lands adds commits held to the policy outright — and
it cannot rise. A checkout with no trunk to compare against reports that it measured nothing
rather than reporting a pass, so the gate cannot go quietly green in a shallow clone.

**The judge is deliberately narrow.** It does not decide whether the subject is a good
sentence, whether the change is atomic, or whether the work was worth doing. A reviewer
decides those, and a validator that pretended to would be a validator nobody trusts. The rule
therefore stays wider than the gate, which is the honest relation between the two.

**One risk is accepted about the learned vocabulary.** It is inference over the repository's
own past, so it will reproduce a scope the history used badly, and a subsystem that is
renamed carries its old scope for as long as the window holds commits about it. Both are
warnings and neither refuses anything, which is the containment; a repository that wanted the
old behaviour back would be asking for the table, and the alternatives above say why it is
not coming back.

**The plan's fingerprint does not make planning safe, only honest.** Two workers can still
stage into one checkout and one of them will lose. What changed is that the loser is told
which of the repository, the worktree, HEAD or the change set moved, in a sentence, instead
of committing somebody else's staged files under their own message.

---
id: plan-a-commit-and-judge-its-message
kind: use-case
title: 'Divide a working tree into commits, and hold every message to a policy'
summary: 'Ask what this tree would commit and why, learn the scope vocabulary from the history rather than a table, and get a verdict on one message or on a whole range before a reviewer has to give one.'
category: policy
status: active
target: advisory
weight: 45
actors: [maintainer, contributor, agent]
difficulty: intermediate
commands: [doctor, doctrine]
mcp_tools: [majordomus_commit_plan, majordomus_commit_scopes, majordomus_commit_validate, majordomus_commit_history]
doctrines: [majordomus.enforcement-wiring]
claims: [commit-message-is-judged, commit-scopes-are-learned, commit-plan-is-refusable]
responsibilities: [policy, doctor]
---

# Situation

A commit message is the only record of *why* a change was made that survives the branch, the
pull request and the person. This repository stated what one should look like and then did not
decide it: version 1 of `project.conventional-commits` ended with *"No command decides this
rule; a reviewer does."* The measurement taken the day that sentence was replaced says what
that cost — over 1161 non-merge commits, 48 subjects exceed the width this project declares
for a line of anything and 38 carry a type word no vocabulary contains. Nothing had noticed,
because nothing was looking.

The other half of the problem is not enforcement but authoring. Somebody — a person at the end
of an afternoon, or an agent at the end of a mandate — has a working tree with fifty changed
files in it and has to decide how many commits that is, what scope each carries, and what type
word is honest. The usual answer is a table of path patterns mapping a directory to a scope. A
table is correct on the day it is written; then a directory is added, a subsystem splits, and
the table names a scope the history stopped using months ago, with nothing to notice, because a
table has no way to be wrong.

The third half lived outside the repository altogether: the procedure for writing a commit —
derive the scope from the path, link the work, do not invent an issue number, a fix needs a
test — sat in prose that each AI client read and carried out by hand, one file per client. A
workflow whose semantics live in a prompt is unversioned, unenforceable, untestable, and true
for exactly one tool.

# What you run

Four capabilities, declared once in
`apps/majordomus-cli/src/capability/builtin/commit.rs` and projected onto the command line, the
MCP tool list and the HTTP routes without being registered anywhere twice. All four are
commands of the Rust executable, so the launcher is `bin/majordomus-cli`, which builds it when
it must:

- `majordomus commit scopes` (`commit.scopes`, `majordomus_commit_scopes`,
  `GET /api/v1/commit/scopes`): the vocabulary, learned from at most 1500 commits of `git log`
  and counted against the directory prefixes each scope is associated with, four segments deep.
  Read in this repository at `8998e11fd` it answers `learned from 1161 commit(s), at most 1500`
  and then `derive` with 211 commits over `site, site/data, site/data/registry`, `site` with
  134, `ci` with 60, `plan` with 44. Nothing maintains that list: a subsystem committed today is
  in it today, and one nobody has touched for a year sinks on its own.
- `majordomus commit plan` (`commit.plan`, `majordomus_commit_plan`,
  `GET /api/v1/commit/plan`): the working tree divided into the commits the history's own
  scoping supports, under a fingerprint of repository, worktree, HEAD and a hash over every
  change with its stage and status. Each group carries its reasoning — how many prior commits
  scope those paths that way, or that several scopes have equal claim, or that a change set
  holding generated files is one commit because only the last of several could carry current
  derived data (`project.derived-files-regenerated`). The subject is left as `<subject>` on
  purpose: what a change *did* is the one thing no evidence in the tree can state, and a planner
  that wrote a confident sentence about it would be writing fiction into the history.
- `majordomus commit validate [FILE] [--paths a,b] [--rev REV]` (`commit.validate`,
  `majordomus_commit_validate`, `GET /api/v1/commit/validate`): the verdict on one message,
  read from a file, from stdin, or from a commit already made. Exit 0 when the worst finding is
  a warning, exit 10 when one is an error. `feat(commit): the header is parsed once` passes with
  `warning  commit.unknown_scope`, because `commit` is a scope this history has not used yet and
  inference must not refuse the first commit of a subsystem it has never seen.
  `update stuff.` fails with `error    commit.not_conventional` and is told the eleven type
  words. `--rev HEAD` on a merge commit answers
  `exempt   git_authored: git composed this subject, not a person`.
- `majordomus commit history [RANGE]` (`commit.history`, `majordomus_commit_history`,
  `GET /api/v1/commit/history`): every commit in a range judged in one pass, exit 10 when any
  carries an error. `HEAD~5..HEAD` here answers
  `HEAD~5..HEAD range      11 commit(s): 6 exempt, 0 failing` and exits 0; the whole history
  answers `HEAD range      1755 commit(s): 561 exempt, 54 failing` and exits 10. This is what
  `scripts/ci/commit-policy` asks, which is why the gate can say
  `commit-policy: history 54 failing, baseline 54` — the debt is recorded in
  `.ai/repo/commit-policy-baseline.txt`, the gate fails when it rises and reports when it falls,
  and commits a branch *adds* are held to the policy outright with no baseline at all.

Two commands of the shell tool carry the enforcement half, and they are what the scenario runs:

- `doctor`: the `commit-msg` hook is declared under `enforcement:` in `.ai/repo/policy.yaml` as
  `commit-policy-on-message`, so the wiring is reconciled rather than assumed — the path must
  resolve, be executable, and be invoked without its exit code being swallowed.
- `doctrine list`: the enforced subset of the rule set that a shell validator decides. The
  commit rule is deliberately not in it.

# Scenario

```yaml
mode: live
given:
  - 'the layer installed and the hooks wired; the executable built, because the hook and the gate both call it'
steps:
  - id: the-judge-is-wired
    run: ['doctor']
    note: 'the hook that refuses a message is declared in the policy and reconciled against what .githooks/commit-msg actually invokes; a declared enforcement nothing calls is a failure, not a passing line'
    expect:
      exit: 0
      stdout_contains: ['^OK   wiring      commit-policy-on-message — wired via .githooks/commit-msg', '^doctor: 0 failure']
  - id: no-shell-validator-decides-it
    run: ['doctrine', 'list']
    note: 'the doctrine registry is what a lib/ validator decides; project.conventional-commits is absent from it because the judge is the executable, asked by a hook and by a CI gate'
    expect:
      exit: 0
      stdout_contains: ['^majordomus.enforcement-wiring +blocking']
      stdout_not_contains: ['^project\.conventional-commits']
then:
  - 'a rule of this project is enforced by a gate and a hook that call the executable, never by a validator added to the shell tool'
  - 'the grammar is one parser: the changelog and the judge read the same CommitHeader::parse and disagree only about what to do with a header nobody spelled conventionally'
  - 'the three levels in the policy - off, warning, error - are decisions a repository can read back; off is a value, not an absence'
  - 'test/cases/274_commit_policy.sh drives the four commands over real repositories, real worktrees and the real hook; scripts/ci/commit-policy reads 1755 commits against the baseline'
```

# Outcome

The message a person writes is judged at the one moment the message and the files both exist,
which is why `fix_requires_test` can be asked at `commit-msg` and nowhere else: a fix with no
test among its files is worth noticing while adding one is still cheap. The gate reading the
history knows no paths and therefore does not make that finding at all — a judgement whose
evidence is absent is not made rather than guessed.

Making the commit is deliberately not here. `git commit` is a repository mutation and the
exposure policy stops every machine surface at local mutation, so an agent may ask what a
commit *would* be and whether a message passes, and a person commits. That shape is what makes
the planner and the judge pure functions of a tree and a message: they work offline, they
cannot half-succeed, and every answer can be recomputed and checked.

The fingerprint on a plan is the multi-agent half of it. Several workers share a checkout here
and more share a repository, so a plan computed at one HEAD and executed at another commits
files somebody else staged under a message about work that is no longer what changed. The
fingerprint does not lock anything and does not prevent that race; it refuses to be the one
that loses it silently, and says which half moved — that HEAD moved, or that the plan is of
another worktree.

`subject_max_chars: 100` is measured rather than chosen. The git convention is 72; over the
1161 non-merge commits this vocabulary is learned from, the median subject is 73 characters and
the 95th percentile is 98. A limit half the history violates is not a limit — it is noise that
teaches people to ignore the check.

What it does not do is judge whether the subject is a good sentence, whether the change is
atomic, or whether the work was worth doing. A reviewer decides those, and a validator that
pretended to is a validator nobody trusts.

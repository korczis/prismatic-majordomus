+++
title = "A suite started from a git hook cannot write the repository the hook belongs to, and doctor names the git configuration such a write leaves behind"
description = "A case of this project's suite builds repositories: git init, git branch -M master,"
weight = 252
[extra]
claim_id = "fixture-cannot-write-its-repository"
status = "guaranteed"
source = "docs/claims/fixture-cannot-write-its-repository.md"
+++
{% raw %}

## What it means

A case of this project's suite builds repositories: `git init`, `git branch -M master`,
`git commit`, `git config user.email t@example.com`, `git worktree add`, `git tag`. Each of
them is meant for a disposable directory the runner made a moment earlier.

Git tells a hook which repository it is in through the environment. A `pre-push` hook run
from a linked worktree gets `GIT_DIR`, and every process the hook starts inherits it. With
that variable set, git does not look at the directory a command stands in: `git init` in an
empty temporary directory reinitialises the repository being pushed, and every command
after it acts there.

So the guarantee has two halves. The runner, the gate dispatcher and the coverage harness
drop every variable git reads its repository from before anything they start can run, so a
fixture acts on its own directory whoever started the suite. And `majordomus doctor`, which
runs on every commit, fails and names the state such a write leaves in a repository's own
configuration, with the command that undoes it.

## How it works

`test/run.sh`, `scripts/ci/run-plan` and `scripts/shell-coverage` each begin by asking git
which variables belong to one repository (`git rev-parse --local-env-vars`) and unsetting
them, with `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_COMMON_DIR` and `GIT_PREFIX`
named as well for a git too old to list them. Two of the listed variables stay:
`GIT_CONFIG_PARAMETERS` and `GIT_CONFIG_COUNT` carry `git -c` settings and name no
repository, git keeps them itself when it enters another repository, and the test library
tells every case's git to start no housekeeping through them.

The doctor finding is `mj_report_git_config` in `lib/doctor.sh`. It reads the repository's
own configuration, which every worktree shares, and fails on three states:

- `user.email` or `user.name` set there to an identity a fixture gives itself (an address
  in a domain reserved for examples and tests, such as `example.com` or anything under
  `.invalid`, or `t@t`, `t@e`, or the name `t`) while a commit in the repository was
  written by anybody else. A scratch repository whose every commit is a fixture's carries
  that identity on purpose and reads OK.
- `core.bare=true` while the git directory is the `.git` of a checkout with an index. From
  that checkout nothing resolves and every other command stops at "not inside a git
  repository", so doctor asks this before it resolves the repository and names it instead.
- paths marked `merge=derived` in `.gitattributes` with no driver command behind the name.
  Where the policy declares the driver as an enforcement entry, the wiring check decides
  that fact and this one does not name it a second time.

Doctor reads. It prints the command and leaves the repair to the person.

## How to see it

```bash
bash test/run.sh 1030_a_fixture_cannot_write_the_repository_it_runs_in
bash test/run.sh 1031_doctor_names_a_poisoned_git_config
majordomus doctor
```

The first case builds a repository with a linked worktree and a real `pre-push` hook that
starts a copy of the runner, pushes from the worktree, and compares the pushed repository
before and after as bytes: every ref, both `HEAD`s, the worktree list, the configuration and
the files of both checkouts. It repeats that with all five variables exported by hand, for
a gate of the dispatcher and for a case under tracing. Then it cuts the guard out of a copy
of each script and runs the same push: the fixture renames the pushed branch to `master`,
marks the repository bare and writes its identity and a tag into it, which is what the
guard prevents and what the comparison would catch.

The second case plants each of the three states in a repository with somebody else's
history, requires doctor to fail and name it, runs the command the finding prints, and
requires the finding to be gone. It also requires a fixture, an owner's own identity and a
repository that is bare on purpose to read OK, and the configuration to be byte-identical
after every reading. In a healthy repository the line begins:

```text
OK   git-config  local — no fixture identity over another author's history, core.bare agrees with the checkout
```

## What it does not cover

The guard is in the three scripts that start cases and gates. A case started by hand
outside the runner is refused by the test library for another reason (it must stand in a
fixture the runner made), and a program that builds repositories and is started from a
hook by some other road is not covered by this claim. The crate's own tests run under
`cargo test`, which this guard does not wrap.

The two variables that carry `git -c` settings are kept on purpose, so a setting passed
that way still reaches a fixture's git.

Doctor names three states and no others. It does not notice a branch a fixture renamed, a
ref it moved, or a branch or tag it left behind: those are in the refs, and the finding
reads the configuration. A fixture identity is named only when another author's commit
exists, so a repository with no commits, or one whose whole history is a fixture's, is
never named. The reverse also holds: a scratch clone of a real repository that a test gives
a fixture identity is named, because the finding cannot tell that clone from the original.
Doctor does not repair anything it names.

## Why it exists

On 2026-10-10 a `pre-push` hook on an unmerged branch ran the suite with git's hook
environment still exported. The fixtures renamed the branch being pushed to `master`, moved
the local `master` and `gh-pages` onto fixture commits, left eight branches and a tag, and
wrote `user.name=t`, `user.email=t@example.com` and `core.bare=true` into the configuration
every worktree of the repository shares; the derived merge driver lost its command. Every
session working in that repository then committed under a fixture's name, and nothing said
so for an hour. The rule that tests run in disposable repositories was written down and
depended on every caller starting the suite from a clean environment. It now depends on
the runner.
{% endraw %}

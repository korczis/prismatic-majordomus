# Personal mode: Majordomus as one person's instrument

Majordomus is built to be committed: the `.ai/repo/` half of the layer travels with the
repository so that every clone and every worker reads the same rules. Sometimes that is not
yours to decide. You work in a repository a team owns, you want your own tasks to have a
scope and a finish contract, you want a briefing at the start of a session and the UI over
your own records, and the team's tree is not yours to add a commit, a line of `.gitignore`
or a new file to.

Personal mode is the name for that arrangement: the layer lives beside the code, in your
clone only, and the repository stays byte for byte what the team committed. Organisations
have had a word for the hardware version of this for years: bring your own device. This is
the same bargain one level up, bring your own discipline. The habits and the tool are
yours, the repository and its rules are the organisation's, and neither changes the other.

There is a switch for it: `majordomus init --personal`, and `--personal` on the installer
(ADR 0145, [`claims/personal-layer-changes-nothing-shared.md`](claims/personal-layer-changes-nothing-shared.md)).
[The switch](#the-switch) says what it does. The rest of this page is the measurement the
switch was built from, taken by hand on 2026-10-12 in a scratch repository (one `git init`,
a README, one source file, one commit) against the released build of that day, which had
no switch. It stays because it shows what each piece is and why it is ordinary practice.
Every step says whether it was run. Two were not, and they are named. Where the switch
changes a statement below, the statement says so.

## The switch

<!-- majordomus:unrun creates a layer in the repository it is run in and rewrites that repository's .git/info/exclude; test/cases/2979_a_personal_layer_changes_nothing_shared.sh runs it in fixtures -->
```sh
majordomus init --personal
majordomus update
majordomus doctor
```

`init --personal` creates the layer a plain `init` creates, covers it in a marked block of
git's per-clone exclude file, and exits 10 unless `git status` reads the same after it as
before. What it does that the hand-made arrangement below did not:

| By hand, below | With the switch |
|---|---|
| `init` writes two lines to `.gitignore`, restored with `git checkout .gitignore` | `.gitignore` is not written |
| the exclude lines are typed, and nothing checks them | one marked block, verified by `init`, by the installer and by every `doctor` |
| the policy is narrowed so that `update` writes one private file | `init --personal` fits the seeded projections to the repository: a target the project already tracks moves to the file its provider reads for one person (`personal_target` in [`share/providers.yaml`](../share/providers.yaml), `CLAUDE.local.md` for Claude Code), and a provider with no such file is left out. `update` covers each target before writing it, and still refuses, whole, a policy that names a tracked target |
| `capture install` was not run, because it writes under `.claude/` | it covers what it writes the same way |
| the session record's commit and push are turned off in the policy | under a personal layer neither happens, whatever the policy says |
| the server needs `--discovery filesystem`, started before the client | discovery walks the work tree by itself when the layer is tracked nowhere |
| `doctor` keeps the failure about the README | the project's README is not asked to name `AGENTS.md` |
| converting means deleting the exclude lines and `init --extend` | `majordomus init --extend --shared` |

The hook is still yours to add, and what the organisation's policy says about tools is
still the organisation's to say. Where to add it is git's answer, and `init` prints it:
under `.git/hooks/` the two lines go straight in. Where `core.hooksPath` points at a
directory the project tracks, the hook git calls is the project's; a dispatcher that runs
`<hook>.d/` leaves room for a file of your own, `<hook>.d/majordomus-personal`, which
`init --personal` covers before it exists, and a tracked hook with no such directory has
no place of your own, which `init` says instead of advising an edit to a shared file.

## What it is for

The arrangement is non-invasive in a precise sense: the repository's diff is empty, nobody
reviews anything, no teammate's checkout behaves differently, CI runs what it ran before,
and removing it is deleting a directory. That is what makes it usable in the situations
below, where the ordinary adoption is not available or not yet earned.

| Situation | What you get from it |
|---|---|
| **Trying it before proposing it.** You think the team would benefit and have nothing to show. | Two weeks of your own tasks with a scope, a finish contract and handovers, on the real repository. You propose it with records instead of an opinion. |
| **A contractor or consultant in a client's repository.** The conventions are the client's and not yours to extend. | Your own discipline travels with you: every engagement starts with a briefing and ends with a handover, and the client's tree is as you found it. |
| **An audit, a review or due diligence.** The brief is to read, and changing the repository is out of bounds. | Findings, decisions and open questions as typed records, each tied to the commit it was made at, so a finding says by itself when the code moved under it. |
| **Contributing to an open-source project.** Upstream does not want your tool's files, rightly. | The scope keeps a contribution to the paths it was about, and the finish contract asks for the verification command before you open the pull request. |
| **Learning a codebase you did not write.** | What you work out is written down once, as knowledge candidates and decisions, and is in the next session's briefing instead of being worked out again. |
| **A repository where adding a file is a change request.** Regulated code, a frozen release branch, a vendor drop. | The layer is beside the code and not in it, so nothing needs approving. |
| **Working with an AI agent on your own.** | The agent reads your rules and your context from `CLAUDE.local.md`, and the task scope refuses a change outside the paths you named. The team's instruction files are untouched. |
| **The same habits in every repository.** | One lifecycle everywhere, whatever each repository's own conventions are. |

What it is not for: anything that needs the layer to be shared. Rules a team is held to, a
board where colleagues see each other, a plan several people work from and evidence CI
records all need the committed adoption in [`ADOPTION.md`](ADOPTION.md). This arrangement
is one person's instrument.

## Three ways to run it

| | Personal mode | Committed adoption | Committed, with the mesh |
|---|---|---|---|
| where the layer lives | this clone, excluded from git | `.ai/repo/` in the repository | the same, with runtimes linked across machines |
| who reads the rules | you and your AI client | every clone and every worker | every machine |
| who has to agree | nobody, because nothing changes for anybody | the team, by reviewing a pull request | the team, and whoever owns the machines |
| what refuses a change | your own `pre-push` hook | the hooks and the CI gates | the same |
| the peer board | your own sessions | every worker of the repository | every machine's workers |
| backup | yours to make | git | git |
| leaving | deleting a directory | a pull request | a pull request |

The columns are a path as much as a choice. Personal mode is where one person finds out
whether the lifecycle is worth having; [From personal to shared](#from-personal-to-shared)
is the measured way from the first column to the second, and [`MESH.md`](MESH.md) is the
third.

## Why this is ordinary practice

It uses each mechanism for what it was made for.

- **`.git/info/exclude`** is git's own answer to "patterns specific to one person's
  workflow that should not be shared". It is the documented alternative to `.gitignore`
  for exactly this.
- **`CLAUDE.local.md`** is the file an AI client reads for one person's instructions beside
  the project's.
- **Hooks under `.git/hooks/`** have never been shared by git; every clone has its own.
- An editor's personal settings, a `.envrc.local`, a scratch directory and a private notes
  file are the same category of thing, and every developer already has some of them.

What stays exactly as it was is what matters to everybody else. Your commits are ordinary
commits and are reviewed like any others. CI judges them with the gates the team chose. No
check of the team's is bypassed, weakened or answered on its behalf, and nothing leaves
the machine: Majordomus calls no model and sends nothing anywhere.

Three things the arrangement does not change, and that you still owe:

- **The organisation's tool policy.** Where it decides which tools may read a repository,
  that covers a tool that lives only in your clone. Private is not the same as permitted.
- **The AI client's data policy.** The layer is local; the client that reads it may not be.
  If AI tools are not allowed on this code, this does not make them allowed.
- **Honesty about your work.** What stays local is a tool, never a change. If a colleague
  asks how you keep your tasks straight, the answer is one sentence, and when it has proved
  itself the next step is to propose the committed adoption.

## Leaving

Removing it is as small as adding it, and leaves the repository bit for bit as it was.

```sh
rm -rf .ai CLAUDE.local.md .git/hooks/pre-push
git status --short --ignored
```

Then delete the lines you added to `.git/info/exclude`. The other way out, sharing the
layer, is [From personal to shared](#from-personal-to-shared).

## Where everything ends up

| Place | What is there | Who sees it |
|---|---|---|
| committed files | nothing new; `git status` stays clean | the team |
| the work tree, excluded | `.ai/repo/`, `.ai/local/`, `CLAUDE.local.md` | this disk only |
| `.git/` | `info/exclude`, `hooks/pre-push` | this clone only; git never pushes it |

`.git/info/exclude` has the syntax of `.gitignore` and none of its visibility: it is not a
file of the work tree, so it is never committed. It lives in git's common directory, which
means it holds for every linked worktree of the clone as well.

## What Majordomus writes, and how each piece is kept out

| What appears | Written by | Kept out by | Measured |
|---|---|---|---|
| the `.ai/` directory | `init` | a line `.ai/` in `.git/info/exclude` | yes |
| two lines appended to `.gitignore` | `init` | `git checkout .gitignore`; excluding `.ai/` covers both | yes |
| `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` | `update` | one projection left in the private policy, into `CLAUDE.local.md`, excluded | yes |
| git hooks | you, by hand | `.git/hooks/` is local by nature; `pre-push` is enough | yes |
| the Claude Code settings file and `.claude/hooks/` | `capture install` | exclude both; where the team tracks that settings file, do not run `capture install` | no |
| `.mcp.json` | you, by hand | register the MCP server for your user rather than for the project | no |

## The steps, by hand

This is the arrangement as it was assembled before the switch existed. It still works, and
it is the one to read when the switch refuses something and you want to know why.

<!-- majordomus:unrun these lines rewrite a repository's .git/info/exclude and its .gitignore; they were run by hand in a scratch repository on 2026-10-12, and a documentation check has no repository of somebody else's to run them in -->
```sh
majordomus init
git checkout .gitignore
printf '.ai/\nCLAUDE.local.md\n' >> .git/info/exclude
git status --short
```

`init` appends `.ai/local/` and `.ai/repo/**/.tmp.*` to `.gitignore`. Restoring the file
loses nothing, because the exclude line covers the whole directory. The last command prints
nothing.

The policy at `.ai/repo/policy.yaml` is yours alone now. Leave one enforcement and one
projection in it:

```yaml
enforcement:
  - name: finish-on-push
    path: majordomus
    args: [finish, --check]
    wired_by: git-hook:pre-push

projections:
  - provider: claude-code
    target: CLAUDE.local.md
    always_loaded: true

session:
  commit_record_on_end: false
  push_record_on_end: false
```

The two `session` keys are a precaution. The seeded policy
([`share/skeleton/policy.yaml`](../share/skeleton/policy.yaml)) commits and pushes the
session record when an episode closes. What those do with an excluded layer was not
measured, so they are off here. For the same reason `continuity publish` and `continuity
sync` are not for this arrangement: they push a ref to the shared remote, where everybody
can read it.

`doctor` refuses a policy with no enforcement entry and one in which no projection is
marked `always_loaded`, so one of each is the floor. `CLAUDE.local.md` is the file Claude
Code reads beside the team's `CLAUDE.md`, which stays as it is.

<!-- majordomus:unrun update writes CLAUDE.local.md and the second line writes a git hook into the repository being adopted; both were run by hand in the scratch repository of 2026-10-12 -->
```sh
majordomus update
printf '#!/bin/sh\nmajordomus finish --check || exit $?\n' > .git/hooks/pre-push
chmod +x .git/hooks/pre-push
```

Do not wire `doctor` into `pre-commit` here. In this arrangement it keeps one failure that
cannot be removed (below), and a hook that always fails refuses every commit.

<!-- majordomus:unrun starts a server that keeps running and binds a port; run by hand in the scratch repository of 2026-10-12, where /ui, /cockpit and /api/v1/rules answered 200 -->
```sh
majordomus serve --discovery filesystem
```

This is the step that matters. Discovery defaults to `vcs`: the executable asks git's index
which files exist, and an excluded layer is in no index. Measured, the default reads:

```console
index built objects=1 diagnostics=4 state=Degraded
required class 'policy' (:(glob).ai/repo/policy.yaml) discovered nothing
required class 'rule' (:(glob).ai/repo/rules/**/*.md) discovered nothing
```

With `--discovery filesystem` the same tree reads `state ok`, with its policy, profiles,
prompts and the vendored rules as objects, and the server answers at `/ui` and `/cockpit`.
Start it before the AI client: a client attaches to the server that is already running and
takes its options, and one that starts its own takes the default.

From there the task lifecycle is the ordinary one. In the scratch repository `check` counted
the one edited source file as inside the scope, `finish --check` in the hook passed, and
`git status` showed that file and nothing of the layer. `finish` also refused
`--verify-command "true"`: a command that does nothing verifies nothing.

## How it fits the rest of the tool

| Part of the tool | In this arrangement | Why |
|---|---|---|
| task lifecycle: `start`, `check`, `finish` | works, measured | `check` counted the edited file as inside the scope and `finish --check` passed |
| the task scope | works, measured | `git status` showed the source change and nothing of the layer |
| briefing and context | works, measured | `context` printed git, the task and the profile |
| checkpoints, handovers, decisions | works | they are local records in the committed adoption as well |
| instruction projection | works, measured | `update` wrote `CLAUDE.local.md` and no other file |
| the `pre-push` hook | works, measured | it lives under this clone's `.git/hooks/` |
| the server, `/ui` and the Cockpit | works with `--discovery filesystem`, measured | the default discovery asks git's index |
| the plan | works locally | `/cockpit/plan` answered |
| `doctor` | one failure stays | the README does not name `AGENTS.md` |
| MCP tools for an AI client | start the server first | reading the layer was measured; an attached client was not |
| the peer board | your own sessions only | nobody else attaches to your server |
| knowledge records | local | candidates are derived for one reader; the derivation at an episode's close was not measured |
| the evidence ledger | local | it supports no guarantee anybody else reads |
| Claude Code session hooks | not measured | `capture install` writes under `.claude/` in the work tree |
| the session record's commit and push | turn off | the two policy keys above |
| `continuity publish` and `sync` | do not use | they push a ref to the shared remote |
| the mesh | do not use | it shares session and claim metadata with other machines |
| CI gates | nothing to plan | a private layer declares no CI model; the team's CI is untouched |

## What each statement rests on

| Statement | Where it is decided |
|---|---|
| `init` writes `.ai/` and two lines of `.gitignore`, and installs no hook | [`lib/init.sh`](../lib/init.sh), [`test/cases/01_init.sh`](../test/cases/01_init.sh), [`claims/local-state-ignored.md`](claims/local-state-ignored.md), [`claims/ai-layer-manifest.md`](claims/ai-layer-manifest.md) |
| instruction files are projections of the policy, and a projection's target is one line | [`lib/update.sh`](../lib/update.sh), [`claims/projection-generation.md`](claims/projection-generation.md), [`claims/region-projection.md`](claims/region-projection.md) |
| a touched file outside the scope is a failure, and `finish` evaluates its contract line by line | [`claims/scope-enforcement.md`](claims/scope-enforcement.md), [`claims/finish-contract.md`](claims/finish-contract.md) |
| a verification command that does nothing is refused | [`verification-integrity`](../.ai/repo/rules/vendor/majordomus/rules/verification-integrity.v1.md) |
| nothing performs a network call | [`claims/no-network.md`](claims/no-network.md), [`no-network-no-eval`](../.ai/repo/rules/project/no-network-no-eval.v1.md) |
| a personal layer changes nothing shared, and the command that says so checks it | [`lib/common.sh`](../lib/common.sh), [`test/cases/2979_a_personal_layer_changes_nothing_shared.sh`](../test/cases/2979_a_personal_layer_changes_nothing_shared.sh), [`claims/personal-layer-changes-nothing-shared.md`](claims/personal-layer-changes-nothing-shared.md) |
| discovery can walk the work tree instead of asking the index | [`apps/majordomus-cli/src/cli.rs`](../apps/majordomus-cli/src/cli.rs), [`MCP.md`](MCP.md), [`the-server-sees-the-current-tree`](../.ai/repo/rules/project/the-server-sees-the-current-tree.v1.md) |
| what a worker never reads is declared once | [`claims/scope-declared.md`](claims/scope-declared.md), [`SCOPE.md`](SCOPE.md) |

## What does not fit

- **By hand, `doctor` keeps one failure.** Of the six a fresh adoption reports, five go
  with the policy above and the hook. The one left is `bootstrap README.md — does not name
  AGENTS.md`, and removing it means editing the README. With the switch that check is not
  asked of a personal layer.
- **The released build has no `ui` command.** The `/ui` surface is served; open it in a
  browser.
- **Nothing is shared, so nothing is backed up.** ADRs, knowledge records, session records
  and the plan exist on one disk. `git clean -fdx` deletes them, because `-x` removes
  excluded files, and `git stash -a` hides them. Copy `.ai/` somewhere else.
- **The view of the team is git's.** The board shows your own sessions. What you know of
  anybody else is what their branches and commits say. Whether `/ui` reads another
  repository's pull requests was not measured.
- **Two rows of the table above were not run.** The session hooks of Claude Code and an MCP
  registration outside the repository are described from how they are built, not from a run.

## From personal to shared

Personal mode has done its job when you can show what it gave you. The proposal to a team
is then a pull request and a set of records, not an opinion: the tasks you ran with a
scope, the handovers a session started from, the times `finish` refused. The conversion was
measured on 2026-10-12 in a scratch repository whose layer still carried the seeded policy.

With the switch this is one command, `majordomus init --extend --shared`: it removes the
cover, adds the two `.gitignore` lines, prints what is now the repository's to commit, and
commits nothing. By hand it was this.

Delete the two lines you added to `.git/info/exclude`. `git status --short` then shows the
layer as one untracked directory. After that:

<!-- majordomus:unrun these lines write .gitignore and the instruction files of the repository being converted; they were run by hand in a scratch repository on 2026-10-12 -->
```sh
majordomus init --extend
majordomus update
majordomus doctor
```

What each did there:

| Step | Measured result |
|---|---|
| removing the exclude lines | `git status --short` shows `?? .ai/` and nothing else |
| `init --extend` | adds `.ai/local/` and `.ai/repo/**/.tmp.*` back to `.gitignore`, and nothing else |
| `git add -A -n` | names 96 paths under `.ai/` and `.gitignore`; none of them is under `.ai/local/` |
| `update` | with the seeded policy, creates `AGENTS.md`, `CLAUDE.md` and `GEMINI.md` |
| `doctor` | two failures, both wiring: the `pre-commit` and `pre-push` hooks it names are not installed |

A policy you narrowed for personal mode does not widen itself. Before `update`, return its
`projections` and `enforcement` blocks to what the team wants, starting from
[`share/skeleton/policy.yaml`](../share/skeleton/policy.yaml); that edit was not part of
the measurement. Your own records under `.ai/local/` stay local in every arrangement. From
here the path is [`ADOPTION.md`](ADOPTION.md).

## How to say it

Nothing about personal mode needs a careful answer. What each asker needs is short and is
decided somewhere that can be read.

| Who asks | What is true | Where it is decided |
|---|---|---|
| a colleague | my tasks, their scope and my handovers are in a local directory git ignores; nothing of it is in the repository | `git status --short --ignored` in my clone |
| a team lead | the repository, its review and its CI are exactly what they were; if it proves useful I will propose it as a pull request | [From personal to shared](#from-personal-to-shared) |
| security | the tool performs no network call and calls no model; the AI client I use is the one the organisation already allows, under the same data policy | [`claims/no-network.md`](claims/no-network.md), [`no-network-no-eval`](../.ai/repo/rules/project/no-network-no-eval.v1.md) |
| an architect | the same lifecycle the committed adoption runs, with a private policy; no rule of the team's is restated or overridden | [`ADOPTION.md`](ADOPTION.md) |
| whoever owns tooling policy | it is a local tool in the category of an editor configuration, and it stays subject to the policy on which tools may read this code | the organisation's policy, which this does not change |

## What stays visible

Personal is not hidden. These are the places where the arrangement shows, and each is worth
knowing before a screen is shared.

- `git add -f` adds an excluded path. Read `git status` before a commit.
- Commit trailers, the shape of commit messages and branch names are visible to everybody.
- The banner drawn on entering the directory is visible on a shared screen;
  `MAJORDOMUS_BANNER=off` turns it off.
- `git check-ignore -v .ai/manifest.yaml` names the line of `.git/info/exclude` that excludes
  the layer, and `git status --ignored` lists everything excluded.

Private is not the same as permitted. Where an organisation decides which tools may read a
repository, that decision covers a tool that lives only in your clone. Majordomus sends nothing
anywhere and calls no model; the AI client using it does.

## Related

- [`ADOPTION.md`](ADOPTION.md) for the ordinary, committed adoption.
- [`INSTALL.md`](INSTALL.md) for the installer.
- [`MCP.md`](MCP.md) for the shared server, its discovery modes and what a client inherits.
- [`SCOPE.md`](SCOPE.md) for what the executable reads of a repository at all.

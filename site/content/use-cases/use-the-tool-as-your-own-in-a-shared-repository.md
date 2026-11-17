+++
title = "Use the tool as your own in a shared repository, and propose it to the team when it has made its case"
description = "One flag installs the same layer as yours alone: no tracked file changes, nothing is added to .gitignore, git status reads the same — the command checks it — and one more command hands the layer to the team."
weight = 6
[extra]
id = "use-the-tool-as-your-own-in-a-shared-repository"
source = ".ai/repo/use-cases/use-the-tool-as-your-own-in-a-shared-repository.md"
category = "adoption"
maturity = "guaranteed"
+++

## Situation

You work in a repository other people share: your team's, a client's, an open-source
project's. You want your own agent sessions supervised in it today, on real work. A plain
`init` would add a directory, three files at the root and a line in the README — a change
to what everyone shares, which is a proposal, and you have nothing to propose yet.

A team adopts a tool on evidence. Personal mode is how one person gathers it without
asking anyone to change anything first.

## Scenario

```yaml
setup: bare
given:
  - 'a shared repository with no AI layer'
steps:
  - id: init
    run: ['init', '--personal']
    note: 'the same layer a plain init makes; the difference is whose it is'
    expect:
      exit: 0
      stdout_contains: ['^OK   personal', 'personal: this layer is yours; nothing the repository shares was changed']
      files_exist: ['.ai/repo/policy.yaml', '.ai/manifest.yaml']
  - id: update
    run: ['update']
    note: 'the provider files at the root are covered before they are written'
    expect:
      exit: 0
      files_exist: ['AGENTS.md', 'CLAUDE.md']
  - id: doctor
    run: ['doctor']
    note: 'doctor holds the layer to its claim on every run; the two hook lines are still yours to add, which is the one thing its exit code is about'
    expect:
      exit: 10
      stdout_contains: ['^OK   personal', 'not asked to name AGENTS.md']
  - id: share
    run: ['init', '--extend', '--shared']
    note: 'the trial made its case: the cover is removed and the layer is the repository''s to commit — by you, when you choose to'
    expect:
      exit: 0
      stdout_contains: ['shared: the cover is removed', 'git add .ai/']
then:
  - 'until the last step, git status read the same as before the first one'
  - 'the tracked .gitignore and README.md were untouched until the layer was shared'
  - 'nothing was committed by the tool at any step'
```

## What you run

```bash
curl -fsSL https://majordomus.dev/install.sh | sh -s -- --personal
```

or, with the tool already installed:

```bash
majordomus init --personal
```

`init --personal` reads `git status` before it writes anything, covers `.ai/` in the
repository's own exclude file, creates the layer, and exits 10 unless the status reads the
same afterwards. The installer then compares the status itself, having written none of it.
`majordomus update` and `majordomus capture install` cover each file before they write it,
and leave a file the project already tracks alone — a `CLAUDE.md` the project commits is
the project's.

When you want the team to have it:

```bash
majordomus init --extend --shared
```

It removes the cover, adds the two `.gitignore` lines a plain init adds, and prints what is
now the repository's to commit. It commits nothing.

To stop instead: delete the block between `# >>> majordomus personal >>>` and
`# <<< majordomus personal <<<` in `.git/info/exclude` and remove the files it lists.

## Outcome

Your sessions are supervised — scoped tasks, a finish contract, handovers — in a repository
whose shared state is exactly what it was. The layer is yours and this machine's: it does
not travel with a clone, a push or a stash.

What you learn is what the team would get, because the layer is the ordinary one; and the
step from "mine" to "ours" is one command and a commit you make yourself.

Which files are covered is not a list anyone maintains. The layer is one pattern, and
every other entry is the path a writer was about to write, taken from the policy's
projection targets and the provider adapters at the moment of writing.

Personal mode is not access control and not a way around an organisation's rules about
tools; whether a tool may be used on a codebase is the organisation's decision.

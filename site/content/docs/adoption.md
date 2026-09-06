+++
title = "Adoption"
description = "day one, week one, several workers, removal"
weight = 21
[extra]
source = "docs/ADOPTION.md"
+++

{% raw %}

Majordomus sits around an existing workflow. Nothing is rewritten.

## Day one: one repository, one person

```bash
git clone <this repository> ~/majordomus
export PATH="$HOME/majordomus/bin:$PATH"     # or symlink bin/majordomus somewhere on PATH

cd <your project>
majordomus init          # .ai/ with the policy, profiles, rules, prompts and workflows
majordomus update        # CLAUDE.md, AGENTS.md, GEMINI.md generated from the policy
majordomus doctor        # tells you exactly which hook lines are missing
```

Add the two hook lines `init` printed. Run `doctor` again; it should report zero
failures. Commit `.ai/repo/` and the generated files. `.ai/local/` is this checkout's own
state — the task record, ledger, checkpoints, handovers — and `init` ignores it; it never
travels through git, and a fresh clone starts without it.

From then on:

```bash
majordomus start "<task>" --scope <paths> [--profile <name>]
# ... the AI worker reads the generated instructions and works ...
majordomus check
majordomus finish --outcome completed --verify-command "<your test command>"
```

## A repository that already has a CLAUDE.md

Most repositories worth supervising already have hand-written instructions for their AI
tools. `mode: region` keeps them. Majordomus then owns only the text between two markers
and copies everything else through byte for byte:

```yaml
# .ai/repo/policy.yaml
projections:
  - provider: claude-code
    target: CLAUDE.md
    mode: region
    always_loaded: true
```

```bash
majordomus update        # appends the region once; the rest of CLAUDE.md is untouched
majordomus doctor        # hashes the region, and reports the host document's length
```

What that buys you: an edit outside the markers is the repository's own business and is
never reported as drift, while an edit inside them is caught exactly as a whole-file
projection would be. The budget, the link check and the count check all measure the
region, so every failure `doctor` reports can be fixed by editing the policy.

Malformed markers — unclosed, out of order, or repeated — are refused rather than
guessed at, and nothing is written.

## Hooks that are dispatchers

If `pre-commit` runs every executable in `pre-commit.d/`, put the invocation in a subhook
and leave the dispatcher alone. `doctor` looks in `<hook>.d/` as well and names the file
that carries it. A subhook that is present but not executable is reported as not wired,
because that is what the dispatcher does with it.

## Week one: adjust the policy, not the projections

Every rule the workers see comes from `.ai/repo/policy.yaml`, the profiles and the rules
under `.ai/repo/rules/`. Edit those, run `update`. If someone edits `CLAUDE.md` directly, `doctor` and `watch` say so,
and `update` refuses to overwrite until you look at the diff.

Typical first edits:

- lower `always_loaded_budget_lines` if the generated file is longer than you want
  workers to read every session
- change `profiles.default` to `routine` for a maintenance-heavy repository
- add a project-specific verification requirement by adding a profile

## Several workers at once

Give each concurrent autonomous writer its own git worktree. `start` in each worktree
reports any scope that overlaps another worktree's active task. Nothing blocks; the
report is for the person deciding who works where.

With one worker, or workers that run strictly one after another, plain branches are
correct and worktrees are overhead. The README says so; believe it.

## Existing repository with history

The pre-push hook runs `finish --check`, which passes when no task is active. Nothing
about the repository's past is judged. Start supervising from the first `start`.

## Upgrading from the pre-`.ai` layout

Repositories set up before the `.ai/` layer kept everything under `.majordomus/`. Every
command refuses that layout and names the one that moves it:

```bash
majordomus migrate --dry-run   # the whole plan, one line per file; writes nothing
majordomus migrate             # git-moves the tracked half into .ai/repo/, moves the state
                               # into .ai/local/state/ after a verified byte-for-byte backup
                               # whose path it prints, re-stamps the projections, runs doctor
```

Nothing under `.majordomus/` that the migration does not recognise is deleted; it is
reported, and the directory stays until you move it by hand. A `.majordomus/bin/majordomus`
is a tool installation, not project data, and is left alone. See `CLI.md`.

## Removing Majordomus

Delete `.ai/`, the generated projections, and the two hook lines. For a region
projection, delete the marked block from the host document; everything around it was
never touched.

## What adoption does not require

- no server, account, token, or network access
- no change to how the AI tool is launched
- no runtime dependency beyond bash 3.2, git, and a checksum tool
- no buy-in from the whole team on day one; one repository, one person is enough to see
  whether the finish contract catches anything
{% endraw %}

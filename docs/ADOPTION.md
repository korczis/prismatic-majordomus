# Adoption

Majordomus sits around an existing workflow. Nothing is rewritten.

## Day one: one repository, one person

```bash
git clone <this repository> ~/majordomus
export PATH="$HOME/majordomus/bin:$PATH"     # or symlink bin/majordomus somewhere on PATH

cd <your project>
majordomus init          # .majordomus/ with policy, profiles, templates
majordomus update        # CLAUDE.md, AGENTS.md, GEMINI.md generated from the policy
majordomus doctor        # tells you exactly which hook lines are missing
```

Add the two hook lines `init` printed. Run `doctor` again; it should report zero
failures. Commit `.majordomus/` and the generated files.

From then on:

```bash
majordomus start "<task>" --scope <paths> [--profile <name>]
# ... the AI worker reads the generated instructions and works ...
majordomus check
majordomus finish --outcome completed --verify-command "<your test command>"
```

## Week one: adjust the policy, not the projections

Every rule the workers see comes from `.majordomus/policy.yaml` and the profiles. Edit
those, run `update`. If someone edits `CLAUDE.md` directly, `doctor` and `watch` say so,
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

## Removing Majordomus

Delete `.majordomus/`, the generated projections, and the two hook lines. Nothing else
was touched.

## What adoption does not require

- no server, account, token, or network access
- no change to how the AI tool is launched
- no runtime dependency beyond bash 3.2, git, and a checksum tool
- no buy-in from the whole team on day one; one repository, one person is enough to see
  whether the finish contract catches anything

## Operating contract for this repository

This repository is Prismatic Majordomus itself, supervised by its own tool. This file is
generated from `.majordomus/policy.yaml` and `.majordomus/providers/body.md`; edit those,
then run `bin/majordomus update`.

### Read first

`docs/DESIGN.md` (what and why), `docs/CLI.md` (commands, exit codes), `docs/SCHEMAS.md`
(files), `docs/EXTRACTION_REPORT.md` (the evidence behind each decision).

### Rules for every change

- **English only** in code, comments, commits, documents.
- **Portable shell:** bash 3.2 and BSD userland are the floor. No associative arrays,
  no `mapfile`, no GNU-only flags. `shellcheck -x -s bash` must pass.
- **No counts in prose.** Write the command that computes a number, never the number.
- **No claim without a test.** A capability sentence in `README.md` or `docs/` is
  backed by a case in `test/cases/` or is phrased as a target.
- **Every finding carries a reproduce command.**
- **Blocking checks are deterministic and cheap.** Work in progress is reported, not blocked.
- **No new nouns:** no agents, personas, roles, tiers, registries.
- **Unknown configuration keys are errors.** Every state field is both written and read.
- **Never store or summarise transcripts.**
- **No network, no telemetry, no `eval`, no `curl | sh`, no silent overwrite, no
  recursive deletion** in `bin/`, `lib/`, `share/`, or `test/`.
- **Clean-room:** the Prismatic name is the only thing carried from anywhere else. No
  internal paths, hostnames, component names, doctrine vocabulary, or quotations.
- **Derived files are regenerated, never edited:** this file's projections, and
  everything `docs/GITHUB_PAGES_ARCHITECTURE.md` lists as generated. Change the
  canonical source, rerun the generator, commit both.

### Never author identity fields

`repository_id`, `branch`, `head`, `working_tree`, `changed_files` on any state record
are computed from git. Do not write or guess them.

### Lifecycle

```
bin/majordomus start "<task>" --scope <paths> [--profile <name>]
bin/majordomus check                 # before claiming anything is done
bin/majordomus handover < note.md    # to continue elsewhere, or
bin/majordomus finish --outcome <completed|partial|blocked|no_match|failed> --verify-command "bash test/run.sh"
```

Checkpoint with `bin/majordomus check --checkpoint` at least every {{CHECKPOINT_DEFAULT}}.
Default profile: `{{DEFAULT_PROFILE}}`.

### Profiles

{{PROFILE_TABLE}}

### Finish contract for `completed`

{{FINISH_CONTRACT}}

`no_match` means the thing sought does not exist; `failed` means the work could not be
done. Different facts. Other outcomes need a note with `# Next Action` or `# Reason`.

### Handover

Body on stdin with non-empty {{REQUIRED_SECTIONS}}. Front matter is computed.

### Working here

Conventional commits, `type(scope): description`, with the co-author footer the
session provides. Commit and push incrementally. Tests run in disposable repositories
via `bash test/run.sh`, never against this checkout.

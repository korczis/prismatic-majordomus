# AGENTS.md — operating contract for this repository

This file is read by AI coding workers. It is intentionally short. `CLAUDE.md` points
here so that every worker reads the same rules. When `majordomus update` exists, both
files will be generated from `.majordomus/policy.yaml`; until then this one is
hand-maintained and `CLAUDE.md` is a pointer.

## What this repository is

Prismatic Majordomus: a lightweight supervisory control layer for AI-assisted work.
Design phase. Read `docs/DESIGN.md` before changing anything. Implement against
`docs/CLI.md` (commands, exit codes) and `docs/SCHEMAS.md` (files). Read
`docs/EXTRACTION_REPORT.md` to see why each decision was made before proposing to
change one.

## Rules that apply to every change

- **English only** in code, comments, commits, and documents.
- **Portable shell.** Target bash 3.2 and BSD userland as well as GNU. No associative
  arrays, no `mapfile`, no GNU-only flags. Run `shellcheck` if available.
- **No counts in prose.** Write the command that computes a number, never the number.
- **No claim without an executable behind it.** A README sentence that describes a
  capability must be backed by a test in `test/cases/` or it is removed or reworded.
- **Every finding the tool emits carries a reproduce command.**
- **Blocking checks are deterministic and cheap.** Anything about work in progress is
  reported, never blocked.
- **No new nouns.** No agents, personas, roles, tiers, or registries. If a change adds
  a named entity, it needs a justification in the pull request that the design's
  "Intentionally Absent" list does not already reject.
- **Unknown configuration keys are errors**, in the tool and in this repository's own
  files.
- **Every state field is written and read.** A field nothing reads is removed.
- **Never store or summarise transcripts.** Handovers carry durable facts.
- **No network calls, no telemetry, no `eval`, no `curl | sh`, no silent overwrite,
  no recursive deletion** anywhere in `bin/`, `lib/`, or `test/`.

## Clean extraction boundary

This project may carry the Prismatic name. It carries nothing else from any other
codebase. Do not introduce internal paths, hostnames, component names, doctrine
vocabulary, or quotations from private material. Magnitudes are fine; attributions
are not. If in doubt, leave it out.

## Working here

- Commits: conventional format, `type(scope): description`, and the co-author footer
  the session provides.
- Commit and push incrementally. Small commits that each leave the tree consistent.
- Tests run in disposable temporary repositories, never against this checkout.
- Before claiming a phase is complete, compare every capability sentence in
  `README.md` against `test/cases/`.

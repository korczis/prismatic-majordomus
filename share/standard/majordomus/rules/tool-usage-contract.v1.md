---
id: majordomus.tool-usage-contract
version: 1
kind: rule
title: A tool's command line is stated once and kept
description: "Every executable a repository ships states its command line in one usage text that its parser, its --help and every document that shows it agree with."
statement: "A repository's tools print their whole usage on --help, exit 0 and write nothing; refuse an argument they do not read with exit 2 before doing any work; and are shown in documents only with flags their parser accepts."
status: active
class: advisory
depends_on: [majordomus.verify-outcomes@1]
tags: [principle, tooling, documentation]
---

# Rationale

A tool's flags live in three places: its parser, its usage text and the documents that show it being run. Nothing keeps them in step, so each drifts on its own, and the person who finds out is the one who followed the text. A 2026-11-17 audit of the OpenTTD research fork's research tooling found every kind of drift:

- flags the parser accepted and the usage never named;
- `--help` arms that printed a fixed line range, so the usage line itself was cut off, or the range ran on into the script's code;
- tools with no `--help` at all, which took the word as a URL, fetched it, or opened a browser on it;
- `--help` exiting 2, so a caller could not tell asking from failing.

Two tools rewrote `README.md` when given `--help` or a typo, because any argument they did not test for meant "write". A server took an unknown subcommand as "serve", bound its port and ran forever. A shell check that does not refuse, plus a default that does work, makes a typo the most expensive command a tool has.

None of these were found by a test: every tool did what it was written to do. They were found by putting the three places side by side, which a machine can do on every commit.

# Required behaviour

For every executable the repository tracks under its tools:

- `-h` and `--help` print the whole usage text, exit 0 and change nothing. A script whose usage is its header comment prints the comment block to its first code line (`sed -n '2,/^[^#]/s/^# \{0,1\}//p' "$0"`), never a fixed line range.
- An argument the tool does not read is refused with exit 2 and the usage, before any work. A tool that passes arguments on to another program says so in its usage.
- The usage names every flag the parser accepts, with any stated default being the one the code uses.
- A document that shows the tool run with a flag shows only flags the parser accepts.
- Exit codes keep the common meaning: 0 success, 1 a failed check, 2 usage, 3 a missing prerequisite. A prerequisite message names the command that provides it.

# Failure behaviour

No validator ships with this rule, so a violation is reported by nobody unless the repository wires a check. The OpenTTD research fork does: `tools/surfaces/surfaces check` reads every tracked script's parser (shell `case` arms and `[[ ]]` tests, Python argparse and `argv` tests) and fails with a `usage`, `help` or `flag` finding that names the file and line (korczis/OpenTTD, `tools/surfaces/README.md`, section "Tool flags").

# Verification

Run each tool with `--help` and with `--zz-not-a-flag` in a scratch copy of the repository. The first exits 0 and the second exits 2, and `git status --porcelain` is empty after both. A wired check is verified by a document that shows a tool with an unknown flag: the check fails on it.

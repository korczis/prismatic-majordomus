---
id: project.portable-shell
version: 1
kind: rule
title: Portable shell
description: bash 3.2 and BSD userland are the floor: no associative arrays, no mapfile, no GNU-only flags, and shellcheck -x -s bash passes on every script.
statement: bash 3.2 and BSD userland are the floor: no associative arrays, no mapfile, no GNU-only flags, and shellcheck -x -s bash passes on every script.
status: active
class: blocking
depends_on: []
tags: [shell, portability]
---

# Rationale

The tool runs on the machine the repository is on, which is a Mac as often as a Linux host; a construct that works on one and not the other is a bug that shows up only where nobody is looking.

# Required behaviour

bash 3.2 and BSD userland are the floor: no associative arrays, no mapfile, no GNU-only flags, and shellcheck -x -s bash passes on every script.

# Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

# Verification

Review. `shellcheck -x -s bash -S warning bin/majordomus lib/*.sh test/run.sh test/lib.sh test/cases/*.sh` runs in CI (.github/workflows/validate.yml) and test/cases/08_no_forbidden_constructs.sh scans for the constructs SECURITY.md forbids.

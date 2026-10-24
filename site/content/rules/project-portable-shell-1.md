+++
title = "Portable shell"
description = "Portable shell"
weight = 143
[extra]
kind = "rule"
slug = "project-portable-shell-1"
identity = "project.portable-shell@1"
status = "active"
source = ".ai/repo/rules/project/portable-shell.v1.md"
+++
{% raw %}

## Rationale

The tool runs on the machine the repository is on, which is a Mac as often as a Linux host; a construct that works on one and not the other is a bug that shows up only where nobody is looking.

## Required behaviour

bash 3.2 and BSD userland are the floor: no associative arrays, no mapfile, no GNU-only flags, and shellcheck -x -s bash passes on every script.

## Failure behaviour

Case 3480 refuses an associative array, mapfile or readarray, and a GNU-only flag it lists, unless the same line carries the BSD form, in every tracked shell unit; the shell-lint gate refuses a script shellcheck flags. What neither reads, such as a GNU-only flag the case does not list, a reviewer decides, and a change that violates it is not merged.

## Verification

`bash test/run.sh 3480_the_shell_floor_is_bash_3_2_and_bsd_userland` reads every tracked shell unit for the constructs the floor forbids, after holding its scanner to a fixture whose answer is known. `scripts/ci/shell-lint`, the shell-lint gate, runs `shellcheck -x -s bash -S warning` over the tool, the scripts and the cases. test/cases/08_no_forbidden_constructs.sh scans for the constructs SECURITY.md forbids, eval and network clients among them. What none of them reads is review's.
{% endraw %}

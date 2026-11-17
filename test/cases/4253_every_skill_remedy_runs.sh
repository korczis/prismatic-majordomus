# majordomus-covers: skills
# Every remedy doctor prints for a skill is a command the shell accepts (I4253). doctor's skill
# findings say `reproduce: majordomus skills explain <id>`; the shell knew list, show and check
# and answered "unknown subcommand 'explain'", so an adopter told what to run could not run it.
#
#   skills status, explain and verify reach the executable with their arguments unchanged;
#   every `majordomus skills <sub>` named in a doctor finding is a subcommand the shell accepts;
#   an unknown subcommand is still refused, and names every one the shell accepts.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p .ai/repo/skills/demo
cat > .ai/repo/skills/demo/SKILL.md <<'SKILL'
---
schema: skill/v1
id: demo
version: 1
title: Demo
description: A skill nothing tests or invokes, so doctor names its remedies.
status: active
---

# Purpose

Exercise the remedies.

# Procedure

1. Nothing.

# Output

Nothing.
SKILL
git add . && git commit -qm base

# the shell refuses what it does not know, and says what it knows
expect_exit 2 "$MJ" skills nonesuch
expect_grep 'list[|]show[|]check[|]status[|]explain[|]verify'

RB="$(rust_bin 2>/dev/null)" || skip "the executable is not built here"
[ -x "$RB" ] || skip "the executable is not built here"
# the checkout's launcher hands the executable over through MAJORDOMUS_BIN
export MAJORDOMUS_BIN="$RB"
for sub in status verify "explain demo"; do
  # shellcheck disable=SC2086
  code=0; out="$("$MJ" skills $sub 2>&1)" || code=$?
  case "$out" in *"unknown subcommand"*) echo "    skills $sub was refused by the shell: $out"; exit 1 ;; esac
  [ "$code" -ne 2 ] || { echo "    skills $sub exited 2 (usage): $out"; exit 1; }
done

# every remedy the sources print for a skill names a subcommand the shell accepts
remedies="$(grep -rhoE 'majordomus skills [a-z]+' "$ROOT/apps/majordomus-cli/src/skill" "$ROOT/lib" | sort -u)"
[ -n "$remedies" ] || { echo "    no skills remedy found in the sources"; exit 1; }
printf '%s\n' "$remedies" | while read -r _ _ sub; do
  case "$sub" in list|show|check|status|explain|verify) ;; *) echo "    a remedy names 'skills $sub', which the shell refuses"; exit 1 ;; esac
done
echo "ok: skills status, explain and verify reach the executable; every doctor remedy runs"

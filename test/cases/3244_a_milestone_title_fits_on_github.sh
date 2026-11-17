# majordomus-covers: plan
# majordomus-negative: plan
# A milestone's title fits on GitHub (I2844). scripts/github-sync projects a milestone as
# "<id> — <title>", and GitHub refuses a milestone title over 255 characters with HTTP 422:
# one record (282 characters) turned master's github-projection red (run 39392245555) and
# left the release top's structure job unclearable from a branch. `plan validate` now refuses
# such a record before it is projected, counting characters rather than bytes, in the shell
# engine (lib/project.awk) here; the executable's engine (plan.rs) holds the same rule in its
# unit test a_milestone_title_github_would_refuse_is_refused.
#
#   1  a projected title of exactly 255 characters, one of them multi-byte, passes
#   2  one character more is refused by name, with the projected length
. "$ROOT/test/lib.sh"

"$MJ" init >/dev/null
pj_init
pj_milestone M001
# "M001 — " is 7 characters; "é" is one character and two bytes
title_of_length() { printf 'é'; printf 'x%.0s' $(seq 1 $(( $1 - 7 - 1 ))); }
set_title() { sed "s/^title: .*/title: \"$(title_of_length "$1")\"/" .ai/repo/project/milestones/M001.yaml > "$T/m" \
  && cat "$T/m" > .ai/repo/project/milestones/M001.yaml; }

# ---------------------------------------------------------------- 1. exactly at the limit
set_title 255
"$MJ" plan validate > "$T/at" 2>&1 \
  || { echo "    a 255-character projected title was refused:"; sed 's/^/      /' "$T/at"; exit 1; }
grep -q 'milestone_title_too_long' "$T/at" \
  && { echo "    a 255-character projected title was reported as too long"; exit 1; }

# ---------------------------------------------------------------- 2. one character over
set_title 256
rc=0; "$MJ" plan validate > "$T/over" 2>&1 || rc=$?
[ "$rc" != 0 ] || { echo "    a 256-character projected title passed plan validate"; exit 1; }
grep -q 'milestone_title_too_long *M001 .*256-character title' "$T/over" \
  || { echo "    the refusal does not name the milestone and its projected length:"; sed 's/^/      /' "$T/over"; exit 1; }

echo "    a milestone title that fits on GitHub passes; one character over is refused by name"

# majordomus-covers: start decision handover
# majordomus-timeout: 600
# Stop on one machine, continue on another (ADR 0105). Two machines, simulated as two
# clones of one bare remote, each with its own HOME and its own XDG_STATE_HOME — so its own
# device key — and nothing shared but the remote:
#
#   A: start a task, record a decision, write a handover, publish it, sync
#   B: clone, sync, see the resumable handover, plan, resume
#      -> the intent, issue, decision, handover and next action are B's records now,
#         the provenance names A, and B's own paths are recomputed, never A's copied
#   B: continue, publish, sync          A: sync -> B's record continues A's line
#
# Plus the boundary: no stored record names either machine's disk, and a second sync with
# nothing new moves nothing.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
S="$(mktemp -d "${TMPDIR:-/tmp}/mj821.XXXXXX")"; trap 'rm -rf "$S"' EXIT
git init -q --bare -b main "$S/shared.git"

# on MACHINE CMD...: run CMD in MACHINE's clone with MACHINE's own home and state
on() { m="$1"; shift; ( cd "$S/$m/repo" && HOME="$S/$m/home" XDG_STATE_HOME="$S/$m/state" "$@" ); }
mkdir -p "$S/a/home" "$S/b/home"

# --- machine A: a repository of the layer, pushed
mkdir -p "$S/a/repo"
on a git init -q -b main .
on a git config user.email a@example.com; on a git config user.name a
on a "$MJ" init >/dev/null; on a "$MJ" update >/dev/null
mkdir -p "$S/a/repo/lib"; echo a > "$S/a/repo/lib/a"
on a "$RB" continuity device --label macbook-pro --format json > "$S/a.device.json" || exit 1
# B's key, made on B (its own state directory) before B has a clone: the trust list is
# committed once, with both devices on it
( cd "$S/a/repo" && HOME="$S/b/home" XDG_STATE_HOME="$S/b/state" "$RB" continuity device --label mac-mini --format json ) \
  > "$S/b.device.json" || exit 1
# the repository's trust list names both devices: what the mesh declaration already is
mkdir -p "$S/a/repo/.ai/repo/mesh"
{
  printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: false\n'
  printf 'trust:\n  policy: deny_unknown\n  allow:\n'
  printf '    - %s\n' "$(jq -r .public_key "$S/a.device.json")" "$(jq -r .public_key "$S/b.device.json")"
} > "$S/a/repo/.ai/repo/mesh/majordomus.yaml"
on a git add -A; on a git commit -qm base
on a git remote add origin "$S/shared.git"
on a git checkout -qb feature/x
on a git push -q -u origin feature/x 2>/dev/null

on a "$MJ" start "Ship the continuity fixture" --scope lib >/dev/null 2>&1 || { echo "    start failed"; exit 1; }
on a "$MJ" decision add "Records travel in a ref of their own" --why "no branch carries them" >/dev/null
printf '# Objective\nShip feature x\n\n# Current State\nthe parser is done\n\n# Next Action\nwrite the test for lib/a\n' \
  | on a "$MJ" handover >/dev/null 2>&1 || { echo "    handover failed"; exit 1; }
on a "$RB" continuity publish --issue '#184' --format json > "$S/a.pub.json" || { echo "    publish failed"; cat "$S/a.pub.json"; exit 1; }
REC_A="$(jq -r .record.id "$S/a.pub.json")"
[ "$(jq -r .record.device.label "$S/a.pub.json")" = macbook-pro ] || { echo "    the record does not name its device"; exit 1; }
on a "$RB" continuity sync --format json > "$S/a.sync.json" || { echo "    A's sync failed"; cat "$S/a.sync.json"; exit 1; }
[ "$(jq -r .published "$S/a.sync.json")" = 1 ] || { echo "    A published nothing"; cat "$S/a.sync.json"; exit 1; }
git --git-dir="$S/shared.git" rev-parse -q --verify refs/majordomus/continuity >/dev/null \
  || { echo "    the remote holds no continuity ref"; exit 1; }
[ "$(git --git-dir="$S/shared.git" rev-parse feature/x)" = "$(on a git rev-parse HEAD)" ] \
  || { echo "    the sync moved a branch"; exit 1; }

# --- machine B: a clone, and nothing else
mkdir -p "$S/b"; git clone -q "$S/shared.git" "$S/b/repo"
on b git config user.email b@example.com; on b git config user.name b
on b git checkout -q feature/x
on b "$RB" continuity status --format json > "$S/b.status0.json" || exit 1
[ "$(jq '.resumable | length' "$S/b.status0.json")" = 0 ] \
  || { echo "    status found a record before any sync: it must not use the network"; exit 1; }
on b "$RB" continuity sync --format json > "$S/b.sync.json" || { cat "$S/b.sync.json"; exit 1; }
[ "$(jq -r .fetched "$S/b.sync.json")" = 1 ] || { echo "    B fetched nothing"; cat "$S/b.sync.json"; exit 1; }
on b "$RB" continuity status --format json > "$S/b.status.json"
jq -e --arg r "$REC_A" '.resumable[0].id == $r and .resumable[0].device.label == "macbook-pro" and .resumable[0].trust == "trusted"' \
  "$S/b.status.json" >/dev/null || { echo "    B does not see A's handover as resumable from a trusted device"; jq .resumable "$S/b.status.json"; exit 1; }

on b "$RB" continuity plan --format json > "$S/b.plan.json"
jq -e '.status == "ready" and .source.relation == "exact" and (.blockers | length) == 0' "$S/b.plan.json" >/dev/null \
  || { echo "    the plan is not ready"; jq '{status, blockers, warnings, source}' "$S/b.plan.json"; exit 1; }
jq -e '.restores | index("handover") and index("decisions") and index("next_action") and index("lineage")' "$S/b.plan.json" >/dev/null \
  || { echo "    the plan does not say what it restores"; exit 1; }
jq -e '.recomputes | index("worktree") and index("repository_id") and index("episode")' "$S/b.plan.json" >/dev/null \
  || { echo "    the plan does not say what it recomputes"; exit 1; }

on b "$RB" continuity resume --format json > "$S/b.resume.json" || { echo "    resume failed"; jq . "$S/b.resume.json"; exit 1; }
jq -e '.resumed == true and .decisions_carried == 1' "$S/b.resume.json" >/dev/null || { echo "    not resumed"; exit 1; }
# intent, issue, next action, provenance
jq -e '.plan.task.title == "Ship the continuity fixture" and .plan.record.issue == "#184"
       and .plan.next_action == "write the test for lib/a" and .plan.record.device.label == "macbook-pro"' \
  "$S/b.resume.json" >/dev/null || { echo "    the resumed plan lost intent, issue, next action or provenance"; jq .plan "$S/b.resume.json"; exit 1; }
# the handover is B's record now, found by the shell tool's own resolution
on b "$MJ" handover --resolve > "$S/b.resolve.txt" 2>&1 || { cat "$S/b.resolve.txt"; exit 1; }
expect_grep "write the test for lib/a" "$S/b.resolve.txt"
expect_grep "the parser is done" "$S/b.resolve.txt"
expect_grep "Records travel in a ref of their own" "$S/b/repo/.ai/local/state/decisions.md"
# B's own paths are B's: the record it wrote names no path of A's disk
if grep -rqF "$S/a" "$S/b/repo/.ai/local/state"; then
  echo "    a path of machine A reached machine B"; grep -rF "$S/a" "$S/b/repo/.ai/local/state" | head -3; exit 1
fi
# resuming again rewrites nothing
on b "$RB" continuity resume --record "$REC_A" --format json > "$S/b.resume2.json"
set -- "$S"/b/repo/.ai/local/state/handovers/*--continuity--*.md
[ "$#" = 1 ] || { echo "    a second resume wrote a second handover"; exit 1; }
[ "$(grep -c 'Records travel in a ref' "$S/b/repo/.ai/local/state/decisions.md")" = 1 ] || { echo "    a decision was carried twice"; exit 1; }

# --- B continues; its record continues A's line
on b "$MJ" start "Ship the continuity fixture" --scope lib >/dev/null 2>&1
echo b >> "$S/b/repo/lib/a"; on b git commit -qam "test lib/a"
printf '# Objective\nShip feature x\n\n# Current State\nthe test is written\n\n# Next Action\nopen the pull request\n' \
  | on b "$MJ" handover >/dev/null 2>&1
on b "$RB" continuity publish --format json > "$S/b.pub.json" || { cat "$S/b.pub.json"; exit 1; }
REC_B="$(jq -r .record.id "$S/b.pub.json")"
jq -e --arg a "$REC_A" '.record.parent == $a and .record.line == $a and .record.issue == "#184"' "$S/b.pub.json" >/dev/null \
  || { echo "    B's record does not continue A's line (or lost its issue)"; jq .record "$S/b.pub.json"; exit 1; }
on b git push -q origin feature/x 2>/dev/null
on b "$RB" continuity sync --format json > /dev/null || exit 1

# --- A sees the lineage: A's record, then B's
on a "$RB" continuity sync --format json > "$S/a.sync2.json" || exit 1
jq -e --arg a "$REC_A" '.lines[] | select(.line == $a) | .relation == "remote_newer"' "$S/a.sync2.json" >/dev/null \
  || { echo "    A does not see B's record as newer"; jq .lines "$S/a.sync2.json"; exit 1; }
on a "$RB" continuity records --format json > "$S/a.records.json"
jq -e --arg a "$REC_A" --arg b "$REC_B" '(.records | length) == 2 and (.lines | length) == 1
   and (.lines[0].heads[0].id == $b) and ([.records[] | select(.id == $b)][0].parent == $a)' "$S/a.records.json" >/dev/null \
  || { echo "    the lineage A -> B is not what A's store holds"; jq . "$S/a.records.json"; exit 1; }
on a git pull -q --ff-only origin feature/x 2>/dev/null
on a "$RB" continuity plan --format json > "$S/a.plan.json"
jq -e --arg b "$REC_B" '.record.id == $b and .lineage == "newer" and .record.device.label == "mac-mini"' "$S/a.plan.json" >/dev/null \
  || { echo "    A's plan does not offer B's continuation"; jq '{status, lineage, record}' "$S/a.plan.json"; exit 1; }

# --- idempotent: nothing new, nothing moves
on a "$RB" continuity sync --format json > "$S/a.sync3.json"
jq -e '.action == "none" and .fetched == 0 and .published == 0' "$S/a.sync3.json" >/dev/null \
  || { echo "    a sync with nothing new moved something"; jq . "$S/a.sync3.json"; exit 1; }

# --- the boundary, read from the store itself: no path of either disk, no home, no pid
for id in "$REC_A" "$REC_B"; do
  git --git-dir="$S/shared.git" cat-file -p "refs/majordomus/continuity:records/$id.json" > "$S/rec.json"
  # the two home prefixes are spelled in halves: this file is committed, and case 58 refuses
  # a committed file that names one
  for leak in "$S" "/Us""ers/" "/ho""me/" '"pid"' "server.json" ".sock"; do
    if grep -qF -- "$leak" "$S/rec.json"; then echo "    record $id carries $leak"; exit 1; fi
  done
done
exit 0

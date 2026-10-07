# majordomus-covers: handover
# majordomus-timeout: 600
# Divergence is decided by lineage, never by a clock (ADR 0105). A publishes R1; B resumes it
# and publishes R2; A, which never synced, publishes R3 from R1 as well. After the syncs the
# line has two heads: the store keeps all three records, sync reports the line diverged, the
# status says so, and a plan that would pick one is a conflict (exit 10) until a person names
# the record — then it proceeds, warning what it leaves behind.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
S="$(mktemp -d "${TMPDIR:-/tmp}/mj823.XXXXXX")"; trap 'rm -rf "$S"' EXIT
git init -q --bare "$S/shared.git"

# Each machine is a clone with its own HOME and XDG_STATE_HOME, so its own device key; the
# bare repository is the only thing they share. (The same harness as case 821.)
on() { m="$1"; shift; ( cd "$S/$m/repo" && HOME="$S/$m/home" XDG_STATE_HOME="$S/$m/state" "$@" ); }
cj() { m="$1"; shift; on "$m" "$RB" continuity "$@" --format json; }
origin() {
  mkdir -p "$S/$1/repo" "$S/$1/home"
  on "$1" git init -q -b main .
  on "$1" git config user.email "$1@example.com"; on "$1" git config user.name "$1"
  on "$1" "$MJ" init >/dev/null; on "$1" "$MJ" update >/dev/null
  mkdir -p "$S/$1/repo/lib"; echo a > "$S/$1/repo/lib/a"
  on "$1" git add -A; on "$1" git commit -qm base
  on "$1" git remote add origin "$S/shared.git"
  on "$1" git push -q -u origin main 2>/dev/null
}
clone() {
  mkdir -p "$S/$1/home"; git clone -q "$S/shared.git" "$S/$1/repo"
  on "$1" git config user.email "$1@example.com"; on "$1" git config user.name "$1"
}
# handover MACHINE STATE NEXT: a handover written with the shell tool
# (under the active task when there is one; a handover needs none — ADR 0052)
handover() {
  body="$(printf '# Objective\nShip it\n\n# Current State\n%s\n\n# Next Action\n%s\n' "$2" "$3")"
  printf '%s\n' "$body" | on "$1" "$MJ" handover >/dev/null 2>&1 \
    || printf '%s\n' "$body" | on "$1" "$MJ" handover --no-task >/dev/null 2>&1 \
    || { echo "    handover on $1 failed"; exit 1; }
}


origin a
on a git checkout -qb feature/x; on a git push -q -u origin feature/x 2>/dev/null
handover a "one" "two"
cj a publish > "$S/r1.json"; R1="$(jq -r .record.id "$S/r1.json")"
cj a sync >/dev/null
clone b; on b git checkout -q feature/x
cj b sync >/dev/null; cj b resume >/dev/null || { echo "    B could not resume R1"; exit 1; }
handover b "B went on" "B next"
cj b publish > "$S/r2.json"; R2="$(jq -r .record.id "$S/r2.json")"
cj b sync >/dev/null
# A continues from R1 as well, offline from B
sleep 1
handover a "A went on" "A next"
cj a publish > "$S/r3.json"; R3="$(jq -r .record.id "$S/r3.json")"
jq -e --arg r1 "$R1" ".record.parent == \$r1" "$S/r2.json" "$S/r3.json" >/dev/null || { echo "    both must continue R1"; exit 1; }
cj a sync > "$S/a.sync.json" || exit 1
jq -e --arg r1 "$R1" ".lines[] | select(.line == \$r1) | .relation == \"diverged\" and (.heads | length) == 2" "$S/a.sync.json" >/dev/null \
  || { echo "    sync did not report the divergence"; jq .lines "$S/a.sync.json"; exit 1; }
jq -e "[.diagnostics[].code] | index(\"continuity.diverged\")" "$S/a.sync.json" >/dev/null || { echo "    no diverged diagnostic"; exit 1; }
cj a records > "$S/a.records.json"
[ "$(jq ".records | length" "$S/a.records.json")" = 3 ] || { echo "    a record was overwritten"; exit 1; }
jq -e --arg r2 "$R2" --arg r3 "$R3" "(.lines[0].heads | map(.id) | sort) == ([\$r2, \$r3] | sort)" "$S/a.records.json" >/dev/null \
  || { echo "    the two heads are not B's and A's continuations"; jq .lines "$S/a.records.json"; exit 1; }
cj a status > "$S/a.status.json"
jq -e ".lines[0].state == \"diverged\"" "$S/a.status.json" >/dev/null || { echo "    status hides the divergence"; exit 1; }

# B: R3 is newer by the clock and still not a winner
cj b sync >/dev/null
rc=0; cj b plan > "$S/b.plan.json" || rc=$?
[ "$rc" = 10 ] || { echo "    a diverged plan exited $rc"; exit 1; }
jq -e --arg r3 "$R3" ".status == \"conflict\" and .record.id == \$r3
       and ([.blockers[].code] | (index(\"line_diverged\") or index(\"would_diverge\")))" "$S/b.plan.json" >/dev/null \
  || { echo "    the divergence is not a conflict"; jq "{status, blockers}" "$S/b.plan.json"; exit 1; }
n0="$(ls "$S/b/repo/.ai/local/state/handovers" | wc -l | tr -d " ")"
rc=0; cj b resume > "$S/b.resume.json" || rc=$?
[ "$rc" = 10 ] && jq -e ".resumed == false" "$S/b.resume.json" >/dev/null || { echo "    a conflict was resumed"; exit 1; }
[ "$(ls "$S/b/repo/.ai/local/state/handovers" | wc -l | tr -d " ")" = "$n0" ] || { echo "    a declined resume wrote a handover"; exit 1; }
# a person chooses
cj b plan --record "$R3" > "$S/b.plan2.json"
jq -e ".status == \"ready_with_warnings\" and ([.warnings[].code] | index(\"line_diverged\") and index(\"would_diverge\"))" "$S/b.plan2.json" >/dev/null \
  || { echo "    an explicit choice is not planned with its warnings"; jq "{status, warnings, blockers}" "$S/b.plan2.json"; exit 1; }
exit 0

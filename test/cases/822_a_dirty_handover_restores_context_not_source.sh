# majordomus-covers: handover
# majordomus-timeout: 600
# A handover published from a dirty tree restores its context on another machine and says
# plainly that its source did not travel (ADR 0105): the plan is ready_with_warnings with
# source_incomplete, never ready, the uncommitted paths are listed and none of their content
# appears on the other machine. And the source check before it: a commit that was never
# pushed is head_missing, an older local branch is local_behind, another branch is
# branch_differs — each requires_source_update, exit 10, with the git command to run and
# nothing run.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
S="$(mktemp -d "${TMPDIR:-/tmp}/mj822.XXXXXX")"; trap 'rm -rf "$S"' EXIT
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
echo "work in progress on the mac" >> "$S/a/repo/lib/a"
echo "an untracked draft" > "$S/a/repo/lib/new"
handover a "half of lib/a is rewritten" "finish lib/a"
cj a publish > "$S/a.pub.json" || { cat "$S/a.pub.json"; exit 1; }
jq -e ".record.working_tree == \"dirty\" and .record.changed_total == 2" "$S/a.pub.json" >/dev/null \
  || { echo "    the record does not say the tree was dirty"; jq .record "$S/a.pub.json"; exit 1; }
cj a sync >/dev/null || exit 1

clone b; on b git checkout -q feature/x
cj b sync >/dev/null || exit 1
cj b plan > "$S/b.plan.json"
jq -e ".status == \"ready_with_warnings\" and .source.relation == \"exact\" and .source.origin_dirty
       and (.source.origin_changed == [\"lib/a\", \"lib/new\"])
       and ([.warnings[].code] | index(\"source_incomplete\"))" "$S/b.plan.json" >/dev/null \
  || { echo "    a dirty origin is not reported as incomplete source"; jq "{status, source, warnings}" "$S/b.plan.json"; exit 1; }
cj b resume > "$S/b.resume.json" || { jq . "$S/b.resume.json"; exit 1; }
jq -e ".resumed" "$S/b.resume.json" >/dev/null || exit 1
f="$(ls "$S"/b/repo/.ai/local/state/handovers/*continuity*.md)"
expect_grep "working_tree: dirty" "$f"
expect_grep "  - lib/a" "$f"
expect_grep "finish lib/a" "$f"
# the uncommitted work itself did not travel, and was not invented
if grep -q "work in progress" "$S/b/repo/lib/a" || [ -e "$S/b/repo/lib/new" ]; then
  echo "    uncommitted source appeared on the other machine"; exit 1
fi

# --- a commit that never reached the remote
on a git commit -qam "lib/a done" ; on a git add lib/new; on a git commit -qm "lib/new"
handover a "lib/a is done" "write the docs"
cj a publish > "$S/a.pub2.json"; REC2="$(jq -r .record.id "$S/a.pub2.json")"
cj a sync >/dev/null
cj b sync >/dev/null
rc=0; cj b plan --record "$REC2" > "$S/b.plan2.json" || rc=$?
[ "$rc" = 10 ] || { echo "    a missing commit exited $rc, not 10"; exit 1; }
jq -e ".status == \"requires_source_update\" and .source.relation == \"head_missing\"
       and (.actions | index(\"git fetch origin\"))" "$S/b.plan2.json" >/dev/null \
  || { echo "    head_missing is not planned"; jq "{status, source, actions}" "$S/b.plan2.json"; exit 1; }
rc=0; cj b resume --record "$REC2" > "$S/b.resume2.json" || rc=$?
[ "$rc" = 10 ] && jq -e ".resumed == false" "$S/b.resume2.json" >/dev/null \
  || { echo "    a resume ahead of its source proceeded"; exit 1; }
# --- pushed and fetched, but not pulled: the local branch is behind
on a git push -q origin feature/x 2>/dev/null; on b git fetch -q origin
cj b plan --record "$REC2" > "$S/b.plan3.json" || true
jq -e ".source.relation == \"local_behind\" and (.actions | index(\"git pull --ff-only origin feature/x\"))" "$S/b.plan3.json" >/dev/null \
  || { echo "    local_behind is not planned"; jq "{status, source, actions}" "$S/b.plan3.json"; exit 1; }
# --- on another branch
on b git checkout -q main
cj b plan --record "$REC2" > "$S/b.plan4.json" || true
jq -e ".source.relation == \"branch_differs\" and (.actions | index(\"git switch feature/x\"))" "$S/b.plan4.json" >/dev/null \
  || { echo "    branch_differs is not planned"; jq "{status, source, actions}" "$S/b.plan4.json"; exit 1; }
# --- git does what the plan said, and the plan is satisfied
on b git checkout -q feature/x; on b git pull -q --ff-only origin feature/x 2>/dev/null
cj b plan --record "$REC2" > "$S/b.plan5.json"
jq -e ".source.relation == \"exact\" and .status != \"requires_source_update\"" "$S/b.plan5.json" >/dev/null \
  || { echo "    the source is in place and the plan still asks for it"; jq "{status, source}" "$S/b.plan5.json"; exit 1; }
exit 0

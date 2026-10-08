# majordomus-covers: start handover
# majordomus-timeout: 600
# Two machines working on two different things at once is ordinary (ADR 0105): A on issue
# one, B on issue two, each publishing its own line. After both sync, the store holds two
# linear lines and no divergence; a plan on B for the handover of A is about the source (another
# branch), never a conflict.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
S="$(mktemp -d "${TMPDIR:-/tmp}/mj824.XXXXXX")"; trap 'rm -rf "$S"' EXIT
git init -q --bare -b main "$S/shared.git"

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
on a git checkout -qb feature/one; on a git push -q -u origin feature/one 2>/dev/null
on a "$MJ" start "Issue one" --scope lib >/dev/null 2>&1
handover a "one is half done" "finish one"
cj a publish --issue "#1" >/dev/null || exit 1
cj a sync >/dev/null || exit 1
clone b
on b git checkout -qb feature/two; on b git push -q -u origin feature/two 2>/dev/null
on b "$MJ" start "Issue two" --scope lib >/dev/null 2>&1
handover b "two has begun" "continue two"
cj b publish --issue "#2" >/dev/null || exit 1
cj b sync > "$S/b.sync.json" || exit 1
cj a sync >/dev/null || exit 1
for m in a b; do
  cj "$m" status > "$S/$m.status.json"
  jq -e "(.lines | length) == 2 and all(.lines[]; .state == \"linear\")
         and ([.diagnostics[].code] | index(\"continuity.diverged\") | not)" "$S/$m.status.json" >/dev/null \
    || { echo "    $m reports separate work as a conflict"; jq "{lines, diagnostics}" "$S/$m.status.json"; exit 1; }
done
jq -e ".resumable | length == 1 and .[0].issue == \"#2\"" "$S/a.status.json" >/dev/null || { echo "    A does not see B as resumable"; exit 1; }
cj b plan > "$S/b.plan.json" || true  # requires_source_update exits 10
jq -e ".status == \"requires_source_update\" and .source.relation == \"branch_differs\"
       and ([.blockers[].code] == [\"branch_differs\"])" "$S/b.plan.json" >/dev/null \
  || { echo "    separate work is planned as something else than a branch to switch to"; jq "{status, blockers}" "$S/b.plan.json"; exit 1; }
exit 0

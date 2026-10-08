# majordomus-covers: handover
# majordomus-timeout: 600
# Offline first (ADR 0105): with the remote gone, a publication still succeeds — it is a
# local ref — and a sync reports the remote unreachable, exits 10, changes nothing and leaves
# the record pending. When the remote returns, the next sync publishes it, and another
# machine finds it. Nothing is lost on the way.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
S="$(mktemp -d "${TMPDIR:-/tmp}/mj825.XXXXXX")"; trap 'rm -rf "$S"' EXIT
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
on a git checkout -qb feature/x; on a git push -q -u origin feature/x 2>/dev/null
mv "$S/shared.git" "$S/away.git"
handover a "written on a plane" "land and sync"
cj a publish > "$S/pub.json" || { echo "    an offline publication failed"; cat "$S/pub.json"; exit 1; }
REC="$(jq -r .record.id "$S/pub.json")"
rc=0; cj a sync > "$S/sync1.json" || rc=$?
[ "$rc" = 10 ] || { echo "    an unreachable remote exited $rc, not 10"; exit 1; }
jq -e ".action == \"unreachable\" and .published == 0 and ([.diagnostics[].code] | index(\"continuity.remote_unreachable\"))" "$S/sync1.json" >/dev/null \
  || { echo "    the unreachable remote is not reported"; jq . "$S/sync1.json"; exit 1; }
cj a status > "$S/status1.json"
jq -e ".store.records == 1 and (.store.sync == \"never_synced\" or .store.sync == \"pending\") and .last_sync.outcome == \"unreachable\"" "$S/status1.json" >/dev/null \
  || { echo "    the pending record is not reported as pending"; jq "{store, last_sync}" "$S/status1.json"; exit 1; }
mv "$S/away.git" "$S/shared.git"
cj a sync > "$S/sync2.json" || { echo "    the sync after the remote returned failed"; jq . "$S/sync2.json"; exit 1; }
jq -e ".published == 1 and .store.sync == \"in_sync\"" "$S/sync2.json" >/dev/null || { echo "    nothing was published"; jq . "$S/sync2.json"; exit 1; }
clone b; on b git checkout -q feature/x
cj b sync >/dev/null || exit 1
cj b status > "$S/b.status.json"
jq -e --arg r "$REC" ".resumable[0].id == \$r" "$S/b.status.json" >/dev/null || { echo "    B does not find the record"; exit 1; }
exit 0

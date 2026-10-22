# majordomus-covers: capture session
# A lifecycle step run against an absent or stale executable says so, and the episode goes on
# (I3320).
#
# The start event's side steps — ensure the server, join the mesh, the keep-awake brief — run
# the Rust executable. Each used to test `-x` on its own and return quietly, and an executable
# handed over through MAJORDOMUS_BIN was never judged at all, so one built before the contract
# its tree declares ran with old semantics and nothing said so. Now every one of them goes
# through mj_rust_lifecycle (lib/rust_bin.sh): the contract is LIFECYCLE_CONTRACT in the
# crate's lib.rs, the executable reports its own through `distribution build`, and a step that
# cannot run it leaves provider.event.failed naming the cause. The hook still exits 0 and the
# episode still opens and closes: the hard gate on the episode's own opener and closer is
# I3141's, when the Rust closer becomes the writer. Stand-in executables answer for the real
# one, so nothing here builds:
#
#   1  the contract is declared once, as a number, and a stand-in reports it the real way
#   2  the resolution, run by hand: absent, stale (older contract), unanswering → exit 12 and
#      the cause on stderr; current and newer → the path and 0; a tree with no crate judges
#      nothing; what an executable reported is kept per path, size and mtime
#   3  a start event with an absent executable: provider.event.failed for the server and for
#      the brief, each naming `absent`; the episode opened; the hook exited 0
#   4  the same with an executable of an older contract: `stale`, and no `serve ensure` ran
#   5  a current one: no failure, and the server step ran it
#   6  doctor reports the failed events; the end event still closes the episode
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID MAJORDOMUS_BIN
fail() { printf '    %s\n' "$*"; exit 1; }
export XDG_CACHE_HOME="$T/cache"

# ---------------------------------------------------------------- 1. declared once
need="$(sed -n 's/^pub const LIFECYCLE_CONTRACT: u32 = \([0-9][0-9]*\);$/\1/p' "$ROOT/apps/majordomus-cli/src/lib.rs")"
[ -n "$need" ] || fail "lib.rs declares no LIFECYCLE_CONTRACT the shell can read"
n="$(grep -rn 'LIFECYCLE_CONTRACT: u32 =' "$ROOT/apps/majordomus-cli/src" | wc -l | tr -d ' ')"
[ "$n" = 1 ] || fail "LIFECYCLE_CONTRACT is declared $n times"
grep -q 'lifecycle_contract: crate::LIFECYCLE_CONTRACT' "$ROOT/apps/majordomus-cli/src/capability/builtin/distribution.rs" \
  || fail "distribution build does not report the contract the executable was built with"

# A stand-in answers `distribution build` with the contract in $CONTRACT ("none" for an
# executable from before the field, "refuse" for one that does not answer), records every call,
# and says nothing useful to `serve ensure`, so no server is started.
mkdir -p stand-in
cat > stand-in/majordomus <<'SH'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$CALLS"
if [ "$1 $2" = "distribution build" ]; then
  case "$CONTRACT" in
    refuse) exit 15 ;;
    none) printf '{\n  "version": "0.0.0",\n  "commit": "stand-in"\n}\n' ;;
    *) printf '{\n  "version": "0.0.0",\n  "lifecycle_contract": %s,\n  "commit": "stand-in"\n}\n' "$CONTRACT" ;;
  esac
  exit 0
fi
[ "$1 $2" = "serve ensure" ] && { echo "not ensured: a stand-in"; exit 0; }
exit 0
SH
chmod +x stand-in/majordomus
BIN="$T/stand-in/majordomus"
export CALLS="$T/calls"; : > "$CALLS"

# ---------------------------------------------------------------- 2. the resolution, by hand
resolve() {
  # shellcheck source=../../lib/rust_bin.sh
  ( . "$ROOT/lib/rust_bin.sh"; mj_rust_lifecycle "$ROOT" "$T" ) > "$T/out" 2> "$T/err"
}
fresh_cache() { rm -rf "$XDG_CACHE_HOME"; }
rc=0; MAJORDOMUS_BIN="$T/nothing-here" resolve || rc=$?
[ "$rc" = 12 ] || fail "an absent executable exited $rc, not 12"
grep -q "^absent: $T/nothing-here is not an executable" "$T/err" || fail "absent cause not named: $(cat "$T/err")"
[ -s "$T/out" ] && fail "an absent executable still printed a path"

for case in "none:stale: $BIN implements lifecycle contract 0; this tree declares $need" \
            "0:stale: $BIN implements lifecycle contract 0; this tree declares $need" \
            "refuse:unanswering: $BIN did not say"; do
  fresh_cache; rc=0
  CONTRACT="${case%%:*}" MAJORDOMUS_BIN="$BIN" resolve || rc=$?
  [ "$rc" = 12 ] || fail "contract ${case%%:*} exited $rc, not 12"
  grep -qF "${case#*:}" "$T/err" || fail "contract ${case%%:*}: cause not named: $(cat "$T/err")"
done

fresh_cache; rc=0; CONTRACT="$need" MAJORDOMUS_BIN="$BIN" resolve || rc=$?
[ "$rc" = 0 ] && [ "$(cat "$T/out")" = "$BIN" ] || fail "a current executable was refused ($rc): $(cat "$T/err")"
fresh_cache; rc=0; CONTRACT="$((need + 1))" MAJORDOMUS_BIN="$BIN" resolve || rc=$?
[ "$rc" = 0 ] || fail "an executable of a newer contract was refused: $(cat "$T/err")"

# kept per path, size and mtime: the same file is not asked twice, a changed one is
: > "$CALLS"; CONTRACT="$need" MAJORDOMUS_BIN="$BIN" resolve || fail "the cached answer was refused"
[ "$(grep -c '^distribution build' "$CALLS")" = 0 ] || fail "an unchanged executable was asked again"
printf '# changed\n' >> "$BIN"
CONTRACT=0 MAJORDOMUS_BIN="$BIN" resolve && fail "a changed executable was judged by its old answer"
[ "$(grep -c '^distribution build' "$CALLS")" = 1 ] || fail "a changed executable was not asked again"

# a tree with no crate (an installed release) declares nothing, and its executable is its own
mkdir -p installed/libexec && cp "$BIN" installed/libexec/majordomus-cli
fresh_cache; rc=0
( . "$ROOT/lib/rust_bin.sh"; CONTRACT=0 mj_rust_lifecycle "$T/installed" "$T" ) > "$T/out" 2> "$T/err" || rc=$?
[ "$rc" = 0 ] || fail "a tree without the crate judged its own executable: $(cat "$T/err")"

# ---------------------------------------------------------------- 3-6. the start event
mkdir repo && cd repo || exit 1
git init -q && git -c user.email=case@example.invalid -c user.name=case commit -q --allow-empty -m root
"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A >/dev/null 2>&1; git -c user.email=case@example.invalid -c user.name=case commit -qm base >/dev/null 2>&1 || true
"$MJ" capture install >/dev/null
# how a repository that is not the tool's own checkout reaches majordomus
PATH="$(dirname "$MJ"):$PATH"; export PATH
grep -q '^  ensure_server_on_start: true ' .ai/repo/policy.yaml || fail "the fixture does not ensure a server on start"
grep -q '^  brief_on_interval: true ' .ai/repo/policy.yaml || fail "the fixture does not keep the session awake"
STATE=.ai/local/state
LEDGER="$STATE/ledger.jsonl"
failed() { grep '"event":"provider.event.failed"' "$LEDGER" 2>/dev/null || true; }
start_event() { printf '{"session_id":"%s","source":"startup"}' "$1" | ./.claude/hooks/majordomus-session-start; }
end_event() { printf '{"session_id":"%s","reason":"clear"}' "$1" | ./.claude/hooks/majordomus-session-end; }

# 3. absent
rc=0; MAJORDOMUS_BIN="$T/nothing-here" start_event a1 > "$T/start.out" 2>&1 || rc=$?
[ "$rc" = 0 ] || fail "the start hook exited $rc; it belongs to the provider and exits 0"
[ -f "$STATE/sessions-open/a1.yaml" ] || fail "an absent executable stopped the episode from opening"
failed | grep -q '"provider_event":"start".*"reason":"no server was ensured: the executable is absent: ' \
  || fail "no failure names the server and the absent executable: $(failed)"
failed | grep -q '"reason":"the keep-awake brief was not given: the executable is absent: ' \
  || fail "no failure names the brief and the absent executable: $(failed)"
grep -q 'not ensured: the executable is absent' "$T/start.out" || fail "the briefing does not say why no server: $(cat "$T/start.out")"

# 4. an executable of an older contract
: > "$CALLS"; before="$(failed | wc -l | tr -d ' ')"
fresh_cache; CONTRACT=0 MAJORDOMUS_BIN="$BIN" start_event a2 > "$T/start.out" 2>&1 || fail "the start hook failed"
[ -f "$STATE/sessions-open/a2.yaml" ] || fail "a stale executable stopped the episode from opening"
[ "$(failed | wc -l | tr -d ' ')" -gt "$before" ] || fail "a stale executable left no failure"
failed | grep -q "no server was ensured: the executable is stale: .*implements lifecycle contract 0; this tree declares $need" \
  || fail "the failure does not name the contracts: $(failed | tail -n 2)"
grep -q '^serve ensure' "$CALLS" && fail "a stale executable was still asked to ensure the server"

# 5. a current one
: > "$CALLS"; before="$(failed | wc -l | tr -d ' ')"
fresh_cache; CONTRACT="$need" MAJORDOMUS_BIN="$BIN" start_event a3 > "$T/start.out" 2>&1 || fail "the start hook failed"
[ "$(failed | wc -l | tr -d ' ')" = "$before" ] || fail "a current executable was recorded as a failure: $(failed | tail -n 1)"
grep -q '^serve ensure' "$CALLS" || fail "a current executable was not run: $(cat "$CALLS")"

# 6. health reads it, and the episode closes
rc=0; "$MJ" doctor > "$T/doctor" 2>&1 || rc=$?
grep -q 'lifecycle event(s) arrived and did not complete their work' "$T/doctor" \
  || fail "doctor does not report the failed lifecycle events: $(grep -i lifecycle "$T/doctor")"
MAJORDOMUS_BIN="$T/nothing-here" end_event a1 >/dev/null 2>&1 || fail "the end hook failed"
[ ! -f "$STATE/sessions-open/a1.yaml" ] || fail "the end event did not close the episode an absent executable opened"
grep -q '"event":"session.closed"' "$LEDGER" || fail "no session.closed was written"

echo "    a lifecycle step names an absent or stale executable, and the episode goes on"

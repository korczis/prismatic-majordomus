# majordomus-covers: none
# majordomus-timeout: 180
# An enrolled machine's servers bind the mesh-facing interface with nothing run by hand
# (ADR 0234, I2411).
#
# On 2026-11-22 the owner's four machines were linked only because a person had started
# `majordomus serve --host 0.0.0.0 --port 8791 --idle 0` by hand on each: the server a session
# gets binds loopback unless MAJORDOMUS_HTTP_HOST names an interface, the variable was set on
# none of them, and a runtime on loopback advertises an address nobody can dial. The
# repository's committed declaration listed every one of their keys the whole time.
#
# What is held, in a fixture whose trunk commits an enabled declaration and whose node
# identity lives outside it:
#
#   1  no identity yet: loopback
#   2  the key enrolled in the working tree only: loopback (a working tree cannot widen trust)
#   3  the key committed: `serve` binds 0.0.0.0 and says why, naming the opt-out; the server
#      `serve ensure` starts binds it; `desired.host` says it
#   4  MAJORDOMUS_HTTP_HOST=127.0.0.1 and --host 127.0.0.1 keep loopback: the opt-out
#   5  a linked worktree of the enrolled machine binds 0.0.0.0 too
#   6  the declaration committed disabled: loopback
#
# Every mesh transport of the fixture is off (no multicast, no tailnet, no hub), so nothing is
# sent anywhere; only the bind is observed.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "jq not installed"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# this case holds the bind nothing names, so the harness's loopback default is taken back
unset MAJORDOMUS_HTTP_HOST
# the logs, the node identity and the linked worktree live outside the fixture repository, so
# that no `git add -A` of the fixture takes them in
S="$(mktemp -d "${TMPDIR:-/tmp}/mj4654.XXXXXX")"
XDG_STATE_HOME="$S/state"; export XDG_STATE_HOME
pid=""
cleanup() {
  [ -n "$pid" ] && kill "$pid" 2>/dev/null || true
  "$RB" serve stop >/dev/null 2>&1 || true
  [ -d "$S/wt" ] && ( cd "$S/wt" && "$RB" serve stop >/dev/null 2>&1 ) || true
  rm -rf "$S"
}
trap cleanup EXIT
fail() { printf '    %s\n' "$*"; exit 1; }
gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }

"$MJ" init >/dev/null
DECL=.ai/repo/mesh/mesh.yaml
[ -f "$DECL" ] || fail "init seeded no mesh declaration at $DECL"
declare_mesh() {   # enabled
  printf 'schema: mesh/v1\nkind: mesh-declaration\nid: mesh\nenabled: %s\nmulticast:\n  enabled: false\ntailscale:\n  enabled: false\nbroadcast:\n  mode: disabled\ntrust:\n  policy: deny_unknown\n  allow: []\n' "$1" > "$DECL"
}
declare_mesh true
gitq add -A >/dev/null && gitq commit -qm "an enabled mesh that lists nobody"

# Start `serve` with the arguments given, wait for it to say where it listens, stop it, and
# leave its log in $S/$1.log.
served() {   # name [serve arguments...]
  local name="$1" n=0; shift
  "$RB" serve --port 0 --idle 60 "$@" </dev/null >"$S/$name.log" 2>&1 &
  pid=$!
  while [ $n -lt 300 ] && ! grep -q 'shared server listening' "$S/$name.log" 2>/dev/null; do
    n=$((n+1)); sleep 0.1
  done
  kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; pid=""
  grep -q 'shared server listening' "$S/$name.log" || { cat "$S/$name.log"; fail "serve ($name) never listened"; }
}
on_loopback() { grep -q 'listening on http://127\.0\.0\.1:' "$S/$1.log" || { cat "$S/$1.log"; fail "$2"; }; }
on_all() { grep -q 'listening on http://0\.0\.0\.0:' "$S/$1.log" || { cat "$S/$1.log"; fail "$2"; }; }

# ---------------------------------------------------------------- 1. no identity yet
served none
on_loopback none "with no node identity the bind left loopback"

# ---------------------------------------------------------------- 2. enrolled, not committed
"$RB" mesh enrol >"$S/enrol.out" 2>&1 || { cat "$S/enrol.out"; fail "mesh enrol failed"; }
key="$(XDG_STATE_HOME="$XDG_STATE_HOME" "$RB" mesh identity --format json 2>/dev/null | jq -r '.identity.public_key // empty')"
[ -n "$key" ] || fail "mesh identity printed no public key"
grep -qF "$key" "$DECL" || fail "mesh enrol did not list the key in $DECL"
served uncommitted
on_loopback uncommitted "a key listed only in the working tree moved the bind: a branch could widen it"

# ---------------------------------------------------------------- 3. enrolled and committed
gitq add -A >/dev/null && gitq commit -qm "enrol this machine"
served enrolled
on_all enrolled "an enrolled machine's server stayed on loopback"
grep -q 'is listed by the committed, enabled mesh declaration' "$S/enrolled.log" \
  || { cat "$S/enrolled.log"; fail "the bind did not say the enrolment chose it"; }
grep -q 'MAJORDOMUS_HTTP_HOST=127.0.0.1 keeps it on loopback' "$S/enrolled.log" \
  || { cat "$S/enrolled.log"; fail "the bind did not name the opt-out"; }
"$RB" serve status --format json >"$S/status.json" 2>/dev/null || true
[ "$(jq -r '.desired.host // empty' "$S/status.json")" = 0.0.0.0 ] \
  || { cat "$S/status.json"; fail "desired.host is not what an enrolled machine binds"; }
"$RB" serve ensure --port 0 --idle 120 --wait 40 --format json >"$S/ensure.json" 2>"$S/ensure.err" \
  || { cat "$S/ensure.json" "$S/ensure.err"; fail "serve ensure did not converge"; }
case "$(jq -r '.url // empty' "$S/ensure.json")" in
  http://0.0.0.0:*) ;;
  *) cat "$S/ensure.json"; fail "the server serve ensure started is not on the mesh-facing interface" ;;
esac
"$RB" serve stop >/dev/null 2>&1 || true

# ---------------------------------------------------------------- 4. the opt-out
MAJORDOMUS_HTTP_HOST=127.0.0.1 served optout
on_loopback optout "MAJORDOMUS_HTTP_HOST=127.0.0.1 did not keep an enrolled machine on loopback"
served flag --host 127.0.0.1
on_loopback flag "--host 127.0.0.1 did not win over the enrolment"

# ---------------------------------------------------------------- 5. a linked worktree
gitq worktree add -q -b side "$S/wt" >/dev/null 2>&1 || fail "could not add a linked worktree"
( cd "$S/wt" && served worktree ) || exit 1
on_all worktree "a linked worktree of an enrolled machine stayed on loopback"

# ---------------------------------------------------------------- 6. disabled
declare_mesh false
# the key stays listed: only the switch moved
printf '    - %s\n' "$key" >> "$DECL"
sed -i.bak 's/^  allow: \[\]$/  allow:/' "$DECL" && rm -f "$DECL.bak"
gitq add -A >/dev/null && gitq commit -qm "turn the mesh off"
served disabled
on_loopback disabled "a disabled declaration moved the bind"
exit 0

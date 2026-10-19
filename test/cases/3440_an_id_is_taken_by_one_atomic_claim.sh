# majordomus-timeout: 300
# majordomus-covers: none
# An id is taken by one atomic claim (I3040, ADR 0177).
#
# `scripts/case-numbers next` and `majordomus adr next` survey and say of their answer that it
# is a best reading, not a reservation; on 2026-10-17 I2795, case 3178 and case 3221 were each
# taken twice in one day. `majordomus mesh allocate <kind>` (plan.allocate) reserves the number
# with one exclusive mesh claim on `id:<kind>/<n>`. Proved here over a running mesh:
#   1. without a server nothing is reserved, and the command says so and exits 10;
#   2. a case, an ADR and an issue number are each one past every source (the tree, every
#      ref), and the answer names the sources it surveyed;
#   3. a second allocation of a kind is one past the first: the journal's reservation is spent;
#   4. six allocations at once, from six sessions, answer six different numbers;
#   5. a number already claimed on its key is spent for every later allocation;
#   6. nothing in the repository is written.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
RB="$(rust_bin)" || rust_bin_exit $?

R="$T/repo"; mkdir -p "$R"; R="$(cd "$R" && pwd -P)"
git -C "$R" init -q .
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
( cd "$R" && "$MJ" init >/dev/null )
mkdir -p "$R/test/cases" "$R/.ai/repo/adrs" "$R/.ai/repo/project/issues" "$R/.ai/repo/mesh"
printf '' > "$R/test/cases/41_a_case.sh"
printf '# 12. An ADR\n' > "$R/.ai/repo/adrs/0012-an-adr.md"
{ printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n'
  printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n'
  printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  repository: case-3440\n'
} > "$R/.ai/repo/mesh/majordomus.yaml"
git -C "$R" add -A >/dev/null && git -C "$R" commit -qm base >/dev/null
# an issue that exists only on another branch is still spent
git -C "$R" checkout -q -b elsewhere
( cd "$R" && pj_init && pj_milestone M000 && pj_issue I0007 M000 )
git -C "$R" add -A >/dev/null && git -C "$R" commit -qm "an issue elsewhere" >/dev/null
git -C "$R" checkout -q -
mkdir -p "$T/state"
export MAJORDOMUS_BIN="$RB" MAJORDOMUS_SHARE="$ROOT/share" MAJORDOMUS_LOG=warn XDG_STATE_HOME="$T/state"
S="$T/io"; mkdir -p "$S"
PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT
before="$(git -C "$R" status --porcelain)"
alloc() { rc=0; ( cd "$R" && "$RB" mesh allocate "$@" --format json ) > "$S/out" 2>"$S/err" || rc=$?; }

# ---------------------------------------------------------------- 1. no server, no reservation
alloc case --session a
[ "$rc" = 10 ] || { echo "    an allocation with no server must exit 10, got $rc:"; cat "$S/out" "$S/err"; exit 1; }
jq -e '.active == false' "$S/out" >/dev/null || { echo "    the refusal does not say no mesh is running:"; cat "$S/out"; exit 1; }

# ---------------------------------------------------------------- 2. one past every source
( cd "$R" && exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$S/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
i=0; until [ -n "$(jq -r '.url // empty' "$R/.ai/local/state/mcp/server.json" 2>/dev/null)" ]; do
  i=$((i + 1)); [ "$i" -lt 300 ] || { echo "    the server never published a lease:"; cat "$S/serve.log"; exit 1; }; sleep 0.2
done
alloc case --session a
[ "$rc" = 0 ] || { echo "    allocate case failed ($rc):"; cat "$S/out" "$S/err"; exit 1; }
jq -e '.value == "42" and .reservation == "id:case/42" and (.claim | length > 0) and .skipped == []
  and ([.sources[].source] | index("tree") != null and index("refs") != null and index("worktrees") != null and index("mesh") != null)
  and (.sources[] | select(.source == "tree") | .highest == 41)' "$S/out" >/dev/null \
  || { echo "    the case number is not one past the tree's highest:"; cat "$S/out"; exit 1; }
alloc adr --session a
jq -e '.value == "0013" and .reservation == "id:adr/13"' "$S/out" >/dev/null \
  || { echo "    the ADR number is not one past 0012:"; cat "$S/out"; exit 1; }
alloc issue --session a
jq -e '.value == "I0008" and (.sources[] | select(.source == "refs") | .highest == 7)' "$S/out" >/dev/null \
  || { echo "    an issue that exists only on another branch was not spent:"; cat "$S/out"; exit 1; }

# ---------------------------------------------------------------- 3. the journal's reservation is spent
alloc case --session b
jq -e '.value == "43"' "$S/out" >/dev/null || { echo "    a reserved number was handed out again:"; cat "$S/out"; exit 1; }

# ---------------------------------------------------------------- 4. six at once, six numbers
for n in 1 2 3 4 5 6; do
  ( cd "$R" && "$RB" mesh allocate case --session "c$n" --format json > "$S/par.$n" 2>&1 ) & PIDS="$PIDS $!"
done
wait_all() { for n in 1 2 3 4 5 6; do i=0; until jq -e '.value' "$S/par.$n" >/dev/null 2>&1; do i=$((i+1)); [ "$i" -lt 300 ] || return 1; sleep 0.1; done; done; }
wait_all || { echo "    a parallel allocation did not answer:"; cat "$S"/par.*; exit 1; }
got="$(for n in 1 2 3 4 5 6; do jq -r '.value' "$S/par.$n"; done | LC_ALL=C sort -n | tr '\n' ' ')"
[ "$got" = "44 45 46 47 48 49 " ] \
  || { echo "    six allocations at once did not answer six different numbers: $got"; cat "$S"/par.*; exit 1; }

# ---------------------------------------------------------------- 5. a claimed key is spent
( cd "$R" && "$RB" mesh claim id:case/60 --session x --intent "a block given by hand" ) >/dev/null 2>&1 \
  || { echo "    could not claim id:case/60"; exit 1; }
alloc case --session d
jq -e '.value == "61" and (.sources[] | select(.source == "mesh") | .highest == 60)' "$S/out" >/dev/null \
  || { echo "    a number claimed on its key was not spent:"; cat "$S/out"; exit 1; }

# ---------------------------------------------------------------- 6. nothing is written
[ "$(git -C "$R" status --porcelain)" = "$before" ] \
  || { echo "    an allocation wrote to the repository:"; git -C "$R" status --porcelain; exit 1; }
exit 0

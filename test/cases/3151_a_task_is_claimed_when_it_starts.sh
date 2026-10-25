# majordomus-timeout: 300
# majordomus-covers: start check finish
# A task is claimed when it starts (I2288). In a fresh linked worktree no shared server runs —
# no provider start event fired there — and until this case `start` claimed nothing, so the
# pre-push `finish --check` refused the task under `mesh` and the only cure was finishing it
# partial and starting again. On a repository that declares its mesh enabled, `start` now
# ensures the checkout's shared server itself (said on stderr), claims the scope, and the
# pre-push `finish --check` passes the mesh line. When `start` cannot bring a server up
# (MAJORDOMUS_RUNTIME=off) it says so as a WARN finding with the repair, and the repair works:
# `serve ensure` and then `check` claim the running task's scope, and `finish --check` passes.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share" MAJORDOMUS_BIN="$RB"
unset MAJORDOMUS_RUNTIME

R="$T/repo"
W1="$T/wt-fresh"
W2="$T/wt-repaired"
stop_servers() {
  local w
  for w in "$W1" "$W2" "$R"; do "$RB" serve stop --repo "$w" >/dev/null 2>&1 || true; done
}
trap stop_servers EXIT

mkdir -p "$R"
cd "$R" || exit 1
git init -q .
git config user.email t@example.com
git config user.name t
"$MJ" init >/dev/null
"$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a
mkdir -p .ai/repo/mesh
printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n' > .ai/repo/mesh/majordomus.yaml
printf 'multicast:\n  enabled: false\ntailscale:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  repository: case-3151\n' >> .ai/repo/mesh/majordomus.yaml
git add -A && git commit -qm "base, mesh enabled"
mkdir -p "$T/state" "$T/bin"
printf '#!/bin/sh\necho "gh: not here in this case" >&2\nexit 4\n' > "$T/bin/gh"
chmod +x "$T/bin/gh"
export PATH="$T/bin:$PATH" XDG_STATE_HOME="$T/state"
printf '# Objective\nx\n\n# Current State\ny\n\n# Next Action\nz\n' > "$T/note.md"
CUR=.ai/local/state/current.yaml

# ---------------------------------------------------------------- 1. a fresh worktree: start brings the server up and claims
git worktree add -q -b feature/fresh "$W1"
cd "$W1" || exit 1
[ -s .ai/local/state/mcp/server.json ] && { echo "    the fresh worktree already has a server lease"; exit 1; }
"$MJ" start "Fresh worktree work" --scope lib >"$T/start1.out" 2>"$T/start1.err" \
  || { echo "    start failed in a fresh worktree"; cat "$T/start1.out" "$T/start1.err"; exit 1; }
# start says on stderr that it brought a server up, and which: serve ensure's own line
grep -q 'ready .*(started by this call)' "$T/start1.err" \
  || { echo "    start did not say which server it started"; cat "$T/start1.err"; exit 1; }
grep -q '^mesh_claim: ' "$CUR" \
  || { echo "    start left the task unclaimed in a fresh worktree"; cat "$T/start1.out" "$T/start1.err" "$CUR"; exit 1; }
grep -q '^WARN mesh' "$T/start1.out" && { echo "    a claimed start still warned"; cat "$T/start1.out"; exit 1; }
"$MJ" finish --check >"$T/fc1" 2>&1 || true
grep -q '^FAIL mesh' "$T/fc1" && { echo "    the pre-push finish --check refused a task start claimed"; cat "$T/fc1"; exit 1; }
grep -q '^OK   mesh .*claimed on the mesh' "$T/fc1" \
  || { echo "    finish --check did not pass the mesh line"; cat "$T/fc1"; exit 1; }
expect_exit 0 "$MJ" finish --outcome partial --note "$T/note.md"
# a claim reaches the mesh through any server of the repository, so section 2 starts with none
"$RB" serve stop --repo "$W1" >"$T/stop1" 2>&1 || { echo "    the started server did not stop"; cat "$T/stop1"; exit 1; }

# ---------------------------------------------------------------- 2. start cannot bring a server up: a finding, then a repair
cd "$R" || exit 1
git worktree add -q -b feature/repaired "$W2"
cd "$W2" || exit 1
MAJORDOMUS_RUNTIME=off "$MJ" start "Off at first" --scope lib >"$T/start2.out" 2>"$T/start2.err" \
  || { echo "    start failed with the runtime off"; cat "$T/start2.out" "$T/start2.err"; exit 1; }
grep -q '^mesh_claim: ' "$CUR" && { echo "    a claim was recorded with the runtime off"; exit 1; }
[ -s .ai/local/state/mcp/server.json ] && { echo "    start brought a server up with the runtime off"; exit 1; }
grep -q '^WARN mesh .*not claimed on it: MAJORDOMUS_RUNTIME=off.*majordomus serve ensure, then majordomus check' "$T/start2.out" \
  || { echo "    start did not name the unclaimed scope, why, and the repair"; cat "$T/start2.out"; exit 1; }
"$MJ" finish --check >"$T/fc2" 2>&1 && { echo "    finish --check passed an unclaimed task"; cat "$T/fc2"; exit 1; }
grep -q '^FAIL mesh .*never claimed on it.*majordomus serve ensure, then majordomus check' "$T/fc2" \
  || { echo "    finish --check does not name the repair"; grep mesh "$T/fc2"; exit 1; }
# finish --check is a question: it claims nothing
grep -q '^mesh_claim: ' "$CUR" && { echo "    finish --check wrote a claim"; exit 1; }
"$RB" serve ensure --repo "$W2" --port 0 --idle 60 --wait 40 >"$T/ensure2" 2>&1 \
  || { echo "    serve ensure did not converge"; cat "$T/ensure2"; exit 1; }
"$MJ" check >"$T/check2" 2>&1 || true
grep -q '^mesh_claim: ' "$CUR" \
  || { echo "    check did not claim the running task's scope once a server answered"; cat "$T/check2" "$CUR"; exit 1; }
grep -q '^OK   mesh .*claimed on the mesh' "$T/check2" \
  || { echo "    check did not pass the mesh line after claiming"; cat "$T/check2"; exit 1; }
"$MJ" finish --check >"$T/fc3" 2>&1 || true
grep -q '^FAIL mesh' "$T/fc3" && { echo "    the repaired task is still refused"; cat "$T/fc3"; exit 1; }
grep -q '^OK   mesh .*claimed on the mesh' "$T/fc3" \
  || { echo "    finish --check did not pass the mesh line after the repair"; cat "$T/fc3"; exit 1; }
expect_exit 0 "$MJ" finish --outcome partial --note "$T/note.md"
exit 0

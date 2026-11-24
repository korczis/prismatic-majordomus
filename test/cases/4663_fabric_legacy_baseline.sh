# majordomus-exclusive: starts servers in disposable checkouts and samples their processes; a busy pool changes what it measures
# majordomus-covers: none
# majordomus-timeout: 600
#
# Majordomus Fabric S01 (gate G1): scripts/fabric-baseline measures today's runtime, one shared
# server per checkout (ADR 0003, ADR 0044), before a host agent changes it. This case proves the
# measurement itself, not a number:
#
#   A. positive: L1 (one repository, one checkout, one client) brings the legacy runtime up in a
#      fixture the script made, gets a real MCP answer (initialize, then tools/call
#      majordomus_peers), and reports one server for the one checkout, its listener, and one
#      attach sample with no failure, in the majordomus-fabric-baseline/v1 schema.
#   B. refusal: a directory that is not empty and not a marked fixture, and one inside a git work
#      tree, are refused (exit 3) before anything is created or started.
#   C. crash: with --kill-one-server, L2's first server is killed just before its clients
#      attach; whatever happens to them, every client is a sample or a failure
#      (samples + failures = attempts), so a crash is reported and never dropped.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "jq not installed"
command -v lsof >/dev/null 2>&1 || skip "lsof not installed"
FB="$ROOT/scripts/fabric-baseline"
# The case runs inside a temporary repository ($T is one), and a baseline refuses any directory
# inside a git work tree. Its fixtures are made beside it, outside every work tree, and removed.
FX="$(mktemp -d "${TMPDIR:-/tmp}/mj-4663.XXXXXX")"; FX="$(cd "$FX" && pwd -P)"
trap 'rm -rf "$FX"' EXIT

# ------------------------------------------------------------------------- A. positive, L1
mkdir -p "$FX/a"
rc=0; "$FB" --level L1 --runs 1 --hold 2 --bin "$RB" --fixture "$FX/a" --out "$T/a.json" 2> "$T/a.err" || rc=$?
[ "$rc" = 0 ] || { echo "    L1 exited $rc"; sed 's/^/    | /' "$T/a.err"; exit 1; }
jq -e '.schema == "majordomus-fabric-baseline/v1" and .level == "L1" and (.runs | length) == 1' "$T/a.json" >/dev/null \
  || { echo "    not a v1 baseline document:"; jq -c '{schema, level}' "$T/a.json"; exit 1; }
jq -e '.runs[0].sample.processes.servers == 1' "$T/a.json" >/dev/null \
  || { echo "    L1 should run exactly one server for its one checkout:"; jq -c '.runs[0].sample' "$T/a.json"; exit 1; }
jq -e '.runs[0].sample.sockets.tcp_listen >= 1' "$T/a.json" >/dev/null \
  || { echo "    the server's listener was not seen:"; jq -c '.runs[0].sample' "$T/a.json"; exit 1; }
jq -e '.runs[0].attempts.attach == 1 and .runs[0].failures.attach == 0 and (.runs[0].attach_ms | length) == 1' "$T/a.json" >/dev/null \
  || { echo "    the client did not get its answer:"; jq -c '.runs[0] | {attempts, failures, attach_ms}' "$T/a.json"; exit 1; }
jq -e '(.runs[0].enter_cold_ms | length) == 1 and (.runs[0].enter_warm_ms | length) == 1' "$T/a.json" >/dev/null \
  || { echo "    cold and warm entry were not both measured:"; jq -c '.runs[0]' "$T/a.json"; exit 1; }
jq -e '.policy_budgets.enter_ms != null and .summary.attach_ms.p50 != null' "$T/a.json" >/dev/null \
  || { echo "    the summary or the policy budgets are missing:"; jq -c '{policy_budgets, summary}' "$T/a.json"; exit 1; }
# The fixture's servers are gone afterwards: each was stopped through its own lease.
A="$FX/a"   # resolved above, as the servers' --repo is (macOS's $TMPDIR is a symlink)
left="$(ps -axo command= | grep -F "$A/" | grep -c ' serve ' || true)"
[ "$left" = 0 ] || { echo "    $left fixture server(s) still run after the baseline"; exit 1; }

# --------------------------------------------------------------------------- B. refusals
mkdir -p "$FX/b-dirty"; echo keep > "$FX/b-dirty/user-file"
rc=0; "$FB" --level L1 --runs 1 --bin "$RB" --fixture "$FX/b-dirty" > /dev/null 2> "$T/b.err" || rc=$?
[ "$rc" = 3 ] || { echo "    a non-empty unmarked directory was not refused (exit $rc)"; exit 1; }
grep -q 'refused: .* is not empty and does not carry' "$T/b.err" || { echo "    the refusal does not say why:"; cat "$T/b.err"; exit 1; }
[ "$(ls -A "$FX/b-dirty")" = user-file ] || { echo "    the refused directory was changed:"; ls -A "$FX/b-dirty"; exit 1; }
mkdir -p "$FX/b-git/inner"; git -C "$FX/b-git" init -q .
rc=0; "$FB" --level L1 --runs 1 --bin "$RB" --fixture "$FX/b-git/inner" > /dev/null 2> "$T/b2.err" || rc=$?
[ "$rc" = 3 ] || { echo "    a directory inside a git work tree was not refused (exit $rc)"; exit 1; }
grep -q 'inside a git work tree' "$T/b2.err" || { echo "    the refusal does not say why:"; cat "$T/b2.err"; exit 1; }

# ------------------------------------------------------------------------------ C. crash
mkdir -p "$FX/c"
rc=0; "$FB" --level L2 --runs 1 --hold 2 --bin "$RB" --fixture "$FX/c" --kill-one-server --out "$T/c.json" 2> "$T/c.err" || rc=$?
[ "$rc" = 0 ] || { echo "    L2 with a killed server exited $rc"; sed 's/^/    | /' "$T/c.err"; exit 1; }
jq -e '.runs[0].killed_servers == 1' "$T/c.json" >/dev/null || { echo "    no server was killed:"; jq -c '.runs[0]' "$T/c.json"; exit 1; }
jq -e '.runs[0] | ((.attach_ms | length) + .failures.attach) == .attempts.attach and .attempts.attach == 4' "$T/c.json" >/dev/null \
  || { echo "    a client was dropped instead of counted:"; jq -c '.runs[0] | {attempts, failures, attach_ms}' "$T/c.json"; exit 1; }
echo "    L1: $(jq -c '.runs[0].sample' "$T/a.json")"
echo "    L2 with a killed server: $(jq -c '.runs[0] | {attempts, failures}' "$T/c.json")"

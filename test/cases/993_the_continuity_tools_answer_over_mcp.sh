# majordomus-covers: none
# majordomus-timeout: 600
# The seven continuity tools, asked as a client asks them.
#
# Case 821 moves a session between two machines through the command line. The same seven
# operations are projected over MCP — majordomus_continuity_device, _publish, _sync,
# _status, _records, _plan and _resume — and nothing had ever sent one of them a
# `tools/call`: scripts/ci/mcp-tool-run-check named all seven as new debt the day the
# branch met the gate. A tool that only the equivalence property reaches is a tool nobody
# has asked a question about a repository.
#
# So the journey of case 821 is made here through the tools alone: machine A names its
# device, publishes its handover and syncs; machine B, a clone with its own home and its own
# state, syncs, sees the handover, plans and resumes. After each step the command line is
# asked the same question in the same clone, and what the tool said is held against it: a
# tool and a command that disagree about one store are two accounts of it.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
S="$(mktemp -d "${TMPDIR:-/tmp}/mj993.XXXXXX")"; trap 'rm -rf "$S"' EXIT
git init -q --bare "$S/shared.git"

# on MACHINE CMD...: run CMD in MACHINE's clone with MACHINE's own home and state
on() { m="$1"; shift; ( cd "$S/$m/repo" && HOME="$S/$m/home" XDG_STATE_HOME="$S/$m/state" "$@" ); }
mkdir -p "$S/a/home" "$S/b/home"

# tool MACHINE TOOL ARGUMENTS FILE: one MCP session in MACHINE's clone, alone — no port, no
# lease — sending TOOL one tools/call; the typed answer goes to FILE, or the case fails
# naming the tool and showing the frame it got instead
tool() {
  m="$1"; t="$2"; args="$3"; out="$4"
  { printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case993","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$t" "$args"
  } | on "$m" "$RB" mcp --standalone --repo "$S/$m/repo" 2>"$S/err" | sed -n 2p > "$S/frame.json"
  jq -e '.result.structuredContent' "$S/frame.json" > "$out" 2>/dev/null \
    || { echo "    $t on machine $m returned no typed answer:"; head -c 800 "$S/frame.json"; echo; sed 's/^/    | /' "$S/err" | head -5; exit 1; }
}
bad() { echo "    $1"; shift; [ $# -eq 0 ] || jq -c . "$1" | head -c 800; echo; exit 1; }

# --- machine A: a repository of the layer
mkdir -p "$S/a/repo"
on a git init -q -b main .
on a git config user.email a@example.com; on a git config user.name a
on a "$MJ" init >/dev/null; on a "$MJ" update >/dev/null
mkdir -p "$S/a/repo/lib"; echo a > "$S/a/repo/lib/a"

# ---------------------------------------------------------------- 1. continuity.device
# The tool names this machine's device, and the command line then finds the same device:
# one key per state directory, whichever surface asked first.
tool a majordomus_continuity_device '{"label":"macbook-pro"}' "$S/a.device.json"
jq -e '.device.label == "macbook-pro" and (.public_key | length) > 0' "$S/a.device.json" >/dev/null \
  || bad "majordomus_continuity_device did not name the device it was asked to:" "$S/a.device.json"
on a "$RB" continuity device --format json > "$S/a.device.cli.json" 2>"$S/err" || { cat "$S/err"; exit 1; }
[ "$(jq -r .public_key "$S/a.device.json")" = "$(jq -r .public_key "$S/a.device.cli.json")" ] \
  || { echo "    the tool and the command line name two different devices on one machine"; exit 1; }

# B's device, made by the tool in B's own state directory before B has a clone of its own
( cd "$S/a/repo" && HOME="$S/b/home" XDG_STATE_HOME="$S/b/state" "$RB" continuity device --label mac-mini --format json ) \
  > "$S/b.device.json" 2>"$S/err" || { cat "$S/err"; exit 1; }
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
printf '# Objective\nShip feature x\n\n# Current State\nthe parser is done\n\n# Next Action\nwrite the test for lib/a\n' \
  | on a "$MJ" handover >/dev/null 2>&1 || { echo "    handover failed"; exit 1; }

# ---------------------------------------------------------------- 2. continuity.publish
# The newest handover becomes a signed record that names its device and the issue it was
# given; nothing has left the machine yet.
tool a majordomus_continuity_publish '{"issue":"I-1842"}' "$S/a.pub.json"
REC_A="$(jq -r .record.id "$S/a.pub.json")"
jq -e '.record.device.label == "macbook-pro" and .record.issue == "I-1842" and (.record.id | length) > 0' "$S/a.pub.json" >/dev/null \
  || bad "majordomus_continuity_publish did not publish a record naming its device and issue:" "$S/a.pub.json"
if git --git-dir="$S/shared.git" rev-parse -q --verify refs/majordomus/continuity >/dev/null; then
  echo "    publish reached the remote: only a sync may"; exit 1
fi

# ---------------------------------------------------------------- 3. continuity.sync
# The record reaches the remote in a ref of its own, and no branch moves.
tool a majordomus_continuity_sync '{}' "$S/a.sync.json"
jq -e '.published == 1' "$S/a.sync.json" >/dev/null \
  || bad "majordomus_continuity_sync published nothing from machine A:" "$S/a.sync.json"
git --git-dir="$S/shared.git" rev-parse -q --verify refs/majordomus/continuity >/dev/null \
  || { echo "    the remote holds no continuity ref after the tool's sync"; exit 1; }
[ "$(git --git-dir="$S/shared.git" rev-parse feature/x)" = "$(on a git rev-parse HEAD)" ] \
  || { echo "    the tool's sync moved a branch"; exit 1; }

# --- machine B: a clone, and nothing else
mkdir -p "$S/b"; git clone -q "$S/shared.git" "$S/b/repo" 2>/dev/null
on b git config user.email b@example.com; on b git config user.name b
on b git checkout -q feature/x

# ---------------------------------------------------------------- 4. continuity.status
# Before a sync the tool finds nothing to resume, because it does not use the network; after
# one it finds A's record, from a trusted device, and says what the command line says.
tool b majordomus_continuity_status '{}' "$S/b.status0.json"
[ "$(jq '.resumable | length' "$S/b.status0.json")" = 0 ] \
  || bad "majordomus_continuity_status found a record before any sync:" "$S/b.status0.json"
tool b majordomus_continuity_sync '{}' "$S/b.sync.json"
jq -e '.fetched == 1' "$S/b.sync.json" >/dev/null \
  || bad "majordomus_continuity_sync fetched nothing on machine B:" "$S/b.sync.json"
tool b majordomus_continuity_status '{}' "$S/b.status.json"
jq -e --arg r "$REC_A" '.resumable[0].id == $r and .resumable[0].device.label == "macbook-pro" and .resumable[0].trust == "trusted"' \
  "$S/b.status.json" >/dev/null || bad "majordomus_continuity_status does not offer A's handover from a trusted device:" "$S/b.status.json"
on b "$RB" continuity status --format json > "$S/b.status.cli.json" || exit 1
[ "$(jq -c '[.resumable[] | {id, trust, label: .device.label}]' "$S/b.status.json")" = "$(jq -c '[.resumable[] | {id, trust, label: .device.label}]' "$S/b.status.cli.json")" ] \
  || { echo "    the tool and the command line offer different records to resume"; exit 1; }

# ---------------------------------------------------------------- 5. continuity.records
# The store as the tool reads it is the store the command line reads: one record, one line.
tool b majordomus_continuity_records '{}' "$S/b.records.json"
jq -e --arg r "$REC_A" '(.records | length) == 1 and .records[0].id == $r and (.lines | length) == 1' "$S/b.records.json" >/dev/null \
  || bad "majordomus_continuity_records does not hold exactly A's record:" "$S/b.records.json"
on b "$RB" continuity records --format json > "$S/b.records.cli.json" || exit 1
[ "$(jq -c '[.records[].id]' "$S/b.records.json")" = "$(jq -c '[.records[].id]' "$S/b.records.cli.json")" ] \
  || { echo "    the tool and the command line list different records"; exit 1; }

# ---------------------------------------------------------------- 6. continuity.plan
# What a resume would do, said before anything is written: ready, exact, and nothing of B's
# state exists yet.
tool b majordomus_continuity_plan '{}' "$S/b.plan.json"
jq -e --arg r "$REC_A" '.status == "ready" and .record.id == $r and .source.relation == "exact" and (.blockers | length) == 0' "$S/b.plan.json" >/dev/null \
  || bad "majordomus_continuity_plan is not a ready plan for A's record:" "$S/b.plan.json"
jq -e '.restores | index("handover") and index("next_action")' "$S/b.plan.json" >/dev/null \
  || bad "the plan does not say what it restores:" "$S/b.plan.json"
set -- "$S"/b/repo/.ai/local/state/handovers/*--continuity--*.md
[ ! -e "$1" ] || { echo "    a plan wrote a handover: only a resume may"; exit 1; }

# ---------------------------------------------------------------- 7. continuity.resume
# The handover and its next action become B's records, with the provenance that names A,
# and the shell tool then finds them by its own resolution.
tool b majordomus_continuity_resume '{}' "$S/b.resume.json"
jq -e '.resumed == true' "$S/b.resume.json" >/dev/null \
  || bad "majordomus_continuity_resume did not resume:" "$S/b.resume.json"
jq -e '.plan.task.title == "Ship the continuity fixture" and .plan.record.issue == "I-1842"
       and .plan.next_action == "write the test for lib/a" and .plan.record.device.label == "macbook-pro"' \
  "$S/b.resume.json" >/dev/null || bad "the resumed plan lost intent, issue, next action or provenance:" "$S/b.resume.json"
on b "$MJ" handover --resolve > "$S/b.resolve.txt" 2>&1 || { cat "$S/b.resolve.txt"; exit 1; }
expect_grep "write the test for lib/a" "$S/b.resolve.txt"
expect_grep "the parser is done" "$S/b.resolve.txt"
# no path of machine A's disk reached machine B through a tool either
if grep -rqF "$S/a" "$S/b/repo/.ai/local/state"; then
  echo "    a path of machine A reached machine B"; grep -rF "$S/a" "$S/b/repo/.ai/local/state" | head -3; exit 1
fi

# --- the gate that named these seven now finds each of them invoked
"$ROOT/scripts/ci/mcp-tool-run-check" > "$S/gate.txt" 2>&1 || true
for t in device plan publish records resume status sync; do
  if grep -q "majordomus_continuity_$t\b" "$S/gate.txt"; then
    echo "    scripts/ci/mcp-tool-run-check still names majordomus_continuity_$t as uninvoked"; exit 1
  fi
done
exit 0

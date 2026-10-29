# majordomus-covers: capability:machine.slot.status capability:machine.slot.acquire capability:machine.slot.release
# majordomus-timeout: 300
# Heavy work shares the machine through a counting slot (I4006).
#
# On 2026-10-22 eleven cargo processes of eleven worktrees queued for 47 min and more at 0% CPU
# on one shared build lock, pre-push hooks timed out inside that queue, and five cold private
# builds took free disk under 35 GiB, while rule project.the-cooperation-model-is-the-owners-word
# clause 7 said at most five builders and none below 35 GiB. `machine.slot` holds a build to
# those numbers, read from the policy's machine.heavy_work. This case proves, against a slot
# directory of its own and a fixture policy of two slots:
#
#   - the repository's policy declares the numbers the rule's statement says (5, 35 GiB);
#   - N holders are admitted; the N+1th waits its bound, is listed as a waiter with what it
#     waits on while it waits, and is refused (exit 75) naming every holder;
#   - a holder whose process is gone is reclaimed by the next acquisition, and named;
#   - below the disk floor an acquisition is refused at once, naming the measurement;
#   - the cargo build path of lib/rust_bin.sh asks for a slot before cargo runs: refused, no
#     cargo is started; admitted, cargo runs and the slot is given back when it ends.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
MAJORDOMUS_MACHINE_SLOTS="$T/slots"; export MAJORDOMUS_MACHINE_SLOTS
PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT
holder() { sleep 300 & PIDS="$PIDS $!"; LAST=$!; }

# ---------------------------------------------------------------- the rule's numbers are the policy's
POL="$ROOT/.ai/repo/policy.yaml"
RULE="$ROOT/.ai/repo/rules/project/the-cooperation-model-is-the-owners-word.v2.md"
slots="$(awk '/^machine:/{m=1} m && /^    slots:/{print $2; exit}' "$POL")"
floor="$(awk '/^machine:/{m=1} m && /^    min_free_gib:/{print $2; exit}' "$POL")"
[ "$slots" = 5 ] && [ "$floor" = 35 ] \
  || { echo "    the policy's machine.heavy_work says slots=$slots min_free_gib=$floor, not the rule's 5 and 35"; exit 1; }
grep '^statement:' "$RULE" | grep -q 'beside five others or below 35 GiB free' \
  || { echo "    the rule's statement no longer says five builders and 35 GiB; the policy and the rule must move together"; exit 1; }

# ---------------------------------------------------------------- a fixture of two slots
"$MJ" init >/dev/null
policy() {
  sed '/^machine:/,$d' .ai/repo/policy.yaml > .ai/repo/policy.tmp
  printf 'machine:\n  heavy_work:\n    slots: 2\n    min_free_gib: %s\n    wait_seconds: %s\n' "$1" "$2" >> .ai/repo/policy.tmp
  mv .ai/repo/policy.tmp .ai/repo/policy.yaml
}
policy 0 2
git add -A >/dev/null && git commit -qm layer

acquire() { # <pid> <purpose> [wait]; sets RC, OUT, ERR
  RC=0
  OUT="$("$RB" machine slot acquire --pid "$1" --purpose "$2" ${3:+--wait "$3"} 2>"$T/err")" || RC=$?
  ERR="$(cat "$T/err")"
}

# ---------------------------------------------------------------- N holders are admitted
holder; A=$LAST; holder; B=$LAST; holder; C=$LAST; holder; D=$LAST
acquire "$A" "build a"
[ "$RC" = 0 ] && case "$OUT" in admitted*) true ;; *) false ;; esac \
  || { echo "    the first holder was not admitted: $RC $OUT $ERR"; exit 1; }
acquire "$B" "build b"
[ "$RC" = 0 ] || { echo "    the second holder was not admitted: $RC $OUT $ERR"; exit 1; }
acquire "$B" "build b"
case "$OUT" in already_held*) ;; *) echo "    a holder asking again was not told it holds one: $OUT"; exit 1 ;; esac

# ---------------------------------------------------------------- the N+1th waits, is seen waiting, is refused
start="$(date +%s)"
( rc=0; "$RB" machine slot acquire --pid "$C" --purpose "build c" --wait 5 >"$T/c.out" 2>"$T/c.err" || rc=$?
  echo "$rc" >"$T/c.rc" ) &
CPID=$!
seen=""
i=0
while [ "$i" -lt 40 ] && [ ! -f "$T/c.rc" ]; do
  W="$("$RB" machine slot status --format json 2>/dev/null || true)"
  if printf '%s' "$W" | grep -q "\"pid\": $C" && printf '%s' "$W" | grep -q '"waiting_on": "machine.slot: 2 of 2 held"'; then
    seen=1; break
  fi
  i=$((i + 1)); sleep 0.25
done
[ -n "$seen" ] || { echo "    a waiter was never listed with what it waits on: $W"; exit 1; }
wait "$CPID"
took=$(( $(date +%s) - start ))
[ "$(cat "$T/c.rc")" = 75 ] || { echo "    the third acquisition ended $(cat "$T/c.rc"), not 75: $(cat "$T/c.out")"; exit 1; }
[ "$took" -ge 5 ] && [ "$took" -le 60 ] || { echo "    a wait bounded at 5s took ${took}s"; exit 1; }
grep -q "^refused (slots_full)" "$T/c.out" || { echo "    the refusal is not typed: $(cat "$T/c.out")"; exit 1; }
grep -q "pid $A (build a" "$T/c.out" && grep -q "pid $B (build b" "$T/c.out" \
  || { echo "    the refusal does not name both holders: $(cat "$T/c.out")"; exit 1; }
grep -q 'project.the-cooperation-model-is-the-owners-word' "$T/c.out" \
  || { echo "    the refusal does not name the rule: $(cat "$T/c.out")"; exit 1; }
grep -q 'waiting for a machine slot' "$T/c.err" \
  || { echo "    the wait said nothing on stderr: $(cat "$T/c.err")"; exit 1; }
"$RB" machine slot status --format json | grep -q '"waiters": \[\]' \
  || { echo "    a refused waiter stayed in the queue"; exit 1; }

# ---------------------------------------------------------------- a dead holder is reclaimed
kill "$A"; wait "$A" 2>/dev/null || true
"$RB" machine slot status | grep -q "pid $A .*dead" \
  || { echo "    a dead holder is not reported dead: $("$RB" machine slot status)"; exit 1; }
acquire "$C" "build c"
[ "$RC" = 0 ] || { echo "    the dead holder's slot was not given to the next: $RC $OUT $ERR"; exit 1; }
printf '%s\n' "$OUT" | grep -q "reclaimed: pid $A (build a)" \
  || { echo "    the reclaim is not named: $OUT"; exit 1; }

# ---------------------------------------------------------------- the disk floor refuses at once
policy 999999999 30
start="$(date +%s)"
acquire "$D" "build d"
took=$(( $(date +%s) - start ))
[ "$RC" = 75 ] || { echo "    below the floor the acquisition ended $RC: $OUT"; exit 1; }
[ "$took" -le 20 ] || { echo "    a disk refusal waited ${took}s"; exit 1; }
printf '%s\n' "$OUT" | grep -q '^refused (disk_floor): .*MiB) free on the volume of .*under the floor of 999999999 GiB' \
  || { echo "    the disk refusal does not name the measurement and the floor: $OUT"; exit 1; }
printf '%s\n' "$OUT" | grep -q "pid $B (build b" || { echo "    the disk refusal does not name the holders: $OUT"; exit 1; }

# ---------------------------------------------------------------- the cargo build path asks first
policy 0 2
FAKE="$T/fake"; mkdir -p "$FAKE"; CALLS="$T/calls.log"; : > "$CALLS"
cat > "$FAKE/cargo" <<'SH'
#!/bin/sh
[ "$1" = metadata ] && exit 1
printf 'cargo %s\n' "$*" >> "$FAKE_CALLS"
exit 0
SH
chmod +x "$FAKE/cargo"
build() { # sets RC, ERR
  RC=0
  ERR="$(env -u CI -u MAJORDOMUS_MACHINE_SLOT PATH="$FAKE:$PATH" FAKE_CALLS="$CALLS" MAJORDOMUS_BIN="$RB" \
        CARGO_HOME="$T/cargo-home" MAJORDOMUS_MIN_FREE_MB=0 MAJORDOMUS_INCREMENTAL_IDLE_MIN=0 "$@" \
        sh -c '. "$1"; mj_cargo_bounded "$2" build --quiet --locked' sh "$ROOT/lib/rust_bin.sh" "$T" 2>&1 >/dev/null)" || RC=$?
}
build
[ "$RC" = 75 ] || { echo "    a build on a full machine ended $RC, not 75: $ERR"; exit 1; }
[ ! -s "$CALLS" ] || { echo "    cargo ran although no slot was given: $(cat "$CALLS")"; exit 1; }
printf '%s\n' "$ERR" | grep -q "pid $B (build b" \
  || { echo "    the refused build does not name the holders: $ERR"; exit 1; }
printf '%s\n' "$ERR" | grep -q 'MAJORDOMUS_MACHINE_SLOT=off' || { echo "    the refused build names no override: $ERR"; exit 1; }

"$RB" machine slot release --pid "$B" | grep -q "^released" || { echo "    the release was not said"; exit 1; }
build
[ "$RC" = 0 ] || { echo "    with a slot free the build ended $RC: $ERR"; exit 1; }
grep -q '^cargo build' "$CALLS" || { echo "    cargo did not run once admitted: $ERR"; exit 1; }
"$RB" machine slot status --format json | grep -q '"held": 1' \
  || { echo "    the build kept its slot after cargo ended: $("$RB" machine slot status)"; exit 1; }

: > "$CALLS"
build MAJORDOMUS_MACHINE_SLOT=off
printf '%s\n' "$ERR" | grep -q 'building without a machine slot (MAJORDOMUS_MACHINE_SLOT=off)' \
  || { echo "    the override is silent: $ERR"; exit 1; }

# ---------------------------------------------------------------- the same answers over MCP
command -v jq >/dev/null 2>&1 || skip_case "jq is absent, so the MCP half was not measured"
mcp_call() { # <tool> <arguments json>: the tool's typed answer
  { printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case4006","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2"
  } | "$RB" mcp --standalone --repo "$PWD" 2>"$T/mcp.err" | sed -n 2p | jq -c '.result.structuredContent // empty'
}
holder; E=$LAST
a="$(mcp_call majordomus_machine_slot_acquire "{\"pid\":$E,\"purpose\":\"build e\",\"wait_seconds\":0}")"
[ "$(printf '%s' "$a" | jq -r .verdict)" = admitted ] \
  || { echo "    an acquisition over MCP was not admitted: $a $(cat "$T/mcp.err")"; exit 1; }
st="$(mcp_call majordomus_machine_slot_status '{}')"
printf '%s' "$st" | jq -e --argjson e "$E" '.held == 2 and any(.holders[]; .pid == $e and .purpose == "build e")' >/dev/null \
  || { echo "    the status over MCP does not list the holder: $st"; exit 1; }
r="$(mcp_call majordomus_machine_slot_release "{\"pid\":$E}")"
[ "$(printf '%s' "$r" | jq -r .released)" = true ] || { echo "    the release over MCP was not done: $r"; exit 1; }

echo "    N holders admitted, the next waits its bound visibly and is refused naming them, a dead holder is reclaimed, the disk floor refuses, and a build asks first"

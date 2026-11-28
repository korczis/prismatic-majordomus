# majordomus-covers: none
# majordomus-timeout: 300
# One command lists every Majordomus runtime of this user on this host, with no checkout open
# (I3184; intent nothing-in-the-mesh-waits-on-something-dead, criterion host-lists-its-runtimes).
#
# Two servers of two repositories run side by side. The first writes its root into the
# reader's host index (`checkout-roots`, beside node.json in $XDG_STATE_HOME/majordomus); the
# second runs with a state directory of its own, so it never writes to the reader's index —
# the shape of a runtime older than the index. `majordomus runtimes --host` is then asked
# from a directory inside no repository:
#
#   both servers are listed, live, with version, repository identity, checkout and port
#   the one that never wrote to the index is found by the process table alone
#   the index deleted, both are still listed: it is a cache and loses nothing but speed
#   the first server killed with SIGKILL, its index entry is listed stale and never live
#   the reader wrote nothing: no index, no node identity, nothing in either repository
#   (asked once more with a state directory of its own, which stays empty)
#
# Every process this case starts is stopped by the pid this case started, never by a name.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
S="$(mktemp -d "${TMPDIR:-/tmp}/mj4665.XXXXXX")"
S="$(cd "$S" && pwd -P)"
PA=""; PB=""
trap 'for p in "$PA" "$PB"; do [ -n "$p" ] && kill "$p" 2>/dev/null; done; rm -rf "$S"' EXIT

A="$(pwd -P)"
B="$S/second"
mkdir -p "$B" "$S/outside" "$S/reader" "$S/own"
( cd "$B" && git init -q . && git config user.email t@example.com && git config user.name t \
  && git commit -q --allow-empty -m init )
"$MJ" init >/dev/null
( cd "$B" && "$MJ" init >/dev/null )
before_a="$(git -C "$A" status --porcelain)"
before_b="$(git -C "$B" status --porcelain)"

# wait_url DIR: the address the lease of the checkout at DIR publishes, within 30 s
wait_url() {
  local lease="$1/.ai/local/state/mcp/server.json" i=0 url=""
  while :; do
    url="$(jq -r '.url // empty' "$lease" 2>/dev/null || true)"
    [ -z "$url" ] || { printf '%s\n' "$url"; return 0; }
    i=$((i+1)); [ "$i" -lt 300 ] || { echo "    no server published a lease at $1" >&2; return 1; }
    sleep 0.1
  done
}
XDG_STATE_HOME="$S/reader" "$RB" serve --repo "$A" --port 0 > "$S/a.out" 2> "$S/a.err" & PA=$!
XDG_STATE_HOME="$S/own" "$RB" serve --repo "$B" --port 0 > "$S/b.out" 2> "$S/b.err" & PB=$!
url_a="$(wait_url "$A")" || { cat "$S/a.err"; exit 1; }
url_b="$(wait_url "$B")" || { cat "$S/b.err"; exit 1; }
index="$S/reader/majordomus/checkout-roots"
grep -qxF "$A" "$index" || { echo "    the first server did not add its root to the index"; cat "$index"; exit 1; }
if grep -qxF "$B" "$index"; then echo "    the second server wrote to the reader's index"; exit 1; fi

ask() { ( cd "$S/outside" && XDG_STATE_HOME="$S/reader" "$RB" runtimes --host --format json ) > "$S/$1.json" \
  || { echo "    runtimes --host failed"; exit 1; }; }

# ---------------------------------------------------------------- both, from outside every repository
ask both
jq -e --arg a "$A" --arg b "$B" --arg ua "${url_a##*:}" --arg ub "${url_b##*:}" --arg pa "$PA" --arg pb "$PB" '
  def row($c): .runtimes[] | select(.checkout == $c);
  (row($a) | .live and .standing == "ready" and (.port | tostring) == $ua and .version != null
     and .checkout_id != null and (.found_by | index("index")) != null
     and any(.processes[]; (.pid | tostring) == $pa and .holds_lease))
  and (row($b) | .live and (.port | tostring) == $ub and .version != null and .checkout_id != null
     and .found_by == ["process_table"]
     and any(.processes[]; (.pid | tostring) == $pb and .holds_lease))
  and (row($a).checkout_id != row($b).checkout_id)
' "$S/both.json" >/dev/null || { echo "    the two runtimes are not both listed as live:"; cat "$S/both.json"; exit 1; }
expect_exit 0 sh -c "cd '$S/outside' && XDG_STATE_HOME='$S/reader' '$RB' runtimes --host"
expect_grep "^ready .* $A\$"
expect_grep "process_table +$B\$"

# ---------------------------------------------------------------- the index is a cache
mv "$index" "$S/index.kept"
ask uncached
jq -e --arg a "$A" --arg b "$B" '
  ([.runtimes[] | select(.checkout == $a or .checkout == $b) | select(.live)] | length) == 2
  and .index.entries == 0
' "$S/uncached.json" >/dev/null || { echo "    deleting the index lost a runtime:"; cat "$S/uncached.json"; exit 1; }
mv "$S/index.kept" "$index"

# ---------------------------------------------------------------- an entry whose process is gone
kill -9 "$PA"; wait "$PA" 2>/dev/null || true
i=0; while kill -0 "$PA" 2>/dev/null; do i=$((i+1)); [ "$i" -lt 100 ] || { echo "    $PA survived SIGKILL"; exit 1; }; sleep 0.1; done
PA=""
ask stale
jq -e --arg a "$A" --arg b "$B" '
  (.runtimes[] | select(.checkout == $a) | .standing == "stale" and (.live | not) and .found_by == ["index"])
  and (.runtimes[] | select(.checkout == $b) | .live)
' "$S/stale.json" >/dev/null || { echo "    a dead server's entry is not listed stale:"; cat "$S/stale.json"; exit 1; }

# ---------------------------------------------------------------- the reader wrote nothing
mkdir -p "$S/fresh"
( cd "$S/outside" && XDG_STATE_HOME="$S/fresh" "$RB" runtimes --host --format json ) > "$S/fresh.json" \
  || { echo "    runtimes --host failed with an empty state directory"; exit 1; }
jq -e --arg b "$B" '.index.entries == 0 and any(.runtimes[]; .checkout == $b and .live)' "$S/fresh.json" >/dev/null \
  || { echo "    with no index the process table did not find the live server:"; cat "$S/fresh.json"; exit 1; }
[ -z "$(ls -A "$S/fresh")" ] || { echo "    the reader wrote into its state directory"; ls -AR "$S/fresh"; exit 1; }
[ -z "$(ls -A "$S/outside")" ] || { echo "    the reader wrote where it was asked from"; ls -A "$S/outside"; exit 1; }
kill "$PB" 2>/dev/null || true; wait "$PB" 2>/dev/null || true; PB=""
[ "$(git -C "$A" status --porcelain)" = "$before_a" ] || { echo "    the first repository changed"; git -C "$A" status --porcelain; exit 1; }
[ "$(git -C "$B" status --porcelain)" = "$before_b" ] || { echo "    the second repository changed"; git -C "$B" status --porcelain; exit 1; }
echo "    two runtimes of two repositories listed from outside both; the unindexed one by the process table; a dead one stale"

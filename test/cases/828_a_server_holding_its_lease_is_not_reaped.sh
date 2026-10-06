# majordomus-covers: none
# A server that holds its checkout's lease is not an abandoned server.
#
# `scripts/reap-orphans` called a server abandoned when its parent was init, no TCP connection
# was established and it was older than --min-age. The shared server meets all three by
# design: `serve` detaches, its clients speak HTTP a request at a time, and it lives for days.
# On 2026-10-03 the report tabled pid 48947 on :57661 — the server five sessions had used
# within the minute — as "would reap", the same misreading case 318 records for 2026-09-13.
#
# A listening server is now asked. One that answers `"leaseholder": true` is kept; one that
# answers false (superseded by another) is a candidate; one that does not answer is kept,
# because what cannot be measured is not reaped. A server that listens nowhere is decided by
# the old conditions alone.
#
# The case is hermetic. `ps`, `lsof` and `curl` are shims describing four servers that do not
# exist, and the reaper runs in report mode only: it never signals anything, and no process on
# the machine running the case is in its subject.
. "$ROOT/test/lib.sh"

REAPER="$ROOT/scripts/reap-orphans"
expect_file "$REAPER"

mkdir -p "$T/bin"
# pid ppid etime rss args — three listening servers, one silent, one with a live parent
cat > "$T/bin/ps" <<'SHIM'
#!/bin/sh
case " $* " in
  *" -eo "*)
    echo "990001 1 01:00:00 20480 /srv/a/apps/majordomus-cli/target/debug/majordomus serve --port 8741"
    echo "990002 1 01:00:00 20480 /srv/b/apps/majordomus-cli/target/debug/majordomus serve --port 8741"
    echo "990003 1 01:00:00 20480 /srv/c/apps/majordomus-cli/target/debug/majordomus serve --port 8741"
    echo "990004 1 01:00:00 20480 /srv/d/apps/majordomus-cli/target/debug/majordomus mcp"
    echo "990005 4242 01:00:00 20480 /srv/e/apps/majordomus-cli/target/debug/majordomus serve" ;;
  *) exit 1 ;;
esac
SHIM
cat > "$T/bin/lsof" <<'SHIM'
#!/bin/sh
pid=""; state=""
while [ $# -gt 0 ]; do
  case "$1" in
    -p) pid="$2"; shift 2 ;;
    -sTCP:*) state="${1#-sTCP:}"; shift ;;
    *) shift ;;
  esac
done
[ "$state" = LISTEN ] || exit 1
case "$pid" in
  990001) port=41001 ;; 990002) port=41002 ;; 990003) port=41003 ;; *) exit 1 ;;
esac
echo "COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME"
echo "majordomu $pid t 9u IPv4 0x0 0t0 TCP 127.0.0.1:$port (LISTEN)"
SHIM
cat > "$T/bin/curl" <<'SHIM'
#!/bin/sh
for a in "$@"; do url="$a"; done
case "$url" in
  *:41001/) echo '{"version":"0.12.0","leaseholder":true}' ;;
  *:41002/) echo '{"version":"0.12.0","leaseholder": false}' ;;
  *) exit 7 ;;
esac
SHIM
chmod +x "$T/bin/ps" "$T/bin/lsof" "$T/bin/curl"

rc=0
PATH="$T/bin:$PATH" "$REAPER" --servers --min-age 0 > "$T/report.txt" 2>&1 || rc=$?
LAST_OUT="$(cat "$T/report.txt")"
[ "$rc" = 10 ] || { echo "    the report exited $rc, not 10 (orphans found):"; sed 's/^/      /' "$T/report.txt"; exit 1; }

echo "    the leaseholder is kept, whatever its parent and connections"
expect_grep "^990001 .*keep — holds its checkout's lease"
expect_no_grep '^990001 .*would reap'

echo "    a server that answers it is superseded is a candidate"
expect_grep '^990002 .*would reap'

echo "    a server that listens and does not answer is kept"
expect_grep '^990003 .*keep — listens but did not say'

echo "    a server that listens nowhere is decided by the old conditions"
expect_grep '^990004 .*would reap'

echo "    a server whose parent is alive is never a candidate"
expect_no_grep '^990005'

expect_grep 'servers — 2 abandoned, 2 attached and kept'

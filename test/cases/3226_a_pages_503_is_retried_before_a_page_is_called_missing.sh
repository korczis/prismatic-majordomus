# majordomus-covers: none
# A page of the published UI that answers 503, or does not answer at all (000), is asked again
# before it is called missing (I2813). GitHub Pages answers a burst of requests with a transient 503 that a reader a second
# later does not meet; on 2026-10-17 that one answer reddened master's validate and 71a's run,
# because `scripts/pages verify-ui` declared a page absent on its first 503. Held here with a
# stand-in site behind a server of this case's own, which answers 503 to one page a set number
# of times before it serves it:
#
#   transient   a page that answers 503 twice and then 200 passes, and the probe says
#               "retried /…: 503 ×2 then 200"
#   persistent  a page that answers 503 on every try fails, as it did, and the probe says it
#               was retried and is still 503: a bounded number of tries, never a wait forever
#   no answer   a page whose connection is closed unanswered once (curl's 000) and then served
#               passes, and the probe says "retried /…: 000 ×1 then 200"; one that is never
#               answered fails, still 000, after the same bounded number of tries. On
#               2026-10-17 at 23:49Z five cockpit pages read 000 were called "not served" while
#               each answered 200 a minute later
#   other       a listed page that answers 404 fails at once and is never retried: only a 503
#               or a 000 is a reason to ask again
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
command -v curl >/dev/null 2>&1 || skip "curl is not installed"
command -v python3 >/dev/null 2>&1 || skip "python3 is not installed"

S="$(mktemp -d "${TMPDIR:-/tmp}/mj3221.XXXXXX")"
SRV=""
trap '[ -n "$SRV" ] && kill "$SRV" 2>/dev/null; rm -rf "$S"' EXIT
pages() { env -u MAJORDOMUS_ROOT -u MAJORDOMUS_SHARE MJ_PAGES_RETRY_SECONDS=0 "$ROOT/scripts/pages" "$@"; }

# the stand-in publication: a site that names commit A, and the UI's snapshot read at A
A=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
W="$S/www"; mkdir -p "$W"
jq -n --arg c "$A" '{schema: 1, commit: $c, dirty: false}' > "$W/build.json"
printf '<html><body>site</body></html>\n' > "$W/index.html"
publication_snapshots "$ROOT" "$W" "$A" || { echo "    the stand-in snapshot could not be placed"; exit 1; }
mount="$(pages snapshots | awk -F '\t' '$1 == "ui" {print $2}')"
[ -n "$mount" ] && [ -f "$W$mount/one/index.html" ] || { echo "    the stand-in holds no page one/index.html under the UI's mount"; exit 1; }
FLAKY="$mount/one/index.html"

# A server for that directory that fails $FLAKY the number of times the file `rule` says (-1:
# always), then serves it as it is: with a 503, or, when the rule names 000, by closing the
# connection without an answer. Every request it answers is counted per path.
cat > "$S/serve.py" <<'PY'
import http.server, os, socket, sys, threading
root, port_file, rule_file, flaky = sys.argv[1:5]
count = {}
lock = threading.Lock()
class H(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k):
        super().__init__(*a, directory=root, **k)
    def log_message(self, *a):
        pass
    def do_GET(self):
        path = self.path.split('?')[0]
        with lock:
            count[path] = count.get(path, 0) + 1
            n = count[path]
        rule = (open(rule_file).read().split() or ['0'])
        limit, kind = int(rule[0]), (rule[1] if len(rule) > 1 else '503')
        if path == flaky and (limit < 0 or n <= limit):
            if kind == '000':
                self.close_connection = True
                self.connection.shutdown(socket.SHUT_RDWR)
                return
            self.send_response(503); self.send_header('Content-Length', '0'); self.end_headers(); return
        return super().do_GET()
server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), H)
open(port_file, 'w').write(str(server.server_address[1]))
server.serve_forever()
PY
serve() {   # <how many times the flaky page fails first; -1 for always> [503|000]
  [ -n "$SRV" ] && { kill "$SRV" 2>/dev/null; wait "$SRV" 2>/dev/null || true; }
  printf '%s %s\n' "$1" "${2:-503}" > "$S/rule"; rm -f "$S/port"
  python3 "$S/serve.py" "$W" "$S/port" "$S/rule" "$FLAKY" & SRV=$!
  local i=0
  while [ ! -s "$S/port" ]; do i=$((i+1)); [ "$i" -lt 100 ] || { echo "    the stub server did not start"; exit 1; }; sleep 0.1; done
  BASE="http://127.0.0.1:$(cat "$S/port")"
}
probe() { pages verify-ui --url "$BASE" --commit "$A" --timeout 0 > "$S/out" 2> "$S/err"; }

# ---------------------------------------------------------------- transient
serve 2
rc=0; probe || rc=$?
[ "$rc" = 0 ] || { echo "    a page that answered 503 twice and then 200 failed the probe (exit $rc):"; sed 's/^/      /' "$S/err" | head -12; exit 1; }
grep -qF "retried /one/index.html: 503 ×2 then 200" "$S/err" \
  || { echo "    the probe passed without saying the page was retried:"; sed 's/^/      /' "$S/err" | head -8; exit 1; }
echo "    transient: a 503 twice then 200 passes, and the retry is said"

# ---------------------------------------------------------------- persistent
serve -1
rc=0; probe || rc=$?
[ "$rc" = 10 ] || { echo "    a page that always answers 503 did not fail the probe as a finding (exit $rc, want 10)"; sed 's/^/      /' "$S/err" | head -8; exit 1; }
grep -qE "retried /one/index.html: 503 ×[0-9]+, still 503 after [0-9]+ retries" "$S/err" \
  || { echo "    a persistent 503 is not said to have been retried:"; sed 's/^/      /' "$S/err" | head -8; exit 1; }
grep -qF "one/index.html answered 503" "$S/err" \
  || { echo "    a persistent 503 is not named as the page's status:"; sed 's/^/      /' "$S/err" | head -8; exit 1; }
echo "    persistent: a 503 on every try fails as it did, after a bounded number of retries"

# ---------------------------------------------------------------- no answer
serve 1 000
rc=0; probe || rc=$?
[ "$rc" = 0 ] || { echo "    a page unanswered once and then served failed the probe (exit $rc):"; sed 's/^/      /' "$S/err" | head -12; exit 1; }
grep -qF "retried /one/index.html: 000 ×1 then 200" "$S/err" \
  || { echo "    the probe passed without saying the unanswered page was asked again:"; sed 's/^/      /' "$S/err" | head -8; exit 1; }
serve -1 000
rc=0; probe || rc=$?
[ "$rc" = 10 ] || { echo "    a page never answered did not fail the probe as a finding (exit $rc, want 10)"; sed 's/^/      /' "$S/err" | head -8; exit 1; }
grep -qE "retried /one/index.html: 000 ×[0-9]+, still 000 after [0-9]+ retries" "$S/err" \
  || { echo "    a page never answered is not said to have been retried:"; sed 's/^/      /' "$S/err" | head -8; exit 1; }
echo "    no answer: a 000 once then 200 passes and is said; a 000 on every try fails after the same bound"

# ---------------------------------------------------------------- other
serve 0
rm -f "$W$FLAKY"
rc=0; probe || rc=$?
[ "$rc" = 10 ] || { echo "    a listed page that answers 404 did not fail the probe (exit $rc, want 10)"; exit 1; }
grep -q 'retried' "$S/err" && { echo "    a 404 was retried: only a 503 or a 000 is a reason to ask again"; sed 's/^/      /' "$S/err" | head -6; exit 1; }
grep -qF "one/index.html answered 404" "$S/err" \
  || { echo "    the 404 is not named as the page's status:"; sed 's/^/      /' "$S/err" | head -6; exit 1; }
echo "    other: a 404 fails at once and is not retried"

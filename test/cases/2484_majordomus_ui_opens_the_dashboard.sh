# majordomus-covers: none
# `majordomus ui` reaches the primary surface in one step (I2484, ADR 0135): with no server it
# starts one and prints an address ending in /ui that answers 200; with a server running it
# prints that server's address and starts no second one; a screen SCREENS holds is its route,
# and an unknown screen is refused by name, with the screens listed, before anything starts.
#
# Every call passes --no-open: a case must never open a browser on the machine it runs on.
. "$ROOT/test/lib.sh"
command -v curl >/dev/null 2>&1 || skip "curl absent"
RB="$(rust_bin)" || rust_bin_exit $?
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
trap '"$RB" serve stop --repo "$T" >/dev/null 2>&1 || true' EXIT
lease=.ai/local/state/mcp/server.json

# --- an unknown screen is refused before anything is started
[ ! -f "$lease" ] || { echo "    a fresh repository has a lease before anything started"; exit 1; }
expect_exit 2 "$RB" ui nowhere --no-open --repo "$T"
expect_grep 'no screen is called `nowhere`'
expect_grep 'roadmap'
[ ! -f "$lease" ] || { echo "    a refused screen started a server"; exit 1; }

# --- no server: one is started, and the address it prints is /ui and answers
url="$("$RB" ui --no-open --idle 60 --repo "$T" 2>"$T/err")"; code=$?
[ "$code" = 0 ] || { echo "    ui exited $code"; cat "$T/err"; exit 1; }
case "$url" in http://*/ui) ;; *) echo "    the address does not end in /ui: '$url'"; cat "$T/err"; exit 1 ;; esac
[ -f "$lease" ] || { echo "    no lease after ui"; exit 1; }
status="$(curl -s -m 30 -o /dev/null -w '%{http_code}' "$url")"
[ "$status" = 200 ] || { echo "    $url answers $status"; exit 1; }

# --- a running server is used: the same address, the same lease, and nothing started
before="$(cat "$lease")"
"$RB" ui --no-open --format json --repo "$T" > "$T/again.json" 2>/dev/null
grep -q '"started": false' "$T/again.json" || { echo "    the second call started a server:"; cat "$T/again.json"; exit 1; }
grep -q "\"url\": \"$url\"" "$T/again.json" || { echo "    the second call names another address:"; cat "$T/again.json"; exit 1; }
[ "$(cat "$lease")" = "$before" ] || { echo "    the lease changed under a second call"; exit 1; }

# --- a named screen is its route, and answers
roadmap="$("$RB" ui roadmap --no-open --repo "$T" 2>/dev/null)"
[ "$roadmap" = "$url/roadmap" ] || { echo "    ui roadmap printed '$roadmap'"; exit 1; }
status="$(curl -s -m 30 -o /dev/null -w '%{http_code}' "$roadmap")"
[ "$status" = 200 ] || { echo "    $roadmap answers $status"; exit 1; }

echo "    majordomus ui ensures one server, prints /ui or a screen's route, and refuses an unknown screen first"

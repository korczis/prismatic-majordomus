# majordomus-covers: none
# A server names the share it serves, and says when that share is not its own.
#
# On 2026-10-14 the shared server of the primary checkout ran the installed 0.26.0 executable
# over the primary's 0.21.0 `share/`, which has no `ui/`: every `/ui/assets/` file answered
# 404, the kiosk was blank, and nothing but a WARN line in a log nobody read said why. The
# reports said `ready`. This case holds the reporting half of the remedy: the lease names the
# share directory the server located, and `server.status` — through `serve status` and
# `GET /api/v1/server` alike — measures that directory on every call against the server's
# version: the version its `version.txt` states, whether it holds `ui/`, and `skewed` with
# the reason when either is wrong.
#
# Reporting only: the server keeps serving a skewed share and stays `ready`. Which share a
# server should pick, and whether a skewed one is replaced, are decisions of their own.
#
# `--port 0` throughout: the default port belongs to whatever already serves this machine.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "jq not installed"
command -v curl >/dev/null 2>&1 || skip "curl not installed"

mkdir -p "$T/wt"; W="$(cd "$T/wt" && pwd -P)"; R="$W/repo"
mkdir -p "$R"
git -C "$R" init -q .
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
( cd "$R" && "$MJ" init >/dev/null )
git -C "$R" add -A >/dev/null && git -C "$R" commit -qm base >/dev/null
MAJORDOMUS_LOG=warn; export MAJORDOMUS_LOG
trap '"$RB" serve stop --repo "$R" >/dev/null 2>&1 || true' EXIT
mj() { ( cd "$R" && "$RB" "$@" ); }
version="$("$RB" --version | awk '{print $NF}')"
[ -n "$version" ] || { echo "    the executable states no version"; exit 1; }

start() {   # start SHARE — a server for the fixture over that share, and its address
  MAJORDOMUS_SHARE="$1" mj serve ensure --port 0 --idle 60 --wait 40 --format json \
    > "$T/ensure.json" 2> "$T/ensure.err" \
    || { echo "    the server did not come up over $1:"; cat "$T/ensure.json" "$T/ensure.err"; exit 1; }
  url="$(jq -r '.url' "$T/ensure.json")"
  [ -n "$url" ] && [ "$url" != null ] || { echo "    no address"; cat "$T/ensure.json"; exit 1; }
}
share_of() {   # share_of NAME — this checkout's served share, through the command line
  mj serve status --format json > "$T/$1.json" \
    || { echo "    serve status --format json failed ($1)"; exit 1; }
  jq '.servers[] | select(.this_checkout) | .lease.share' "$T/$1.json" > "$T/$1.share.json"
}

# ---------------------------------------------------------------- 1. its own share: current
own="$(cd "$ROOT/share" && pwd -P)"
start "$own"
share_of own
jq -e --arg v "$version" --arg p "$own" '.standing == "current" and .version == $v
  and .ui_present == true and .path == $p and .origin == "MAJORDOMUS_SHARE"
  and (has("reason") | not)' "$T/own.share.json" >/dev/null \
  || { echo "    the server's own share is not reported current:"; cat "$T/own.json"; exit 1; }
expect_exit 0 mj serve status
expect_grep "share current  version $version  ui present"
expect_exit 0 mj serve stop

# ---------------------------------------------------------------- 2. a share of another release, without ui/
# what a stale checkout's share is to a newer executable: another version, and no ui/
old="$T/old-share"
cp -R "$ROOT/share" "$old"
old="$(cd "$old" && pwd -P)"
rm -rf "$old/ui"
printf '# planted by case 3041\nversion=0.0.1\n' > "$old/version.txt"
start "$old"
share_of old
jq -e --arg p "$old" '.standing == "skewed" and .version == "0.0.1" and .ui_present == false
  and .path == $p
  and (.reason | contains("states version 0.0.1") and contains("holds no ui/"))' \
  "$T/old.share.json" >/dev/null \
  || { echo "    a share of another release without ui/ is not reported skewed:"; cat "$T/old.json"; exit 1; }
# reporting only: the server serves it and stays ready
jq -e '.standing == "ready"' "$T/old.json" >/dev/null \
  || { echo "    the report changed the server's standing:"; cat "$T/old.json"; exit 1; }
curl -fsS -o /dev/null -w '%{http_code}' -m 10 "$url/ui/assets/ui.css" > "$T/css.code" 2>/dev/null || true
[ "$(cat "$T/css.code")" = 404 ] \
  || { echo "    the skew this case reports is not the one it plants (ui.css answered $(cat "$T/css.code"))"; exit 1; }
expect_exit 0 mj serve status
expect_grep 'share skewed  version 0.0.1  ui absent'
expect_grep 'holds no ui/'
# and the HTTP surface says the same thing about the share
curl -fsS -m 30 "$url/api/v1/server" \
  | jq -S '.servers[] | select(.this_checkout) | .lease.share' > "$T/http.share.json" \
  || { echo "    GET /api/v1/server failed"; exit 1; }
diff -u "$T/http.share.json" <(jq -S . "$T/old.share.json") > "$T/agree.diff" \
  || { echo "    the two surfaces disagree about the share:"; cat "$T/agree.diff"; exit 1; }
# the server said it once, where a person reading why the UI is blank looks first
log="$(jq -r '.log' "$T/ensure.json")"
grep -q 'holds no ui/' "$log" \
  || { echo "    the server's log does not say its share is skewed:"; tail -20 "$log"; exit 1; }

# ---------------------------------------------------------------- 3. measured when asked, not when started
# the same running server, its share repaired under it: the next reading says so
mkdir -p "$old/ui"
printf 'version=%s\n' "$version" > "$old/version.txt"
share_of mended
jq -e '.standing == "current" and .ui_present == true' "$T/mended.share.json" >/dev/null \
  || { echo "    the share was read when the server started, not when it was asked:"; cat "$T/mended.json"; exit 1; }
expect_exit 0 mj serve stop

echo "    a server names its share, and a share of another release or without ui/ is reported skewed, by both surfaces, while it serves"

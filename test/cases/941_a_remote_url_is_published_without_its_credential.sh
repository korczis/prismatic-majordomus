# majordomus-covers: none
# majordomus-timeout: 600
# A remote's URL is published without its credential.
#
# `origin` configured as https://<user>:<token>@host/o/r.git is how a CI clone and many a
# laptop are configured. The shell tool took that string as the repository's identity and
# wrote it into every shared record — and a session record is tracked and pushed — and the
# Rust executable answered it from `session_domain.identity` to any MCP or HTTP client.
#
#   1. the function: over HTTP the whole userinfo goes, over ssh only a password, the scp
#      form and a URL with nothing to remove are left as they are
#   2. a tracked session record, a handover and the archive manifest written under such a
#      remote hold the URL and not the token, and so does nothing else in the tree
#   3. a record written before the fix, carrying the credentialed URL, is still recognised
#      as this repository's
#   4. `session_domain.identity` answers the URL without the token
. "$ROOT/test/lib.sh"
TOKEN="mjcase941token0001"
CLEAN="https://example.invalid/o/r.git"
DIRTY="https://x-access-token:$TOKEN@example.invalid/o/r.git"

# --- 1. the function, as the tool itself loads it
pub() { bash -c '. "$1/lib/common.sh" >/dev/null 2>&1; mj_url_public "$2"' _ "$ROOT" "$1"; }
is() { [ "$(pub "$1")" = "$2" ] || { echo "    mj_url_public $1 gave $(pub "$1"), expected $2"; exit 1; }; }
is "$DIRTY" "$CLEAN"
is "https://ghp_onlyauser@example.invalid/o/r.git" "$CLEAN"
is "http://u:p@example.invalid:8080/o/r.git" "http://example.invalid:8080/o/r.git"
is "ssh://git:hunter2@example.invalid:22/o/r.git" "ssh://git@example.invalid:22/o/r.git"
is "ssh://git@example.invalid/o/r.git" "ssh://git@example.invalid/o/r.git"
is "git@example.invalid:o/r.git" "git@example.invalid:o/r.git"
is "$CLEAN" "$CLEAN"
is "https://example.invalid/o/r@v1.git" "https://example.invalid/o/r@v1.git"

# --- 2. the records a worker writes under a credentialed remote
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
git remote add origin "$DIRTY"

expect_exit 0 "$MJ" session start --worker case/941
"$MJ" start "t1" --scope lib >/dev/null
echo b >> lib/a
expect_exit 0 bash -c "printf '# Objective\nx\n# Current State\ny\n# Next Action\nz\n' | '$MJ' handover"
handover="$LAST_OUT"; [ -f "$handover" ] || { echo "    no handover record: $handover"; exit 1; }
expect_exit 0 bash -c "printf 'The remote carried a credential and the record does not.\n' | '$MJ' session close"
# the record's path is the last line; what was derived at the close is said before it
session="$(printf '%s\n' "$LAST_OUT" | tail -n 1)"; [ -f "$session" ] || { echo "    no session record: $session"; exit 1; }
# the session record is the shared one: it is tracked material, and it names the repository
case "$session" in .ai/repo/sessions/*) ;; *) echo "    the session record is not in the shared section: $session"; exit 1 ;; esac
grep -qF "$CLEAN" "$session" || { echo "    the session record does not name the repository by its URL:"; cat "$session"; exit 1; }

# nothing the tool wrote holds the token: the working tree and the layer's local half, with
# git's own config — where the credential was put, and where it belongs — left out
leaked="$(grep -rlF "$TOKEN" . --exclude-dir=.git 2>/dev/null || true)"
[ -z "$leaked" ] || { echo "    the token was written into:"; echo "$leaked"; exit 1; }
# and the check can see: the same sweep finds the URL without it
grep -rlF "$CLEAN" . --exclude-dir=.git >/dev/null 2>&1 || { echo "    the sweep found no record naming the repository at all"; exit 1; }

# --- 3. a record from before: the credentialed URL as its repository id still resolves
expect_exit 0 "$MJ" session list
expect_grep "$(basename "$session" | sed 's/^[^-]*--\(s-[^-]*-[^-]*\)--.*/\1/')"
old="$(dirname "$session")/$(basename "$session" .md)-old.keep"
sed "s|$CLEAN|$DIRTY|" "$session" > "$old"
grep -qF "$TOKEN" "$old" || { echo "    could not build the pre-fix record"; exit 1; }
# the comparison the listing makes, on the value such a record carries
[ "$(pub "$DIRTY")" = "$(bash -c '. "$1/lib/common.sh" >/dev/null 2>&1; MJ_ROOT="$PWD"; mj_repository_id' _ "$ROOT")" ] \
  || { echo "    a pre-fix repository id no longer matches this repository"; exit 1; }
rm -f "$old"

# --- 4. the executable answers the same URL
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
expect_exit 0 "$RB" run session_domain.identity --format json
expect_no_grep "$TOKEN"
expect_grep "$CLEAN"

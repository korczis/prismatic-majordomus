# majordomus-covers: none
# An installed executable reads its own share: a skewed share found by convention is not
# served when the executable's own is beside it (I2609).
#
# On 2026-10-13 an installed 0.22.0 server, started from a tmux window that had inherited
# MAJORDOMUS_SHARE, read a 0.21.0 checkout's share. On 2026-10-14 the installed 0.26.0
# executable, started from the primary checkout's shell,
# served that checkout's 0.21.0 `share/` because `MAJORDOMUS_SHARE` named it, and every
# `/ui/assets/` file answered 404. ADR 0163 point 2: `--share` is used as given, but a share
# found through `MAJORDOMUS_SHARE` or the repository's `share/` that states another version
# than the executable, or holds no `ui/`, is passed over for a later one that matches — the
# installation's `share/` beside the executable — and the share used says which it passed
# over and why. When nothing matches, the first is served as before (case 3041 reports it).
#
# The installation is laid out here as the installer lays it out: `bin/` beside `share/`.
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
unset MAJORDOMUS_SHARE

# the installation: the executable under test, beside a share of its own version
I="$W/install"
mkdir -p "$I/bin"
cp "$RB" "$I/bin/majordomus"
cp -R "$ROOT/share" "$I/share"
IB="$I/bin/majordomus"
own="$(cd "$I/share" && pwd -P)"
version="$("$IB" --version | awk '{print $NF}')"
grep -qx "version=$version" "$own/version.txt" \
  || { echo "    the share copied from the tree is not of this executable's version $version"; exit 1; }

# a stale checkout's share: another version, and no ui/
stale="$W/stale-share"
cp -R "$ROOT/share" "$stale"
stale="$(cd "$stale" && pwd -P)"
rm -rf "$stale/ui"
printf '# planted by case 2994\nversion=0.0.1\n' > "$stale/version.txt"

trap '"$IB" serve stop --repo "$R" >/dev/null 2>&1 || true; "$RB" serve stop --repo "$R" >/dev/null 2>&1 || true' EXIT
in_repo() { ( cd "$R" && "$@" ); }

# ---------------------------------------------------------------- 1. a skewed MAJORDOMUS_SHARE
expect_exit 0 in_repo env MAJORDOMUS_SHARE="$stale" "$IB" capabilities validate
expect_grep "OK   share       $own \\(installation; passed over MAJORDOMUS_SHARE: the share at $stale states version 0\\.0\\.1"
expect_grep 'holds no ui/'

# ---------------------------------------------------------------- 2. a skewed repository share
cp -R "$stale" "$R/share"
expect_exit 0 in_repo "$IB" capabilities validate
expect_grep "OK   share       $own \\(installation; passed over repository: "
expect_grep 'states version 0\.0\.1'
rm -rf "$R/share"

# ---------------------------------------------------------------- 3. --share is a person's word
expect_exit 0 in_repo "$IB" capabilities --share "$stale" validate
expect_grep "OK   share       $stale \\(--share\\) "

# ---------------------------------------------------------------- 4. no installation to fall back to
# the executable from the build tree has no share beside it: the skewed one is served, as before
expect_exit 0 in_repo env MAJORDOMUS_SHARE="$stale" "$RB" capabilities validate
expect_grep "OK   share       $stale \\(MAJORDOMUS_SHARE\\) "

# ---------------------------------------------------------------- 5. the server serves its own share, and says so
in_repo env MAJORDOMUS_SHARE="$stale" "$IB" serve ensure --port 0 --idle 60 --wait 40 --format json \
  > "$T/ensure.json" 2> "$T/ensure.err" \
  || { echo "    the server did not come up:"; cat "$T/ensure.json" "$T/ensure.err"; exit 1; }
url="$(jq -r '.url' "$T/ensure.json")"
[ -n "$url" ] && [ "$url" != null ] || { echo "    no address"; cat "$T/ensure.json"; exit 1; }
in_repo "$IB" serve status --format json > "$T/status.json" \
  || { echo "    serve status --format json failed"; exit 1; }
jq -e --arg p "$own" '.servers[] | select(.this_checkout) | .lease.share
  | .path == $p and .standing == "current" and .ui_present == true
    and (.origin | startswith("installation; passed over MAJORDOMUS_SHARE: "))' \
  "$T/status.json" >/dev/null \
  || { echo "    the server does not serve the installation's share, or does not say why:"; cat "$T/status.json"; exit 1; }
code="$(curl -sS -o /dev/null -w '%{http_code}' -m 10 "$url/ui/assets/ui.css" 2>/dev/null || true)"
[ "$code" = 200 ] || { echo "    /ui/assets/ui.css answered $code, not 200"; exit 1; }
expect_exit 0 in_repo "$IB" serve stop

echo "    a skewed conventional share is passed over for the installation's own, and the share used says why; --share is used as given"

# majordomus-covers: none
# The Issues & PRs and Mesh screens of majordomus ui (I2482), through a real server over a
# fixture `majordomus init` installs:
#
#   work   the plan's issues by status, each row leading to the issue's inspector
#   mesh   the topology this runtime knows, and a machine it does not know said as such,
#          never drawn as one
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# the fixture's server listens on loopback whatever the machine's own default host is
unset MAJORDOMUS_HTTP_HOST

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
get() { curl -s --max-time 60 -o page.html -w '%{http_code}' "$U$1"; }

# ---------------------------------------------------------------- work
[ "$(get /ui/work)" = 200 ] || { echo "    /ui/work did not answer 200"; exit 1; }
for want in 'Issues &amp; Pull Requests' 'plan.status' 'plan.issues'; do
  grep -qF "$want" page.html || { echo "    /ui/work does not show: $want"; exit 1; }
done

# ---------------------------------------------------------------- mesh
[ "$(get /ui/mesh)" = 200 ] || { echo "    /ui/mesh did not answer 200"; exit 1; }
grep -qF 'mesh.peers' page.html || { echo "    /ui/mesh does not name mesh.peers"; exit 1; }
[ "$(get '/ui/mesh?node=no-such-machine')" = 200 ] || { echo "    ?node= did not answer 200"; exit 1; }
grep -qF 'knows no machine by this name' page.html \
  || { echo "    a machine the mesh does not know is not said as such"; exit 1; }
[ "$(get '/ui/mesh?node=x%22%20onclick%3D%22y')" = 200 ] || { echo "    a hostile node did not answer"; exit 1; }
grep -q 'onclick="y' page.html && { echo "    the page echoes a handler a link carried"; exit 1; }

echo "    the plan's work and the mesh answer, and an unknown machine is said, never drawn"

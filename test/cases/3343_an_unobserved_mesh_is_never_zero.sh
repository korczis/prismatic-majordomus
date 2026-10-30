# majordomus-covers: none
# A process that is on no mesh says the mesh was not observed, with the runtime's own reason,
# and never draws it as measured (I2943). The published /ui/mesh is read on a Pages runner that
# is on no mesh, and it said "Mesh active: no, Machines: 0, Journal clock: 0": a measurement
# nobody made. A fixture `majordomus init` installs has the mesh off by default, which is the
# runner's condition, so its served /ui/mesh and dashboard are what a snapshot publishes.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
unset MAJORDOMUS_HTTP_HOST

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
get() { curl -s --max-time 60 -o page.html -w '%{http_code}' "$U$1"; }
measured() {   # <page>: what an unobserved mesh must never be drawn as
  for word in 'Mesh active' 'Journal clock' 'No machine is known' 'Mesh machines'; do
    grep -qF "$word" page.html && { echo "    $1 draws an unobserved mesh as measured: $word"; return 1; }
  done
  return 0
}

# ---------------------------------------------------------------- the mesh screen
[ "$(get /ui/mesh)" = 200 ] || { echo "    /ui/mesh did not answer 200"; exit 1; }
grep -qF '>not observed<' page.html || { echo "    /ui/mesh does not say the mesh was not observed"; exit 1; }
grep -qF 'this runtime is not on the mesh' page.html \
  || { echo "    /ui/mesh does not say why: the runtime is not on the mesh"; exit 1; }
grep -qF 'The mesh was not observed here' page.html || { echo "    the topology draws an unobserved mesh"; exit 1; }
measured /ui/mesh || exit 1

# ---------------------------------------------------------------- the dashboard's mesh tile and panel
[ "$(get /ui)" = 200 ] || { echo "    /ui did not answer 200"; exit 1; }
grep -qF '>not observed<' page.html || { echo "    the dashboard does not say the mesh was not observed"; exit 1; }
measured /ui || exit 1
echo "    a process on no mesh says the mesh was not observed and why, on /ui/mesh and the dashboard, and draws no count, clock or 'no'"

# majordomus-covers: none
# The Command and Capabilities screens of majordomus ui (I2482), through a real server over a
# fixture `majordomus init` installs:
#
#   command       every command with what it changes; a command the registry also serves as a
#                 capability leads to that capability's inspector; the origin tab narrows it
#   capabilities  the catalogue by kind, and ?id= opens one capability's detail
#   the sources   each screen names the capabilities it read
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

# ---------------------------------------------------------------- command
[ "$(get /ui/command)" = 200 ] || { echo "    /ui/command did not answer 200"; exit 1; }
for want in 'Everything that can be run' 'commands.list' 'executions.list' 'majordomus init'; do
  grep -qF "$want" page.html || { echo "    /ui/command does not show: $want"; exit 1; }
done
grep -qF 'href="/ui/capabilities?id=' page.html \
  || { echo "    no command of the catalogue leads to the capability that serves it"; exit 1; }
[ "$(get '/ui/command?origin=executable')" = 200 ] || { echo "    the origin tab did not answer"; exit 1; }
grep -qF 'aria-current="page"' page.html || { echo "    the origin tab is not marked current"; exit 1; }

# ---------------------------------------------------------------- capabilities
[ "$(get /ui/capabilities)" = 200 ] || { echo "    /ui/capabilities did not answer 200"; exit 1; }
for want in 'By kind' 'By stability' 'capabilities.list' 'repository.info' 'plan.status'; do
  grep -qF "$want" page.html || { echo "    /ui/capabilities does not show: $want"; exit 1; }
done
[ "$(get '/ui/capabilities?id=plan.status')" = 200 ] || { echo "    ?id=plan.status did not answer"; exit 1; }
grep -qF 'id="detail"' page.html || { echo "    ?id=plan.status opened no detail"; exit 1; }

echo "    every command says what it changes and leads to its capability; a capability opens its detail"

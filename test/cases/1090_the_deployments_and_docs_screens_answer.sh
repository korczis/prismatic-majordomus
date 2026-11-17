# majordomus-covers: none
# The Deployments and Documentation screens of majordomus ui (I2482), through a real server
# over a fixture `majordomus init` installs:
#
#   deployments  the declared deployments, and the slow delivery report as a panel of its own:
#                a page without JavaScript, the panel alone with ?fragment
#   docs         the features, the documents under docs/ and the public surfaces, each surface
#                a link to where it is served
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
get() { curl -s --max-time 120 -o page.html -w '%{http_code}' "$U$1"; }

# ---------------------------------------------------------------- deployments
[ "$(get /ui/deployments)" = 200 ] || { echo "    /ui/deployments did not answer 200"; exit 1; }
for want in 'deploy.list' 'data-panel-src="/ui/panels/delivery"'; do
  grep -qF "$want" page.html || { echo "    /ui/deployments does not show: $want"; exit 1; }
done
[ "$(get /ui/panels/delivery)" = 200 ] || { echo "    the delivery panel did not answer 200"; exit 1; }
grep -q '^<!doctype html>' page.html || { echo "    the delivery panel is no page of its own"; exit 1; }
grep -qF 'delivery.report' page.html || { echo "    the delivery panel does not name delivery.report"; exit 1; }
[ "$(get '/ui/panels/delivery?fragment')" = 200 ] || { echo "    the delivery fragment did not answer"; exit 1; }
grep -q '<html' page.html && { echo "    the fragment carries a whole page"; exit 1; }

# ---------------------------------------------------------------- docs
[ "$(get /ui/docs)" = 200 ] || { echo "    /ui/docs did not answer 200"; exit 1; }
for want in 'What the product does' 'objects.list' 'web.surfaces' 'href="/openapi.json"'; do
  grep -qF "$want" page.html || { echo "    /ui/docs does not show: $want"; exit 1; }
done

echo "    deployments and the delivery panel answer as page and fragment; the docs link their surfaces"

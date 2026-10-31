# majordomus-covers: none
# The home page is the UI's front door (I2480), through a real server over a fixture:
#
#   the frame     a browser that opens / lands on a screen of majordomus ui, in its frame and
#                 with its stylesheet, carrying the version of the executable that answered
#   the screens   every screen in the UI's navigation is a card on it
#   the surfaces  every public surface that answers is linked, nothing under /docs (the
#                 fixture serves no documentation), and the page does not link itself; a
#                 surface that is not built and the command that builds it is case 488's
#   the top bar   Docs leads to the UI's Documentation screen, which answers, never to /docs
#                 while it answers 404
#   a program     asking / for JSON still gets the machine-readable index
#
# The unit tests in apps/majordomus-cli/src/ui/front.rs own the rendering; this case owns the
# seam: the router choosing the front door for a browser, on the route a person types.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is not installed"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# the fixture's server listens on loopback whatever the machine's own default host is
unset MAJORDOMUS_HTTP_HOST

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
page="$(curl -s --max-time 120 -H 'Accept: text/html' "$U/")"
index="$(curl -s --max-time 60 -H 'Accept: application/json' "$U/")"
links="$(printf '%s' "$page" | grep -o 'href="[^"]*"' | sed 's/href="//; s/"$//')"

# ---------------------------------------------------------------- the frame
printf '%s' "$page" | grep -q 'class="mj-body mj-ui"' || { echo "    / is not drawn in the UI's frame"; exit 1; }
printf '%s\n' "$links" | grep -q '^/ui/assets/ui\.css' || { echo "    / does not carry the UI's stylesheet"; exit 1; }
version="$("$RB" --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?' | head -1)"
printf '%s' "$page" | grep -qF ">$version<" || { echo "    / does not carry the version $version of the executable that answered"; exit 1; }

# ---------------------------------------------------------------- the screens
# the cards of "Start here" alone: the sidebar links every screen too, and must not answer
# for them
cards="$(printf '%s' "$page" | sed -n 's/.*id="mj-ui-front-screens"\(.*\)id="mj-ui-front-served".*/\1/p' \
  | grep -o 'class="mj-ui-card mj-ui-card--link" href="[^"]*"' | sed 's/.*href="//; s/"$//')"
[ -n "$cards" ] || { echo "    / has no Start here cards"; exit 1; }
for route in /ui /ui/roadmap /ui/issues /ui/prs /ui/capabilities /ui/mesh /ui/evidence /ui/governance; do
  printf '%s\n' "$cards" | grep -qx "$route" || { echo "    no card of / leads to the screen $route"; exit 1; }
done

# ---------------------------------------------------------------- the surfaces
for path in $(printf '%s' "$index" | jq -r '.surfaces[] | select(.visibility == "public" and .ready) | .path'); do
  [ "$path" = / ] && continue
  printf '%s\n' "$links" | sed 's#\(..*\)/$#\1#' | grep -qx "${path%/}" || { echo "    / does not link the surface at $path"; exit 1; }
done
printf '%s\n' "$links" | grep -q '^/docs' && { echo "    / links the documentation, which this fixture has not built"; exit 1; }
printf '%s\n' "$links" | grep -qx '/' && { echo "    / links itself"; exit 1; }

# ---------------------------------------------------------------- the top bar
[ "$(curl -s -o /dev/null -w '%{http_code}' "$U/docs/")" = 200 ] && { echo "    the fixture built documentation after all"; exit 1; }
printf '%s\n' "$links" | grep -qx '/ui/docs' || { echo "    the top bar does not lead to the UI's Documentation screen"; exit 1; }
[ "$(curl -s -o /dev/null -w '%{http_code}' --max-time 120 "$U/ui/docs")" = 200 ] || { echo "    /ui/docs does not answer"; exit 1; }

# ---------------------------------------------------------------- a program
printf '%s' "$index" | jq -e '.version and (.surfaces | length > 0)' >/dev/null \
  || { echo "    / asked for JSON no longer answers the index"; exit 1; }

echo "    / is the UI's front door: its frame, every screen, every surface that answers, nothing that does not"

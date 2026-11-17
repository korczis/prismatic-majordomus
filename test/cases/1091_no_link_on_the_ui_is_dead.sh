# majordomus-covers: none
# No link on majordomus ui is dead (I2482), through a real server over a fixture `majordomus
# init` installs: every screen the navigation lists answers 200, and so does every link those
# screens draw to another UI route or to the technical view of an object. The Rust test
# tests/ui_links.rs follows the links as deep as they go; this case reads one level of them
# from the executable a person runs.
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
status() { curl -s --max-time 120 -o page.html -w '%{http_code}' "$U$1"; }
# every href of page.html this case follows: UI routes and object pages, without fragments
links() {
  grep -o 'href="[^"]*"' page.html | sed -e 's/^href="//' -e 's/"$//' -e 's/&amp;/\&/g' -e 's/#.*//' \
    | grep -E '^/ui(/|\?|$)|^/cockpit/object\?' | grep -v '^/ui/assets/' || true
}

[ "$(status /ui)" = 200 ] || { echo "    /ui did not answer 200"; exit 1; }
links | grep -v '?' | LC_ALL=C sort -u > screens.txt
n="$(wc -l < screens.txt | tr -d ' ')"
[ "$n" -ge 13 ] || { echo "    the navigation lists $n screens, fewer than 13:"; sed 's/^/    | /' screens.txt; exit 1; }

: > linked.txt
while IFS= read -r screen; do
  [ "$(status "$screen")" = 200 ] || { echo "    $screen did not answer 200"; exit 1; }
  links | sed "s|\$| $screen|" >> linked.txt
done < screens.txt

dead=0
checked=0
LC_ALL=C sort -u -k1,1 linked.txt > unique.txt
while IFS=' ' read -r target from; do
  checked=$((checked + 1))
  code="$(status "$target")"
  [ "$code" = 200 ] || { echo "    $target (linked from $from) answered $code"; dead=$((dead + 1)); }
done < unique.txt
[ "$dead" = 0 ] || { echo "    $dead dead link(s)"; exit 1; }
[ "$checked" -gt "$n" ] || { echo "    the screens linked only $checked pages"; exit 1; }

echo "    $n screens and the $checked pages they link all answer"

# majordomus-covers: none
# The Knowledge screen of majordomus ui, through a real server over a fixture
# `majordomus init` installs:
#
#   /ui/knowledge   what the deriver left for review and whether it is still writing, read
#                   from knowledge_base.status and knowledge_base.candidates; an empty queue
#                   is said in words
#   the review      named as the commands a person runs, as text: the screen carries no form
#                   and no button, because a screen reads and never executes (ADR 0142)
#   the navigation  every screen leads to it
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

[ "$(get /ui/knowledge)" = 200 ] || { echo "    /ui/knowledge did not answer 200"; exit 1; }
for want in 'What the work left behind' 'knowledge_base.status' 'knowledge_base.candidates' \
  'The deriver' 'By subject' 'Awaiting review' 'No candidate awaits review.' \
  'majordomus knowledge promote &lt;id&gt;' 'majordomus knowledge reject &lt;id&gt;'; do
  grep -qF -- "$want" page.html || { echo "    /ui/knowledge does not show: $want"; exit 1; }
done
# a reading, not a control: the main part of the page offers nothing that would run a command
# the page is one line of markup, so the main part is cut by its tags, not by lines
awk 'BEGIN { RS = "</main>" } NR == 1 { i = index($0, "<main"); if (i) print substr($0, i) }' page.html > main.html
[ -s main.html ] || { echo "    /ui/knowledge has no main part"; exit 1; }
if grep -qE '<form|<button|method="post"' main.html; then
  echo "    the Knowledge screen carries a control; it reads and never executes"; exit 1
fi

[ "$(get /ui)" = 200 ] || { echo "    /ui did not answer 200"; exit 1; }
grep -qF 'href="/ui/knowledge"' page.html || { echo "    the navigation does not lead to /ui/knowledge"; exit 1; }

echo "    the knowledge screen answers from its two sources, names the review as text, and is in the navigation"

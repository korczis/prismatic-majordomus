# majordomus-covers: none
# The capabilities are searched and faceted (I2479), through a real server over a fixture,
# against the executable's own registry:
#
#   a facet     ?module=plan lists the plan's queries and commands and no other module's, and
#               says how many of all it shows; the module choice counts what it would show
#   together    a kind and a surface narrow it further (the plan's command drops out), every
#               row still exposed on that surface
#   the way in  a row opens its declaration in full and keeps the filters
#   bad values  a value a link carries that is no word of the registry is dropped, never echoed
#
# The unit tests in apps/majordomus-cli/src/ui/capabilities.rs own the counting; this case owns
# the seam: the registry the executable a person runs answers, on the route the screen asks.
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
get() { curl -s --max-time 120 "$U$1"; }
rows() { printf '%s' "$1" | grep -o 'id="capability-[^"]*"' | sed 's/id="capability-//; s/"$//'; }
shown() { printf '%s' "$1" | grep -o '[0-9]* of [0-9]* capabilities shown'; }

# ---------------------------------------------------------------- a facet
all="$(get /ui/capabilities)"
total="$(shown "$all" | sed 's/ of .*//')"
[ -n "$total" ] && [ "$total" -gt 20 ] || { echo "    the screen lists too few capabilities: '$(shown "$all")'"; exit 1; }
page="$(get '/ui/capabilities?module=plan')"
ids="$(rows "$page")"
printf '%s\n' "$ids" | grep -qx 'plan.issues' || { echo "    ?module=plan does not list plan.issues"; exit 1; }
other="$(printf '%s\n' "$ids" | grep -v '^plan\.' | head -1)"
[ -z "$other" ] || { echo "    ?module=plan lists $other"; exit 1; }
n="$(printf '%s\n' "$ids" | grep -c .)"
shown "$page" | grep -qx "$n of $total capabilities shown" \
  || { echo "    ?module=plan shows $n rows but says '$(shown "$page")'"; exit 1; }
printf '%s' "$all" | grep -q ">plan ($n)<" \
  || { echo "    the module choice does not count the $n the plan would show"; exit 1; }

# ---------------------------------------------------------------- together
page="$(get '/ui/capabilities?module=plan&kind=query&surface=http')"
n2="$(rows "$page" | grep -c .)"
[ "$n2" -ge 1 ] && [ "$n2" -lt "$n" ] || { echo "    kind and surface did not narrow the plan's $n to fewer: $n2"; exit 1; }
for id in $(rows "$page"); do
  row="$(printf '%s' "$page" | sed -n "s#.*id=\"capability-$id\"\\(.*\\)#\\1#p" | sed 's#</tr>.*##')"
  printf '%s' "$row" | grep -q '>HTTP<' || { echo "    $id is listed under surface=http without an HTTP projection"; exit 1; }
  printf '%s' "$row" | grep -q '>command<' && { echo "    $id is a command under kind=query"; exit 1; }
done

# ---------------------------------------------------------------- the way in
page="$(get '/ui/capabilities?module=plan')"
printf '%s' "$page" | grep -qF 'href="/ui/capabilities?module=plan&amp;id=plan.issues#detail"' \
  || { echo "    a row does not open its declaration keeping the filters"; exit 1; }
page="$(get '/ui/capabilities?module=plan&id=plan.issues')"
printf '%s' "$page" | grep -q 'id="detail"' || { echo "    ?id=plan.issues opened no declaration"; exit 1; }
shown "$page" | grep -qx "$n of $total capabilities shown" || { echo "    opening a declaration lost the filters"; exit 1; }

# ---------------------------------------------------------------- bad values
page="$(get '/ui/capabilities?module=x%22%20onclick%3D%22y&id=%3Cscript%3E')"
printf '%s' "$page" | grep -q 'onclick\|&lt;script&gt;' && { echo "    the page echoes what a link carried"; exit 1; }
shown "$page" | grep -qx "$total of $total capabilities shown" || { echo "    a dropped value still narrowed the list"; exit 1; }

echo "    the capabilities narrow by module, kind and surface, counted, and a row keeps the filters"

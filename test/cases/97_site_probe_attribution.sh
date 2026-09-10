# The site probe names what actually overflows.
#
# `scripts/site-probe` decides that a page scrolls sideways by measuring it, and that verdict
# was never in doubt. What it says *beside* the verdict was: the walk that discounts an
# element a scroller already clips began above the element it was asked about, so the
# `overflow-x-auto` wrapper a wide table sits directly in was never consulted, every finding
# on a page with a table accused that table — which is already wrapped, so nothing the reader
# fixed there helped — and the element that really escaped was further down a list cut at two.
#
# `--self-check` measures fixtures whose answer a person can read off the markup, with the
# same code the sweep runs. This case runs it, and then proves it is a guard and not a
# decoration: a copy of the script with the old walk restored must fail it, naming the table
# it wrongly accused. A self-check nobody has seen fail is a check in name only.
#
# It reads this repository's own script rather than the disposable fixture, because the
# subject is this repository's probe. It writes nothing into the checkout: the self-check
# needs no built site and measures in a temporary directory of its own.
. "$ROOT/test/lib.sh"
P="$ROOT/scripts/site-probe"
[ -x "$P" ] || { echo "    scripts/site-probe is missing or not executable"; exit 1; }

# 1. the probe agrees with fixtures whose answer is known
out=0; "$P" --self-check > self.txt 2>&1 || out=$?
if grep -q '^SKIP site-probe' self.txt; then
  echo "    skip: $(cat self.txt)"
  exit 0
fi
[ "$out" = 0 ] || { echo "    site-probe --self-check exited $out:"; sed 's/^/    | /' self.txt; exit 1; }
for f in scroller escapee positioned prescroller; do
  grep -q "^OK .*selfcheck.*$f:" self.txt || {
    echo "    the self-check did not answer for the $f fixture:"; sed 's/^/    | /' self.txt; exit 1; }
done
# the two halves the attribution turns on, stated as the fixtures measured them
grep -q "^OK .*escapee: code.token" self.txt || {
  echo "    the escaping token was not the first thing named on the page that carries it:"
  sed 's/^/    | /' self.txt; exit 1; }
grep -q "^OK .*positioned: span.sr-only" self.txt || {
  echo "    the positioned element that escapes its scroller went unnamed:"
  sed 's/^/    | /' self.txt; exit 1; }

# 2. the self-check is a guard: the walk it holds, put back the way it was, must fail it.
#    The mutation is the call site — the question asked, not the walk's own body — because
#    that is where the defect was and where the next edit could put it back.
sed 's/&&!clipped(e))/\&\&!clipped(e.parentElement||e))/' "$P" > mutant
cmp -s "$P" mutant && {
  echo "    the pre-fix call site is no longer in scripts/site-probe, so this case proves nothing"
  echo "    about the guard; update the mutation to the shape the walk is asked about now"
  exit 1; }
chmod +x mutant
out=0; ./mutant --self-check > mutant.txt 2>&1 || out=$?
[ "$out" = 10 ] || {
  echo "    the old walk passed the self-check (exit $out); the fixtures do not hold the defect:"
  sed 's/^/    | /' mutant.txt; exit 1; }
grep -q "table.grid' is inside a scroller and must not be named" mutant.txt || {
  echo "    the self-check failed the old walk without naming the misattribution it makes:"
  sed 's/^/    | /' mutant.txt; exit 1; }
echo "    site-probe names what overflows, and the fixtures fail the walk that did not"

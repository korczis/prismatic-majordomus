# majordomus-covers: none
# The internal-link check examines a non-empty set, and the set it examines is the one the
# site actually emits.
#
# scripts/site-check matched `href="$PREFIX/...` and nothing else. PREFIX is base_url with the
# scheme and host stripped; the site is published at the origin root, so PREFIX was the empty
# string and the pattern was `href="/...` — root-relative links, of which a Zola site built with
# an absolute base_url emits none. The loop therefore ran zero times and printed
# "every internal link resolves" on every build, while ninety-five links on the built site did
# not resolve, twenty-three of them on the homepage alone. A check that cannot reach its subject
# is indistinguishable from one that passed, which is why this case asserts the count and not
# merely the verdict: a repair that matched a different empty set would satisfy a verdict test.
. "$ROOT/test/lib.sh"

# the site must be built for the check to have a subject at all
[ -f "$ROOT/site/public/index.html" ] || { echo "    site/public is not built; run scripts/site-build"; exit 0; }

out="$(cd "$ROOT" && scripts/site-check --no-sync 2>&1)" || true
line="$(printf '%s' "$out" | grep -E '^(OK|FAIL)[[:space:]]+link[[:space:]]' | head -1)"
[ -n "$line" ] || { echo "    site-check reports no verdict on internal links"; exit 1; }

case "$line" in
  OK*) ;;
  *) echo "    $line"; exit 1 ;;
esac

# it says how many targets it looked at, and that number is not zero — the whole defect
seen="$(printf '%s' "$line" | sed -n 's/.*[^0-9]\([0-9][0-9]*\) internal link target(s) examined.*/\1/p')"
[ -n "$seen" ] || { echo "    the link verdict does not say how many targets it examined: $line"; exit 1; }
[ "$seen" -gt 0 ] || { echo "    the link check examined no targets at all; this is the defect it was repaired for"; exit 1; }

# and the number is of the order the site actually has: a pattern matching one stray href
# would be non-zero and still blind. The built site links its own pages in the hundreds.
[ "$seen" -ge 100 ] \
  || { echo "    only $seen link target(s) examined; the site emits far more, so the pattern is still missing most of them"; exit 1; }

# the ratchet exists and the verdict is measured against it rather than against nothing
base="$ROOT/.ai/repo/site-link-baseline.txt"
[ -f "$base" ] || { echo "    no .ai/repo/site-link-baseline.txt; the known-broken set is not recorded anywhere"; exit 1; }
n="$(grep -vcE '^[[:space:]]*(#|$)' "$base" || true)"
printf '%s' "$line" | grep -q "$n known-broken" \
  || { echo "    the verdict does not reconcile with the baseline's $n entr(ies): $line"; exit 1; }

# a link that breaks and is not in the baseline fails. Proved by adding one, not by trusting
# the code path: the page is copied so the real build is never touched.
T2="$(mktemp -d)"; trap 'rm -rf "$T2"' EXIT
cp "$ROOT/site/public/index.html" "$T2/index.html.orig"
base_url="$(sed -n 's/^base_url = "\(.*\)"/\1/p' "$ROOT/site/config.toml" | sed 's#/$##')"
sed "s#href=\"$base_url/\"#href=\"$base_url/a-route-this-site-does-not-build/\"#" \
  "$ROOT/site/public/index.html" > "$T2/injected.html"
cmp -s "$T2/index.html.orig" "$T2/injected.html" \
  && { echo "    could not inject a broken link; the fixture did not change and the negative half proves nothing"; exit 1; }
cp "$T2/injected.html" "$ROOT/site/public/index.html"
out2="$(cd "$ROOT" && scripts/site-check --no-sync 2>&1)" || true
cp "$T2/index.html.orig" "$ROOT/site/public/index.html"
printf '%s' "$out2" | grep -q 'a-route-this-site-does-not-build/ does not resolve, and is not in the baseline' \
  || { echo "    a link outside the baseline did not fail the check"; printf '%s\n' "$out2" | grep -E '(OK|FAIL)[[:space:]]+link' | sed 's/^/    /'; exit 1; }

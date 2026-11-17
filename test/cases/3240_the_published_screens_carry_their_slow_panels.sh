# majordomus-covers: none
# A published screen carries what the live one shows once its script has run (I2850). The
# snapshot's pages have no script that fetches a panel, so a slow panel the UI defers must
# already be in the page, read, where its placeholder stood.
#
#   no placeholder       no page of the snapshot keeps a data-panel-src placeholder
#   the drain is there   the PR Drain carries the drain's own answer, not a link to it
#   one heading          a screen with its panel read in is still a page with one h1
#   and it is checked    a planted placeholder fails this case's own reading
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install
S="$PWD/target/web/ui-snapshot"
"$RB" web report ui-snapshot --limit 40 >/dev/null 2> err.txt \
  || { echo "    the snapshot was not written:"; tail -5 err.txt | sed 's/^/    | /'; exit 1; }

# the pages of a tree that still defer a panel to a script
deferring() {
  find "$1" -name '*.html' | LC_ALL=C sort | while IFS= read -r f; do
    if grep -q 'data-panel-src=' "$f"; then printf '%s\n' "${f#"$1"/}"; fi
  done
}

pages="$(find "$S" -name '*.html' | wc -l | tr -d ' ')"
[ "$pages" -ge 10 ] || { echo "    only $pages page(s) were written; the reading is not looking at the snapshot"; exit 1; }

# ---------------------------------------------------------------- no placeholder
deferring "$S" > bad.txt
if [ -s bad.txt ]; then
  echo "    $(wc -l < bad.txt | tr -d ' ') page(s) still defer a panel to a script the snapshot does not run:"
  head -8 bad.txt | sed 's/^/    | /'; exit 1
fi

# ---------------------------------------------------------------- the drain is there
P="$S/prs/index.html"
[ -f "$P" ] || { echo "    the snapshot holds no PR Drain"; exit 1; }
if grep -q 'Open the drain on its own page' "$P"; then
  echo "    the PR Drain is still the placeholder line"; exit 1
fi
# this repository has no forge, so the drain's own answer is that it observed none
grep -q 'forge was not observed' "$P" \
  || { echo "    the PR Drain carries no answer of the drain:"; grep -o 'mj-ui-panel-title">[^<]*' "$P" | head -5 | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- one heading
for f in "$P" "$S/index.html"; do
  n="$(grep -o '<h1' "$f" | wc -l | tr -d ' ')"
  [ "$n" = 1 ] || { echo "    ${f#"$S"/} has $n headings with its panel read in"; exit 1; }
done

# ---------------------------------------------------------------- and it is checked
mkdir -p planted && cp "$P" planted/index.html
printf '<section data-panel-src="/ui/panels/drain"></section>\n' >> planted/index.html
deferring "$PWD/planted" | grep -q 'index.html' \
  || { echo "    a planted placeholder was not found; this case reads nothing"; exit 1; }

echo "    $pages page(s) of the snapshot: none defers a panel, the PR Drain carries the drain's answer, each screen keeps one heading"

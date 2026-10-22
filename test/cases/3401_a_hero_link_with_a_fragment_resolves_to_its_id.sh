# majordomus-covers: none
# A navigation or hero link of the homepage with a #fragment resolves when its page exists and
# carries an element with that id (I3008). On 2026-10-18 the homepage's first hero link with a
# fragment, /roadmap/#delivery, failed the 0.30.0 top's site job: the rule looked for the file
# site/public/roadmap/#delivery/index.html, although /roadmap/ carried id="delivery". The rule now
# splits the fragment off; this case holds it, through `scripts/site-check --nav`, on a built
# public directory of this case's own:
#
#   present     a hero link to /roadmap/#delivery passes when /roadmap/ carries id="delivery"
#   missing id  the same link fails when the page carries no such element, and says so
#   missing     a hero link to a page the build does not carry fails as unresolved
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"

S="$(mktemp -d "${TMPDIR:-/tmp}/mj3401.XXXXXX")"
trap 'rm -rf "$S"' EXIT
PUB="$S/public"; CFG="$S/config.toml"
mkdir -p "$PUB/roadmap" "$PUB/docs"
printf 'base_url = "https://example.org"\n' > "$CFG"
nav() { "$ROOT/scripts/site-check" --nav "$PUB" "$CFG"; }
# the links are absolute, as Zola writes them: the rule reads a link through the site's base_url
home() {   # <the hero's links, as paths, space-separated>
  { printf '<html><body><nav><a href="https://example.org/docs/">Docs</a></nav><section id="hero">'
    for h in "$@"; do printf '<a href="https://example.org%s">x</a>' "$h"; done
    printf '</section>\n<footer><a href="https://example.org/elsewhere/">past the hero, not read</a></footer></body></html>\n'; } > "$PUB/index.html"
}
printf '<html><body><h1>Docs</h1></body></html>\n' > "$PUB/docs/index.html"

# ---------------------------------------------------------------- present
home /roadmap/#delivery
printf '<html><body><dl><dt id="delivery" class="x">Delivery</dt></dl></body></html>\n' > "$PUB/roadmap/index.html"
expect_exit 0 nav || exit 1
echo "    present: /roadmap/#delivery resolves when /roadmap/ carries id=\"delivery\""

# ---------------------------------------------------------------- missing id
printf '<html><body><dl><dt class="x">Delivery</dt></dl></body></html>\n' > "$PUB/roadmap/index.html"
expect_exit 10 nav || exit 1
expect_grep 'navigation/hero link /roadmap/#delivery names an element its page does not carry' || exit 1
echo "    missing id: the same link fails when the page carries no element with that id"

# ---------------------------------------------------------------- missing
home /nowhere/#top
expect_exit 10 nav || exit 1
expect_grep 'navigation/hero link /nowhere/ does not resolve' || exit 1
echo "    missing: a link to a page the build does not carry fails as unresolved"

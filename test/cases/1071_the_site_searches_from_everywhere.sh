# majordomus-covers: none
# The site searches from everywhere: one index, one search, a trigger on every layout.
#
# The search (site/search.js) reads /docs/index.json, which the build writes from the pages it
# published; scripts/lib/search-check.mjs (site-check 20) holds the two to each other and every
# layout to the trigger. This case builds this checkout's site with zola into a scratch
# directory — the templates, content and committed data as they are, without the stylesheets —
# writes the index the way scripts/site-build does, and proves:
#
#   1. a clean build passes every part of the check, every page carries the trigger, the dialog
#      and the script once, the dialog is the combobox pattern over the listbox, and every index
#      entry names the section it sits in as that section's page titles itself;
#   2. a layout that loses the trigger is refused, at its template and on its built pages;
#   3. a layout family added without the navbar is refused at its source, before it is built;
#   4. an index entry pointing at a page the build did not write is refused, and so is a page
#      the build wrote that the index lost;
#   5. nothing outside the publication is offered: an entry of a kind site/data/publication.toml
#      leaves unpublished, and one naming a source git does not track, are refused;
#   6. site-check runs the check, and site-build ships the script it checks for.
. "$ROOT/test/lib.sh"
command -v zola >/dev/null || skip "zola absent"
command -v node >/dev/null || skip "node absent"
command -v jq >/dev/null || skip "jq absent"
CHECK="$ROOT/scripts/lib/search-check.mjs"
[ -x "$CHECK" ] || { echo "    $CHECK is missing or not executable"; exit 1; }

fail=0
note() { echo "    $1"; fail=1; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj-1071.XXXXXX")"
trap 'rm -rf "$S"' EXIT

# ---------------------------------------------------------------- a copy of this site
mkdir -p "$S/site"
cp -R "$ROOT/site/content" "$ROOT/site/content-src" "$ROOT/site/templates" "$ROOT/site/static" \
  "$ROOT/site/data" "$ROOT/site/config.toml" "$S/site/"
# the stylesheets are Tailwind build products, ignored by git; the pages only link them
[ -f "$S/site/static/app.css" ] || printf '/* fixture: no Tailwind build here. */\n' > "$S/site/static/app.css"
[ -f "$S/site/static/kit.css" ] || printf '/* fixture: no Tailwind build here. */\n' > "$S/site/static/kit.css"
# a build composes no other surface here, and says so the way site-build does
rm -f "$S/site/data/build.json"
ENT="$S/site/data/generated/entities.json"
REG="$S/site/data/registry/registry.json"

build() { # the pages, then the index written from them, as scripts/site-build orders the two
  rm -rf "$S/public"
  ( cd "$S/site" && run_quiet "$S/zola.err" zola build --force -o "$S/public" >/dev/null ) \
    || { echo "    zola could not build the copy of the site"; tail -5 "$S/zola.err"; exit 1; }
  run_quiet "$S/index.err" node "$ROOT/scripts/lib/docs-index.mjs" "$S/public" "$ENT" "$REG" >/dev/null \
    || { echo "    docs-index refused the build"; tail -5 "$S/index.err"; exit 1; }
}
check() { # exit status of the check over the copy; its output in $S/check.out
  local rc=0
  MJ_ROOT="$ROOT" node "$CHECK" --public "$S/public" --templates "$S/site/templates" --entities "$ENT" \
    > "$S/check.out" 2>&1 || rc=$?
  return "$rc"
}
said() { grep -qF -- "$1" "$S/check.out"; }

# ---------------------------------------------------------------- 1. a clean build passes
build
rc=0; check || rc=$?
if [ "$rc" != 0 ]; then note "the check refused a clean build of this checkout (exit $rc):"; sed 's/^/      /' "$S/check.out" | head -20; fi
for part in 'index entries is a page this build wrote' 'built page(s) is an entry' 'no entry lies outside the publication' \
            'carry the trigger, the dialog and the script once' 'every layout reaches the trigger'; do
  said "$part" || note "the clean run does not report: $part"
done
H="$S/public/index.html"
for want in '<button[^>]*data-site-search-open[^>]*aria-controls="site-search"' 'aria-keyshortcuts="/ Control\+K Meta\+K"' \
            '<dialog id="site-search"' 'role="combobox"[^>]*aria-controls="site-search-results"' \
            'id="site-search-results" role="listbox"' 'js/search\.js'; do
  grep -qE -- "$want" "$H" || note "the homepage does not carry /$want/"
done
IX="$S/public/docs/index.json"
jq -e 'all(.pages[]; has("section"))' "$IX" >/dev/null || note "an index entry carries no section"
adrs="$(jq -r '.pages[] | select(.route == "/adrs/") | .title' "$IX")"
[ -n "$adrs" ] || note "the index has no /adrs/ page to name a section"
jq -e --arg t "$adrs" '[.pages[] | select(.kind == "adr")] | length > 0 and all(.section == $t)' "$IX" >/dev/null \
  || note "a decision's section is not the title /adrs/ gives itself ('$adrs')"
jq -e '.pages[] | select(.route == "/") | .section == null' "$IX" >/dev/null || note "the root page names a section"

# ---------------------------------------------------------------- 2. a layout loses the trigger
NAV="$S/site/templates/partials/navbar.html"
cp "$NAV" "$S/navbar.keep"
sed '/<button type="button" data-site-search-open/,/<\/button>/d' "$S/navbar.keep" > "$NAV"
if grep -q 'data-site-search-open' "$NAV"; then note "the mutation did not remove the trigger from the navbar"; fi
build
rc=0; check || rc=$?
[ "$rc" = 10 ] || note "a navbar without the trigger exited $rc, not 10"
said 'site/templates/base.html (no data-site-search-open)' || note "the refusal does not name the layout that lost the trigger"
said 'does not carry the search trigger' || note "the refusal does not name the built pages without the trigger"
cp "$S/navbar.keep" "$NAV"

# ---------------------------------------------------------------- 3. a layout family without the navbar
printf '<!doctype html>\n<html lang="en"><head><title>{%% block title %%}{%% endblock title %%}</title></head>\n<body><main>{%% block content %%}{%% endblock content %%}</main></body></html>\n' \
  > "$S/site/templates/bare.html"
printf '{%% extends "bare.html" %%}\n{%% block content %%}<h1>A page of another family</h1>{%% endblock content %%}\n' \
  > "$S/site/templates/bare-page.html"
rc=0; check || rc=$?
[ "$rc" = 10 ] || note "a layout that renders a document without the navbar exited $rc, not 10"
said 'site/templates/bare.html (no data-site-search-open)' || note "the refusal does not name the layout without the trigger"
rm -f "$S/site/templates/bare.html" "$S/site/templates/bare-page.html"

# ---------------------------------------------------------------- 4. an entry and its page disagree
build
cp "$IX" "$S/index.keep"
jq '.pages += [{"route": "/no-such-page/", "title": "A page nobody built", "description": "", "section": null}]' "$S/index.keep" > "$IX"
rc=0; check || rc=$?
[ "$rc" = 10 ] || note "an index entry pointing at a missing page exited $rc, not 10"
said '/no-such-page/: an index entry the build did not write as a page' || note "the refusal does not name the entry with no page"
cp "$S/index.keep" "$IX"
gone="$(jq -r '[.pages[] | select(.kind == "rule")][0].route' "$IX")"
[ -f "$S/public${gone}index.html" ] || note "the rule page $gone the index lists was not built"
jq --arg r "$gone" '.pages |= map(select(.route != $r))' "$S/index.keep" > "$IX"
rc=0; check || rc=$?
[ "$rc" = 10 ] || note "a built page the index lost exited $rc, not 10"
said "$gone: a page this build wrote that the search cannot find" || note "the refusal does not name the page the index lost"
cp "$S/index.keep" "$IX"

# ---------------------------------------------------------------- 5. nothing outside the publication
hidden="$(jq -r '[.catalogue[] | select(.projection == "none")][0].kind' "$ENT")"
[ -n "$hidden" ] && [ "$hidden" != null ] || note "the catalogue declares no unpublished kind to try"
jq --arg k "$hidden" '(.pages[] | select(.route == "/docs/")) += {"kind": $k}' "$S/index.keep" > "$IX"
rc=0; check || rc=$?
[ "$rc" = 10 ] || note "an entry of the unpublished kind $hidden exited $rc, not 10"
said "/docs/ (kind $hidden, which the publication declares unpublished)" || note "the refusal does not name the unpublished kind"
jq '(.pages[] | select(.route == "/docs/")) += {"source": ".ai/local/notes/private.md"}' "$S/index.keep" > "$IX"
rc=0; check || rc=$?
[ "$rc" = 10 ] || note "an entry naming an untracked source exited $rc, not 10"
said '/docs/ (source .ai/local/notes/private.md, which git does not track)' || note "the refusal does not name the untracked source"
cp "$S/index.keep" "$IX"
rc=0; check || rc=$?
[ "$rc" = 0 ] || { note "the restored build no longer passes (exit $rc):"; head -5 "$S/check.out"; }

# ---------------------------------------------------------------- 6. wired into the gate and the build
grep -qE '^relay_check search "\$ROOT/scripts/lib/search-check\.mjs"' "$ROOT/scripts/site-check" \
  || note "scripts/site-check does not run the search check"
grep -qF 'cp "$SITE/search.js" "$SITE/static/js/search.js"' "$ROOT/scripts/site-build" \
  || note "scripts/site-build does not ship site/search.js"
grep -qE 'for a in .* js/search\.js ' "$ROOT/scripts/site-check" || note "site-check does not require js/search.js among the assets"

exit "$fail"

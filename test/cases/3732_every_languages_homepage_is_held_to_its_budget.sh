# majordomus-covers: none
# Every language's homepage is held to the declared weight budget (I3759): scripts/ci/homepage-check's
# `weight` check measures site/public/index.html for the source language and
# site/public/<lang>/index.html for every other language site/i18n/languages.json declares, against
# the one [budget] of site/data/homepage.toml, exercised against a tree this case builds.
#
# What is proved: a tree whose every homepage is under the budget reports one OK line per declared
# language; a Czech copy over the HTML budget fails, naming cs and the bytes over, while the English
# page still passes; an English page over it fails naming en; a declared language whose homepage the
# build did not write fails rather than being skipped. And the template ships the capability picker
# with the one option it shows, drawing the others from graphs/surfaces.json.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/ci/homepage-check"
[ -x "$CHECK" ] || { echo "    scripts/ci/homepage-check is missing or not executable"; exit 1; }

F="$PWD/fixture"
page() { # <file> <padding bytes>
  { printf '<html><head><title>home</title></head><body><main>\n<section id="hero"><a href="/x/">x</a></section>\n'
    head -c "$2" /dev/zero | tr '\0' 'x'
    printf '\n</main></body></html>\n'; } > "$1"
}
fresh() {
  rm -rf "$F"; mkdir -p "$F/site/data/registry" "$F/site/data/generated" "$F/site/public/cs" "$F/site/i18n" "$F/docs/generated"
  printf 'order = ["hero"]\n\n[budget]\nhtml_bytes = 4000\nassets_bytes = 100\ninline_json_bytes = 20\n' > "$F/site/data/homepage.toml"
  printf '[indexing]\nunlisted = []\n' > "$F/site/data/nav.toml"
  printf '%s\n' '{"schema":"site-languages/v1","source":"en","languages":["en","cs"],"names":{"en":"English","cs":"Czech"}}' > "$F/site/i18n/languages.json"
  printf '%s\n' '{"features":[],"providers":[],"evidence":{"available":false,"claims":{}}}' > "$F/site/data/registry/product.json"
  printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"}]}' > "$F/docs/generated/web.json"
  printf '%s\n' '{"status_order":["guaranteed"],"claims":[]}' > "$F/site/data/generated/capabilities.json"
  printf '%s\n' '{"states":{"guaranteed":"neutral","unknown":"neutral"}}' > "$F/site/data/registry/design.json"
  printf '%s\n' '{"install_command":"i","next_command":"n","verify_command":"v"}' > "$F/site/data/registry/distribution.json"
  printf '%s\n' '{"registry":{"builtin":[]}}' > "$F/site/data/registry/registry.json"
  page "$F/site/public/index.html" 1000
  page "$F/site/public/cs/index.html" 1200
  printf '<?xml version="1.0"?><urlset><url><loc>https://x.test/</loc></url></urlset>\n' > "$F/site/public/sitemap.xml"
}
run() { MJ_ROOT="$F" "$CHECK" --only weight > out.txt 2>&1 || true; }
expect() { # <regex> <what>
  grep -qE "$1" out.txt || { echo "    $2: no line matching '$1'"; cat out.txt; exit 1; }
}
refuse() { # <regex> <what>
  ! grep -qE "$1" out.txt || { echo "    $2: a line matched '$1'"; cat out.txt; exit 1; }
}

# --- every declared language under the budget: one OK line each
fresh; run
expect '^OK   weight +en: the homepage is [0-9]+ of 4000 bytes of HTML' "the English homepage under the budget"
expect '^OK   weight +cs: the homepage is [0-9]+ of 4000 bytes of HTML' "the Czech homepage under the budget"
refuse '^FAIL weight' "a tree under the budget"

# --- the Czech copy over the HTML budget fails, naming cs and the bytes over; English still passes
fresh; page "$F/site/public/cs/index.html" 5000; run
expect '^FAIL weight +cs: the homepage is [0-9]+ bytes of HTML, over the declared 4000 by [0-9]+' "a Czech homepage over the budget"
expect '^OK   weight +en: ' "the English homepage beside an over-budget Czech one"

# --- the English page over it fails naming en
fresh; page "$F/site/public/index.html" 5000; run
expect '^FAIL weight +en: the homepage is [0-9]+ bytes of HTML' "an English homepage over the budget"

# --- negative: a declared language whose homepage the build did not write fails, never skipped
fresh; rm "$F/site/public/cs/index.html"; run
expect '^FAIL weight +cs: site/i18n/languages.json declares the language and the build wrote no cs/index.html' "a declared language with no homepage"

# --- the template ships the picker with the one option it shows and draws the rest from the slice
T="$ROOT/site/templates/index.html"
picker="$(awk '/<select data-surface-picker/{f=1} f{print} f&&/<\/select>/{exit}' "$T")"
[ -n "$picker" ] || { echo "    site/templates/index.html renders no data-surface-picker"; exit 1; }
! printf '%s\n' "$picker" | grep -q '{%-* *for ' || { echo "    the picker still loops over the registry in the template:"; printf '%s\n' "$picker"; exit 1; }
[ "$(printf '%s\n' "$picker" | grep -o '<option value=' | wc -l | tr -d ' ')" = 1 ] \
  || { echo "    the picker ships other than one option:"; printf '%s\n' "$picker"; exit 1; }
printf '%s\n' "$picker" | grep -q '<template x-for="c in (list' \
  || { echo "    the picker draws no options from the fetched slice:"; printf '%s\n' "$picker"; exit 1; }

echo "    weight: every declared language's homepage is held to the one budget, and the picker ships one option"

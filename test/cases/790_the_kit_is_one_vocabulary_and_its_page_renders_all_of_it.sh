# majordomus-covers: none
# The kit is one vocabulary and /kit/ renders all of it: by audit, then by mutation.
#
# The kit is three files that must agree — share/design/kit.css says how a thing looks,
# site/templates/kit/ says what a thing is, and site/templates/kit-page.html is the page that
# renders every component — plus the icons the templates name and the two files a page opts
# into. Nothing else holds them to each other: a component nobody renders is untested, a class
# no template emits is dead weight in a sheet every kit page loads, a class no sheet defines is
# a component that silently looks like nothing, and an icon that is not in the set fails the
# build of the one page that names it. The audit below asks all of it of the repository, from
# the sources alone (no build, no browser); then a fixture made of the same files is broken one
# way at a time, and each break must be named, so a check that cannot fail is caught here.
#
# The browser half — every copy button, tab and search field does what its spec says — is
# scripts/interaction-probe with scripts/lib/interaction-specs/kit-*.mjs.
. "$ROOT/test/lib.sh"

# kit_audit <root>: every finding on its own line; exit 10 when there is one.
kit_audit() {
  local r="$1" kit="$1/site/templates/kit" page="$1/site/templates/kit-page.html" found=0 n c b
  local css="$r/share/design/kit.css" sheets="$r/share/design/kit.css $r/share/design/primitives.css $r/share/design/status.css"
  # 1. every component is called somewhere other than its own definition, and the page that
  #    renders the kit calls it or calls a component that does
  for n in $(cat "$kit"/*.html | sed -n 's/^{% component \([a-z_]*\)(.*/\1/p' | LC_ALL=C sort -u); do
    if ! grep -qE "<${n}[ >/]" "$page" && ! grep -vE "^\{% component ${n}\(" "$kit"/*.html | grep -qE "<${n}[ >/]"; then
      printf 'component %s is rendered nowhere on /kit/\n' "$n"; found=1
    fi
  done
  # 2. every class the kit's sheet styles is emitted by a kit template, literally or as the
  #    stem of a modifier the template composes (`mj-tabs--{{ variant }}`)
  #    (comments are left out: they name classes, `.mj-tone--<tone>`, that are not rules)
  for c in $(tr '\n' ' ' < "$css" | sed 's#/\*[^*]*\*\{1,\}\([^/*][^*]*\*\{1,\}\)*/##g' | grep -oE '\.mj-[a-z0-9-]*[a-z0-9]' | sed 's/^\.//' | LC_ALL=C sort -u); do
    grep -qE "(^|[^a-z0-9-])${c}([^a-z0-9-]|$)" "$kit"/*.html "$page" && continue
    b="${c%--*}"
    [ "$b" != "$c" ] && grep -qF -- "${b}--{{" "$kit"/*.html "$page" && continue
    printf 'kit.css styles .%s, which no kit template emits\n' "$c"; found=1
  done
  # 3. every class a kit template writes literally is defined by a sheet; the stem of a
  #    composed modifier (`mj-tone--{{ tone }}`) is not a class
  for c in $(cat "$kit"/*.html "$page" | grep -oE 'mj-[a-z0-9-]*[a-z0-9](--\{\{)?' | grep -v '{{$' | LC_ALL=C sort -u); do
    case "$c" in mj-tone--*|mj-badge--*|mj-status--*) grep -qF -- ".${c} " "$r/share/design/status.css" && continue ;; esac
    # shellcheck disable=SC2086
    grep -qE "\.${c}([^a-z0-9-]|$)" $sheets && continue
    printf 'a kit template writes the class %s, which no stylesheet defines\n' "$c"; found=1
  done
  # 4. every icon a template names is in the set
  for n in $(cat "$kit"/*.html "$page" | grep -oE '(<icon name|icon|trail|root_icon)="[a-z-]+"|"icon": "[a-z-]+"' | sed -E 's/.*"([a-z-]+)"$/\1/' | LC_ALL=C sort -u); do
    grep -qE "^${n} = '" "$r/site/data/kit/icons.toml" || { printf 'a kit template names the icon %s, which site/data/kit/icons.toml does not hold\n' "$n"; found=1; }
  done
  # 5. with no script nothing is missing: a copy button and a tab strip are rendered hidden,
  #    for site/kit.js to reveal, and never the panels
  grep -qE 'data-copy="[^"]*"[^>]* hidden>' "$kit/base.html" || { printf 'the copy button is not rendered hidden for the script to reveal\n'; found=1; }
  grep -qE 'role="tablist"[^>]* hidden>' "$kit/code.html" || { printf 'the tab strip is not rendered hidden for the script to reveal\n'; found=1; }
  grep -qE 'role="tabpanel"[^>]* hidden' "$kit/code.html" && { printf 'a tab panel is rendered hidden: with no script it could never be read\n'; found=1; }
  # 6. the kit is a page's choice: the base layout and the site's sheet carry none of it, the
  #    page that renders the kit opts in
  grep -qE 'js/kit\.js|kit\.css' "$r/site/templates/base.html" && { printf 'the base layout loads the kit on every page\n'; found=1; }
  grep -qF 'kit.css' "$r/site/tailwind.css" && { printf 'site/tailwind.css compiles the kit into every page'"'"'s sheet\n'; found=1; }
  grep -qF 'partials/kit-assets.html' "$page" || { printf '/kit/ does not load the kit'"'"'s sheet and behaviour\n'; found=1; }
  # 7. a kit page loads one complete sheet: the kit's entry carries every import and plugin of
  #    the site's, so nothing app.css styles is missing on a page that loads kit.css instead
  local d
  for d in $(grep -E '^@(import|plugin) ' "$r/site/tailwind.css" | LC_ALL=C sort -u | tr ' ' '~'); do
    grep -qxF -- "$(printf '%s' "$d" | tr '~' ' ')" "$r/site/kit.tailwind.css" || { printf 'the kit'"'"'s entry lacks %s, which the site'"'"'s entry has\n' "$(printf '%s' "$d" | tr '~' ' ')"; found=1; }
  done
  grep -qE '^@source not ' "$r/site/kit.tailwind.css" && { printf 'the kit'"'"'s entry leaves sources out; its sheet is the whole sheet of a kit page\n'; found=1; }
  [ "$found" = 0 ] || return 10
  printf 'kit: every component rendered, every class emitted and defined, every icon held\n'
}

# --- the repository as it is
expect_exit 0 kit_audit "$ROOT"
expect_grep 'every component rendered'

# --- a fixture of the same files, broken one way at a time
mkdir -p site/templates/kit site/templates/partials site/data/kit share/design
cp "$ROOT"/site/templates/kit/*.html site/templates/kit/
cp "$ROOT"/site/templates/kit-page.html "$ROOT"/site/templates/base.html site/templates/
cp "$ROOT"/site/templates/partials/kit-assets.html site/templates/partials/
cp "$ROOT"/site/data/kit/icons.toml site/data/kit/
cp "$ROOT"/site/tailwind.css "$ROOT"/site/kit.tailwind.css site/
cp "$ROOT"/share/design/kit.css "$ROOT"/share/design/primitives.css "$ROOT"/share/design/status.css share/design/
expect_exit 0 kit_audit "$T"

# 1. a component the page never renders
printf '{%% component orphan(x) %%}<span class="mj-ink">{{ x }}</span>{%% endcomponent orphan %%}\n' >> site/templates/kit/base.html
expect_exit 10 kit_audit "$T"
expect_grep 'component orphan is rendered nowhere on /kit/'
cp "$ROOT"/site/templates/kit/base.html site/templates/kit/

# 2. a class the sheet styles and nothing emits
printf '@layer components { .mj-unworn { color: var(--mj-fg); } }\n' >> share/design/kit.css
expect_exit 10 kit_audit "$T"
expect_grep 'kit.css styles .mj-unworn, which no kit template emits'
cp "$ROOT"/share/design/kit.css share/design/

# 3. a class a template writes and nothing defines
sed -i.bak 's/class="mj-kicker"/class="mj-kicker mj-undefined-thing"/' site/templates/kit/base.html && rm -f site/templates/kit/base.html.bak
expect_exit 10 kit_audit "$T"
expect_grep 'writes the class mj-undefined-thing, which no stylesheet defines'
cp "$ROOT"/site/templates/kit/base.html site/templates/kit/

# 4. an icon the set does not hold
sed -i.bak 's/name="chevron-right"/name="chevron-sideways"/' site/templates/kit/content.html && rm -f site/templates/kit/content.html.bak
expect_exit 10 kit_audit "$T"
expect_grep 'names the icon chevron-sideways'
cp "$ROOT"/site/templates/kit/content.html site/templates/kit/

# 5. a copy button rendered for a page with no script
sed -i.bak 's/ hidden>{{ <icon name="copy"/>{{ <icon name="copy"/' site/templates/kit/base.html && rm -f site/templates/kit/base.html.bak
expect_exit 10 kit_audit "$T"
expect_grep 'copy button is not rendered hidden'
cp "$ROOT"/site/templates/kit/base.html site/templates/kit/

# 7. the kit's entry missing a projection the site's sheet imports
sed -i.bak '/share\/design\/status.css/d' site/kit.tailwind.css && rm -f site/kit.tailwind.css.bak
expect_exit 10 kit_audit "$T"
expect_grep "the kit's entry lacks @import"
cp "$ROOT"/site/kit.tailwind.css site/

# 6. the kit loaded by every page
printf '<script defer src="js/kit.js"></script>\n' >> site/templates/base.html
expect_exit 10 kit_audit "$T"
expect_grep 'the base layout loads the kit on every page'
cp "$ROOT"/site/templates/base.html site/templates/
expect_exit 0 kit_audit "$T"

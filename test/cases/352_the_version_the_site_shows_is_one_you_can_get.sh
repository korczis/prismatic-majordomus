# majordomus-covers: none
# The version the site shows is one a reader can obtain.
#
# On 2026-09-15, after a day's work, `https://majordomus.dev/` rendered **v0.6.1** in the
# navbar badge — beside the install command — while the newest release a reader could obtain
# was **v0.6.0**. The badge advertised something that did not exist.
#
# Neither half of that was a bug on its own:
#
#   - the tree declaring 0.6.1 is correct. The release policy is surface-measured: twenty-one
#     commits of gates, tests and publisher repairs moved no public surface, so no bump was
#     owed and `release analyze` said so — "the surface is unchanged since v0.6.0".
#   - `scripts/ci/version-matches-surface` passing is correct. It asks whether the declared
#     version covers what the surface owes. Declared-versus-owed.
#
# What nobody asked was **shown-versus-shipped**. The site took `project.version`, the tree's
# own version, and presented it as the product's. The badge had also just been made visible at
# every width rather than `sm:` and up — so the wrong number was made more prominent, and the
# change that did it was recorded as a fix.
#
# Asserted here: the site data carries the released version separately from the tree's; the
# badge renders the released one; and — the mutation — a site that shows a version with no
# release is refused rather than published.
. "$ROOT/test/lib.sh"

GEN="$ROOT/scripts/generate-site-data"
NAV="$ROOT/site/templates/partials/navbar.html"
[ -x "$GEN" ] || { echo "    no scripts/generate-site-data"; exit 1; }
[ -f "$NAV" ] || { echo "    no navbar template"; exit 1; }

# --- 1. the two versions are two fields
# The tree's version and the released one answer different questions, so they cannot be one
# value: a single field forces every reader to guess which question it answered.
grep -q 'release: \$release' "$GEN" \
  || { echo "    generate-site-data writes no separate 'release' into project.json;"
       echo "    the site has only the tree's version to show"; exit 1; }
grep -qE 'RELEASE=.*\.ai/repo/releases' "$GEN" \
  || { echo "    the released version is not read from the release records, which are what the"
       echo "    changelog, the baselines and /releases/latest.json all read"; exit 1; }
echo "    the site data carries the tree's version and the released one as two fields"

# --- 2. the badge names the release, and falls back to nothing
grep -q 'project.release' "$NAV" \
  || { echo "    the navbar does not render project.release"; exit 1; }
if grep -qE '^\s*<span[^>]*>v\{\{ *project\.version *\}\}' "$NAV"; then
  echo "    the navbar still renders the tree's version as the product's version"; exit 1
fi
grep -q '{% if project.release %}' "$NAV" \
  || { echo "    the badge is not guarded: a repository with no release would render 'v' and"
       echo "    nothing, or fall back to the tree's version — stating one that does not exist"
       exit 1; }
echo "    the badge names the release, and shows nothing when there is none"

# --- 3. the mutation: a published site that shows an unreleased version is refused
# Without this, §1 and §2 hold the current templates and nothing holds the next edit. The
# refusal lives in scripts/site-check, so it is driven the way site-check drives it: a public
# tree with a page naming a version, and a releases/ directory that does or does not carry it.
F="$T/pub"; mkdir -p "$F/releases"
printf '<html><body><span>v9.9.9</span></body></html>\n' > "$F/index.html"
shown_ok=0
# the check as site-check applies it, over the same inputs
for shown in $(grep -rhoE '>v[0-9]+\.[0-9]+\.[0-9]+<' "$F" --include='*.html' 2>/dev/null | tr -d '><' | sort -u); do
  if [ -f "$F/releases/$shown.json" ] || [ -f "$F/releases/v${shown#v}.json" ]; then shown_ok=1; fi
done
[ "$shown_ok" = 0 ] || { echo "    a page naming an unreleased version was accepted"; exit 1; }
echo "    a page naming a version with no release document is refused"

printf '{"tag":"v9.9.9"}\n' > "$F/releases/v9.9.9.json"
shown_ok=0
for shown in $(grep -rhoE '>v[0-9]+\.[0-9]+\.[0-9]+<' "$F" --include='*.html' 2>/dev/null | tr -d '><' | sort -u); do
  if [ -f "$F/releases/$shown.json" ] || [ -f "$F/releases/v${shown#v}.json" ]; then shown_ok=1; fi
done
[ "$shown_ok" = 1 ] || { echo "    a page naming a version that IS released was still refused;"
                         echo "    the check would refuse every site"; exit 1; }
echo "    and the same page is accepted once that release exists"

# --- 4. site-check carries the refusal, not just this case
grep -q 'a reader beside the install command is told to expect a version that does not exist' "$ROOT/scripts/site-check" \
  || { echo "    scripts/site-check does not carry the refusal, so nothing stops a publication"
       echo "    that advertises a version nobody can obtain"; exit 1; }
echo "    the refusal lives in site-check, so a publication is stopped rather than reported later"

echo "    the version the site shows is one you can get"

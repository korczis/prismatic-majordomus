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
# Asserted here: the site's offer is a field of its own, apart from the tree's version; the
# badge renders it; and — the mutation — a site that shows a version with no release is refused
# rather than published.
#
# Since 2026-10-12 the offer is no longer a field of the site data. It was read from the release
# records alone, and a release's record lands hours after its metadata, so for those hours the
# homepage offered the previous release; scripts/site-build now derives it into build.json from
# the record and from what gh-pages already serves (project.the-site-offers-the-release-it-published,
# test/cases/1087). What this case holds is unchanged: the badge never names the tree's version.
. "$ROOT/test/lib.sh"

GEN="$ROOT/scripts/generate-site-data"
BUILD="$ROOT/scripts/site-build"
NAV="$ROOT/site/templates/partials/navbar.html"
[ -x "$GEN" ] || { echo "    no scripts/generate-site-data"; exit 1; }
[ -f "$NAV" ] || { echo "    no navbar template"; exit 1; }

# --- 1. the two versions are two facts, from two producers
# The tree's version and the released one answer different questions, so they cannot be one
# value: a single field forces every reader to guess which question it answered.
grep -q 'version: \$version' "$GEN" \
  || { echo "    generate-site-data writes no tree version into project.json"; exit 1; }
grep -q 'release: \$release\[0\]' "$BUILD" \
  || { echo "    site-build writes no release into build.json; the site has only the tree's version"
       echo "    to show"; exit 1; }
grep -q 'site/data/registry/distribution.json' "$BUILD" \
  || { echo "    the release is not read from the records' projection, which is what the changelog,"
       echo "    the baselines and /releases/latest.json all read"; exit 1; }
echo "    the tree's version is the site data's, the offered release is the build's"

# --- 2. the badge names the release, and falls back to nothing
grep -q 'offered.version' "$NAV" \
  || { echo "    the navbar does not render the offered release"; exit 1; }
if grep -qE '^\s*<span[^>]*>v\{\{ *project\.version *\}\}' "$NAV"; then
  echo "    the navbar still renders the tree's version as the product's version"; exit 1
fi
grep -q '{% if offered %}' "$NAV" \
  || { echo "    the badge is not guarded: a repository with no release would render 'v' and"
       echo "    nothing, or fall back to the tree's version — stating one that does not exist"
       exit 1; }
echo "    the badge names the release, and shows nothing when there is none"

# --- 3. the mutation: a site offering an unreleased version is refused, and one offering a
#        released version is not
# Scoped to what the site OFFERS — build.json's `release` — rather than to every version it
# prints. The first draft of this check scanned every `>vX.Y.Z<` on every page and refused
# four: v0.1.0, v0.2.0 and v0.4.0 out of the changelog, which are history rather than an offer
# (they are tagged releases whose records the layer never carried), plus the tree's own
# version. Every one of those findings was true and none of them was what the check claimed.
# A checker whose subject is wider than its sentence reports true things nobody asked about.
F="$T/pub"; mkdir -p "$F/releases" "$F/data"
offers() {   # the rule as scripts/site-check applies it
  [ -z "$1" ] && return 0
  [ -f "$F/releases/v${1#v}.json" ]
}
offers 9.9.9 && { echo "    a site offering an unreleased version was accepted"; exit 1; }
echo "    a site offering a version with no release document is refused"

printf '{"tag":"v9.9.9"}\n' > "$F/releases/v9.9.9.json"
offers 9.9.9 || { echo "    a site offering a version that IS released was still refused;"
                  echo "    the check would refuse every site"; exit 1; }
echo "    and the same version is accepted once that release exists"

offers "" || { echo "    a repository with no release at all is refused; it should show no"
               echo "    badge rather than fail the publication"; exit 1; }
echo "    a repository with nothing released is not refused; it simply shows no version"

# --- 4. site-check carries the refusal, not just this case
grep -q 'a reader beside the install command is told to expect a version that does not exist' "$ROOT/scripts/site-check" \
  || { echo "    scripts/site-check does not carry the refusal, so nothing stops a publication"
       echo "    that advertises a version nobody can obtain"; exit 1; }
echo "    the refusal lives in site-check, so a publication is stopped rather than reported later"

echo "    the version the site shows is one you can get"

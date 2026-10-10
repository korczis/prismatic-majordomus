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

# --- 1. the two versions are two fields, and the released one is derived once
# The tree's version and the released one answer different questions, so they cannot be one
# value: a single field forces every reader to guess which question it answered.
grep -q 'release: \$release' "$GEN" \
  || { echo "    generate-site-data writes no separate 'release' into project.json;"
       echo "    the site has only the tree's version to show"; exit 1; }
# Until 2026-10-10 the navbar's release was the highest `v*.yaml` filename by `ls|sort`, with
# no channel and no yanked filter, while the hero, the trust card and /releases/latest.json
# read `Releases::latest_stable`. They agreed only because every record was stable. One
# derivation now: the generator reads the `latest` that `majordomus generate` wrote.
if grep -qE 'ls[^|]*\.ai/repo/releases' "$GEN"; then
  echo "    generate-site-data still lists the release records to choose a version; that is a"
  echo "    second derivation of the latest release beside Releases::latest_stable"; exit 1
fi
grep -qE "RELEASE=.*\.latest\.version" "$GEN" && grep -q 'DIST_DATA=.*site/data/registry/distribution.json' "$GEN" \
  || { echo "    the released version is not read from site/data/registry/distribution.json 'latest',"
       echo "    the derivation the hero badge and /releases/latest.json read"; exit 1; }
"$GEN" --inputs | grep -qx 'site/data/registry/distribution.json' \
  || { echo "    distribution.json is read but not a declared input: a new release record would"
       echo "    not change the generator's fingerprint"; exit 1; }
echo "    the site data carries the tree's version and the released one, read from the one derivation"

# --- 1b. the derivation skips what an installation never resolves to
# Against the real executable: a layer whose highest record is a prerelease and whose next is
# yanked yields, in distribution.json and in /releases/latest.json, the highest stable record
# that is not yanked. The generator copies that value; section 1 proves it reads nothing else.
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
"$MJ" init >/dev/null
awk '/^  deployments: repo\/deployments$/{print; print "  releases: repo/releases"; next} {print}' \
  .ai/manifest.yaml > "$T/.manifest" && mv "$T/.manifest" .ai/manifest.yaml
grep -q '^  releases: repo/releases$' .ai/manifest.yaml \
  || { echo "    the manifest did not gain a releases section; the skeleton's shape changed"; exit 1; }
cat >> .ai/repo/knowledge/sources.yaml <<'Y'

  - id: release
    kind: release-record
    discovery: vcs
    pathspec: ':(glob).ai/repo/releases/*.yaml'
    required: false
Y
mkdir -p .ai/repo/releases
cat > .ai/repo/releases/README.md <<'Y'
---
schema: context/v1
id: ai.repo.releases
kind: context
title: Published releases
description: One record per published release.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 75
---

# Published releases
Y
# A real record as the template, the newest stable one, so its targets are the model's.
OLD="$(jq -r '.latest.version' "$ROOT/site/data/registry/distribution.json")"
REC="$ROOT/.ai/repo/releases/v$OLD.yaml"
[ -f "$REC" ] || { echo "    no record $REC for the checkout's latest release"; exit 1; }
record() {   # VERSION CHANNEL [yanked]
  sed "s/$(printf '%s' "$OLD" | sed 's/\./\\./g')/$1/g; s/^channel: .*/channel: $2/" "$REC" \
    > ".ai/repo/releases/v$1.yaml"
  [ "${3:-}" = yanked ] && printf 'yanked: true\n' >> ".ai/repo/releases/v$1.yaml"
  return 0
}
record 7.1.0 stable; record 7.2.0 stable yanked; record 7.3.0 prerelease
git add -A >/dev/null
"$RB" generate distribution --repo "$T" >/dev/null 2>"$T/.gen.err" \
  || { sed 's/^/      /' "$T/.gen.err"; echo "    majordomus generate distribution failed on the fixture"; exit 1; }
got="$(jq -r '.latest.version // empty' site/data/registry/distribution.json)"
served="$(jq -r '.version // empty' site/static/releases/latest.json)"
[ "$got" = 7.1.0 ] && [ "$served" = 7.1.0 ] \
  || { echo "    with 7.3.0 prerelease, 7.2.0 yanked and 7.1.0 stable, distribution.json says"
       echo "    '$got' and latest.json '$served'; both must be 7.1.0"; exit 1; }
echo "    a prerelease and a yanked record above the newest stable one are not the release"

# --- 1c. this checkout's derived data agree
# project.json is derived from distribution.json, and latest.json from the same records:
# scripts/derive-check keeps them current, so here they must already be one value.
PJ="$(jq -r '.release // empty' "$ROOT/site/data/generated/project.json")"
DL="$(jq -r '.latest.version // empty' "$ROOT/site/data/registry/distribution.json")"
LJ="$(jq -r '.version // empty' "$ROOT/site/static/releases/latest.json" 2>/dev/null)"
[ "$PJ" = "$DL" ] && [ "$DL" = "$LJ" ] \
  || { echo "    project.json release '$PJ', distribution.json latest '$DL' and latest.json '$LJ'"
       echo "    disagree in this checkout"; exit 1; }
echo "    project.json, distribution.json and latest.json name one release ($PJ)"

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

# --- 3. the mutation: a site offering an unreleased version is refused, and one offering a
#        released version is not
# Scoped to what the site OFFERS — `project.release` — rather than to every version string it
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
grep -q 'the badge and the installer disagree on the release' "$ROOT/scripts/site-check" \
  || { echo "    scripts/site-check accepts a shown version that is served but is not the one"
       echo "    /releases/latest.json names, so a yanked or prerelease badge would publish"; exit 1; }
echo "    the refusal lives in site-check, so a publication is stopped rather than reported later"

echo "    the version the site shows is one you can get"

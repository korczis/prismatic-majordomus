# majordomus-covers: init rules update doctor
# majordomus-negative: rules doctor
# The owner's fleet pack reaches an adopter through init, `rules vendor update` and update,
# and carries nothing it does not list and nothing a public tree may not hold (I4336 (b)).
#
#   1  init on an empty fixture, with a pack under XDG_CONFIG_HOME whose applies_to matches,
#      installs exactly the listed files and the stamp; an unlisted file of the pack is never
#      copied, a `visibility: private` entry is skipped for the public fixture, the fleet rule
#      joins the effective set and doctor reports the copy matching its stamp
#   2  a pack file naming a home path, or an absolute one (/opt/...), is refused for a public
#      fixture, and nothing of that pack version is written over the installed copy; the path
#      of a URL is not an absolute path
#   3  a lagging copy (the pack moved on) is a doctor WARN and a watch DRIFT; `rules vendor
#      update` brings it current
#   4  a hand-edited copy is a doctor FAIL that doctor counts and exits on, and the refresh
#      refuses over it
#   4b a stamp row naming a path outside the fleet is a counted doctor FAIL and is never
#      trusted: the forced refresh leaves the adopter's src/ alone and the effective set
#      never loads the file a forged rule row names
#   5  applies_to that does not match installs nothing and says why in one line
#   6  MAJORDOMUS_FLEET=off reads as no pack: update says nothing of it and changes nothing, and
#      the scenario recorder pins it, so recorded evidence is the same with a pack or without
#   7  update with no pack leaves the tree byte for byte as it was
. "$ROOT/test/lib.sh"

export XDG_CONFIG_HOME="$T/xdg"
unset MAJORDOMUS_FLEET_PACK MAJORDOMUS_VISIBILITY 2>/dev/null || true
P="$XDG_CONFIG_HOME/majordomus/fleet"
mkdir -p "$P/rules" "$P/skills/examplekit" "$P/kits"
git remote add origin https://forge.example.test/owner/app.git

cat > "$P/rules/examplekit-use.v1.md" <<'R'
---
id: fleet.examplekit-use
version: 1
kind: rule
title: The UI is built on examplekit
description: Every user interface of the owner's repositories is built on examplekit.
statement: Build every user interface on examplekit, starting from its matching template.
status: active
class: advisory
depends_on: []
tags: [ui]
---

# The UI is built on examplekit

Start from the matching examplekit template; a UI that does not follow it is a defect.
R
cat > "$P/skills/examplekit/SKILL.md" <<'S'
---
schema: skill/v1
id: fleet-examplekit
version: 1
title: Build a UI on examplekit
description: Build or change a user interface on examplekit, starting from the matching template.
status: active
tags: [ui]
inputs:
  - the surface to build and the template it starts from
outputs:
  - markup that follows the template
---

# Purpose

Every user interface starts from the kit's matching template rather than ad-hoc styling.

# Procedure

1. Find the template that matches the surface.
2. Adapt its markup; keep its structure and classes.

# Output

The surface's markup, built on the template.
S
printf 'id: examplekit\ntitle: examplekit\n' > "$P/kits/examplekit.yaml"
printf 'licensed reference markup, never shipped\n' > "$P/reference.md"
printf 'deadbeefdeadbeef\tdeadbeefdeadbeef\tsource.html\n' > "$P/pro-fingerprints.tsv"
pack() { # version, then extra manifest lines
  local v="$1"; shift
  { printf 'schema: ai-fleet-pack/v1\nversion: %s\n' "$v"
    printf 'applies_to:\n  remotes: ["*forge.example.test/owner/*"]\n  paths: []\n'
    printf 'excludes: []\nentries:\n'
    printf '  - { path: rules/examplekit-use.v1.md, kind: rule }\n'
    printf '  - { path: skills/examplekit/SKILL.md, kind: skill }\n'
    printf '  - { path: kits/examplekit.yaml, kind: kit, visibility: private }\n'
    for l in "$@"; do printf '%s\n' "$l"; done
  } > "$P/manifest.yaml"
}
# every file of the fixture but git's own, the pack's and the checkout-local state (the ledger
# records each run), with its content hash
tree_hash() {
  find . -path ./.git -prune -o -path ./xdg -prune -o -path ./.ai/local -prune -o -type f -print | LC_ALL=C sort \
    | while IFS= read -r f; do printf '%s %s\n' "$f" "$(sha256_of_file "$f")"; done | cksum
}
F=.ai/repo/rules/vendor/fleet
SK=.ai/repo/skills/fleet-examplekit/SKILL.md
# doctor, with its exit and its own count of failures; every FAIL it prints is one it counts
doctor() { DOC_RC=0; "$MJ" doctor > doctor.out 2>&1 || DOC_RC=$?
  DOC_SAID="$(sed -n 's/^doctor: \([0-9][0-9]*\) failure(s)$/\1/p' doctor.out)"
  DOC_SEEN="$(grep -c '^FAIL' doctor.out || true)"
  [ "$DOC_SAID" = "$DOC_SEEN" ] \
    || { echo "    doctor printed $DOC_SEEN FAIL line(s) and counted ${DOC_SAID:-none}"; return 1; }
}

# ---------------------------------------------------------------- 1. exactly the listed files
pack 1
expect_exit 0 "$MJ" init || exit 1
expect_grep 'fleet: installed pack v1 into \.ai/repo/rules/vendor/fleet \(2 file\(s\); 1 private entry skipped' || exit 1
got="$( (cd "$F" && find . -type f | LC_ALL=C sort); [ -f "$SK" ] && echo skill)"
want="$(printf './MANIFEST\n./rules/examplekit-use.v1.md\nskill')"
[ "$got" = "$want" ] || { printf '    installed:\n%s\n    want:\n%s\n' "$got" "$want"; exit 1; }
[ ! -e "$F/reference.md" ] && [ ! -e "$F/pro-fingerprints.tsv" ] && [ ! -e "$F/kits" ] \
  || { echo "    an unlisted or private file of the pack was copied"; exit 1; }
grep -rq 'licensed reference markup' .ai && { echo "    the unlisted reference file reached .ai/"; exit 1; }
expect_grep '^# ai-fleet-stamp/v1$' "$F/MANIFEST" || exit 1
expect_grep '^# version: 1$' "$F/MANIFEST" || exit 1
expect_grep "^$(sha256_of_file "$SK")	\\.ai/repo/skills/fleet-examplekit/SKILL\\.md	skill	skills/examplekit/SKILL\\.md$" "$F/MANIFEST" || exit 1
expect_exit 0 "$MJ" rules list || exit 1
expect_grep 'fleet\.examplekit-use' || { echo "    the fleet rule is not in the effective set"; exit 1; }
"$MJ" doctor > doctor.out 2>&1 || true
expect_grep 'OK +rules +\.ai/repo/rules/vendor/fleet — every file matches its stamp \(fleet pack v1\)' doctor.out || exit 1
# a second refresh of the same pack writes nothing
h1="$(tree_hash)"
expect_exit 0 "$MJ" rules vendor update || exit 1
expect_grep 'fleet: pack v1 is current' || exit 1
[ "$(tree_hash)" = "$h1" ] || { echo "    refreshing an unchanged pack changed the tree"; exit 1; }

# ---------------------------------------------------------------- 2. a home path refuses (public)
pack 2
cp "$P/skills/examplekit/SKILL.md" skill.orig
printf '\nThe templates are under ~/licensed/examplekit.\n' >> "$P/skills/examplekit/SKILL.md"
expect_exit 15 "$MJ" rules vendor update || exit 1
expect_grep 'refused skills/examplekit/SKILL.md: carries a home or absolute path' || exit 1
expect_no_grep 'licensed/examplekit' || exit 1
# nothing of v2 is written: the stamp and the copies are v1's
expect_grep '^# version: 1$' "$F/MANIFEST" || exit 1
grep -rq 'licensed/examplekit' .ai && { echo "    a refused pack still wrote into the tree"; exit 1; }
# the line is gone from the pack; everything else stays as it was
cp skill.orig "$P/skills/examplekit/SKILL.md"; rm -f skill.orig
# an absolute path that is no home path refuses as well
cp "$P/rules/examplekit-use.v1.md" rule.orig
printf '\nThe reference copy is /opt/licensed/examplekit/pro/button.html.\n' >> "$P/rules/examplekit-use.v1.md"
expect_exit 15 "$MJ" rules vendor update || exit 1
expect_grep 'refused rules/examplekit-use.v1.md: carries a home or absolute path \(line [0-9]*\)' || exit 1
grep -rq '/opt/licensed' .ai && { echo "    a pack file naming /opt/ reached the tree"; exit 1; }
# the path of a URL is not one: the rule keeps this line through the rest of the case
cp rule.orig "$P/rules/examplekit-use.v1.md"; rm -f rule.orig
printf '\nThe templates are documented at https://docs.example.test/opt/examplekit/.\n' >> "$P/rules/examplekit-use.v1.md"

# ---------------------------------------------------------------- 3. a lagging copy
doctor || exit 1
expect_grep 'WARN rules +\.ai/repo/rules/vendor/fleet — the fleet pack is v2 and this repository holds v1' doctor.out || exit 1
"$MJ" watch > watch.out 2>&1 || true
expect_grep 'DRIFT rules +\.ai/repo/rules/vendor/fleet — the fleet pack is v2' watch.out || exit 1
expect_exit 0 "$MJ" update || exit 1
expect_grep 'fleet: installed pack v2' || exit 1
grep -q 'docs\.example\.test/opt/examplekit' "$F/rules/examplekit-use.v1.md" \
  || { echo "    the rule naming a URL was not installed"; exit 1; }
doctor || exit 1
expect_no_grep 'fleet pack is v2 and' doctor.out || exit 1
base="$DOC_SAID"

# ---------------------------------------------------------------- 4. a hand-edited copy
printf '\nA local addition.\n' >> "$F/rules/examplekit-use.v1.md"
doctor || exit 1
expect_grep 'FAIL rules +\.ai/repo/rules/vendor/fleet — \.ai/repo/rules/vendor/fleet/rules/examplekit-use\.v1\.md differs from its stamp' doctor.out || exit 1
[ "$DOC_SAID" = $((base + 1)) ] && [ "$DOC_RC" != 0 ] \
  || { echo "    the hand edit moved doctor from $base to $DOC_SAID failure(s), exit $DOC_RC"; exit 1; }
expect_exit 15 "$MJ" rules vendor update || exit 1
expect_grep 'installed copy was hand-edited' || exit 1
expect_exit 0 "$MJ" rules vendor update --force || exit 1
doctor || exit 1
expect_no_grep 'differs from its stamp' doctor.out || exit 1

# ---------------------------------------------------------------- 4b. a stamp row is data
mkdir -p src && printf -- '---\nid: src\n---\n' > src/SKILL.md && printf 'fn main() {}\n' > src/lib.rs
cp "$F/MANIFEST" manifest.orig
printf '%s\tsrc/SKILL.md\tskill\tskills/src/SKILL.md\n' "$(sha256_of_file src/SKILL.md)" >> "$F/MANIFEST"
printf '# notes\n' > src/notes.md
printf '%s\tsrc/notes.md\trule\trules/notes.v1.md\n' "$(sha256_of_file src/notes.md)" >> "$F/MANIFEST"
doctor || exit 1
expect_grep 'FAIL rules +\.ai/repo/rules/vendor/fleet — MANIFEST names a path outside the fleet \(src/SKILL\.md, skill\)' doctor.out || exit 1
expect_grep 'FAIL rules +\.ai/repo/rules/vendor/fleet — MANIFEST names a path outside the fleet \(src/notes\.md, rule\)' doctor.out || exit 1
[ "$DOC_SAID" = $((base + 2)) ] && [ "$DOC_RC" != 0 ] \
  || { echo "    the forged rows moved doctor from $base to $DOC_SAID failure(s), exit $DOC_RC"; exit 1; }
expect_exit 0 "$MJ" rules list || exit 1
expect_no_grep 'src/notes' || { echo "    the effective set loaded the file a forged rule row names"; exit 1; }
expect_exit 15 "$MJ" rules vendor update || exit 1
expect_exit 0 "$MJ" rules vendor update --force || exit 1
[ -f src/SKILL.md ] && [ -f src/lib.rs ] && [ -f src/notes.md ] || { echo "    the refresh moved the src/ a forged stamp row named"; exit 1; }
cmp -s "$F/MANIFEST" manifest.orig || { echo "    the forced refresh kept a forged row"; exit 1; }
rm -rf src manifest.orig

# ---------------------------------------------------------------- 5. applies_to does not match
mkdir other && ( cd other && git init -q && git remote add origin https://forge.example.test/someone-else/app.git )
expect_exit 0 "$MJ" --repo other init || exit 1
[ "$(printf '%s\n' "$LAST_OUT" | grep -c '^fleet:')" = 1 ] || { echo "    expected one fleet line: $LAST_OUT"; exit 1; }
expect_grep '^fleet: pack v2 does not apply here \(neither the origin https://forge\.example\.test/someone-else/app\.git' || exit 1
[ ! -e other/.ai/repo/rules/vendor/fleet ] && [ ! -e other/.ai/repo/skills/fleet-examplekit ] \
  || { echo "    a pack that does not apply installed files"; exit 1; }
rm -rf other

# ---------------------------------------------------------------- 6. MAJORDOMUS_FLEET=off: no pack anywhere
# a recorder of published evidence pins the pack off, so a machine holding one records what a
# runner holding none does (I4521): a newer pack that update would install is not seen at all
pack 3
h0="$(tree_hash)"
expect_exit 0 env MAJORDOMUS_FLEET=off "$MJ" update || exit 1
expect_no_grep 'fleet' || exit 1
[ "$(tree_hash)" = "$h0" ] || { echo "    update with the pack off changed the tree"; exit 1; }
expect_grep '^# version: 2$' "$F/MANIFEST" || exit 1
grep -q 'MAJORDOMUS_FLEET=off; export MAJORDOMUS_FLEET' "$ROOT/lib/usecase.sh" \
  || { echo "    the scenario recorder does not pin the fleet pack off"; exit 1; }

# ---------------------------------------------------------------- 7. no pack: update is byte-identical
rm -rf "$XDG_CONFIG_HOME/majordomus"
"$MJ" update >/dev/null 2>&1 || true
h1="$(tree_hash)"
expect_exit 0 "$MJ" update || exit 1
expect_no_grep 'fleet' || exit 1
[ "$(tree_hash)" = "$h1" ] || { echo "    update with no pack changed the tree"; exit 1; }
exit 0

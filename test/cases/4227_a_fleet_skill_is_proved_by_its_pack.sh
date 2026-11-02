# majordomus-covers: skills
# majordomus-negative: skills doctor
# A skill the owner's fleet pack installed is proved by the pack (I4336 (c)). The adopter holds
# `fleet-<id>/SKILL.md` and the install's stamp, .ai/repo/rules/vendor/fleet/MANIFEST, and no
# test or invocation of its own: doctor raises no skill FAIL for the copy the stamp lists with
# the bytes in the tree. The same copy edited, or unlisted, or a skill outside the fleet- prefix
# that the stamp happens to list, owes its proof as before and is the orphan doctor refuses.
# The stamp proves only in the form the install writes: its ai-fleet-stamp/v1 header, the kind
# `skill` and the pack path the fleet-<id> copy came from; a row short of any is no proof.
# Then the one-file check an install runs over a staged pack skill: a valid one passes as the
# fleet-<id> it will become, and each way of breaking skill/v1 is named. Last, the same end to
# end: a real `rules vendor update` from a fixture pack leaves doctor with no skill FAIL, and a
# pack skill that fails skills check refuses the install and leaves the installed copy as it was.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null || skip "jq absent"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN
S="$(mktemp -d "${TMPDIR:-/tmp}/mj4227.XXXXXX")"; trap 'rm -rf "$S"' EXIT
# no pack of the machine's own reaches the fixture until the last section names one
MAJORDOMUS_FLEET_PACK="$S/no-pack"; export MAJORDOMUS_FLEET_PACK
unset MAJORDOMUS_VISIBILITY 2>/dev/null || true
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add -A >/dev/null && git commit -qm base

STAMP=.ai/repo/rules/vendor/fleet/MANIFEST
skill_text() { # id — a skill/v1 file for id, as a pack ships it
  printf -- '---\nschema: skill/v1\nid: %s\nversion: 1\ntitle: Skill %s\ndescription: The %s procedure.\nstatus: active\n---\n# Purpose\n\nWhy.\n\n# Procedure\n\n1. Do it.\n\n# Output\n\nA report.\n' \
    "$1" "$1" "$1"
}
skill() { mkdir -p ".ai/repo/skills/$1"; skill_text "$1" > ".ai/repo/skills/$1/SKILL.md"; }
stamp() { # path... — the install's stamp (lib/fleet.sh), listing exactly these skills as they are now
  local p d
  mkdir -p "$(dirname "$STAMP")"
  { printf '# ai-fleet-stamp/v1\n# version: 1\n'
    for p in "$@"; do
      d="${p%/SKILL.md}"; d="${d##*/}"
      printf '%s\t%s\tskill\tskills/%s/SKILL.md\n' "$(mj_sha256sum "$p" | awk '{ print $1 }')" "$p" "${d#fleet-}"
    done; } > "$STAMP"
}
commit() { git add -A >/dev/null; git commit -qm "$1" >/dev/null; }
doctor() { LAST_OUT="$("$MJ" doctor 2>&1)" || true; }
F=.ai/repo/skills/fleet-examplekit/SKILL.md
# mj_skills_fleet_proved and mj_skills_check_file judge what no command hands them directly (a
# stamp row, a staged file), so they are driven through this probe, as case 282 drives the
# ledger: the function named first, its arguments after
probe="$S/probe.sh"
cat > "$probe" <<PROBE
MJ_BIN_DIR="$ROOT/bin"; MJ_LIB_DIR="$ROOT/lib"; export MJ_BIN_DIR MJ_LIB_DIR
. "$ROOT/lib/common.sh"
mj_require_installed
. "$ROOT/lib/skills.sh"
"\$@"
PROBE

# ---------------------------------------------------------------- stamped: proved by the pack
skill fleet-examplekit
stamp "$F"
commit "a fleet skill, installed and stamped"
expect_exit 0 "$MJ" skills check
expect_grep 'skills: 1 discovered, 1 valid'
doctor
expect_no_grep '^FAIL skill '
expect_grep "^OK +skill +\.ai/repo/skills/fleet-examplekit/SKILL\.md — proved by the fleet pack"
# the Rust verdict is unchanged: the copy names no test of its own, and only the stamp waives that
"$RB" skills verify --format json 2>/dev/null > "$S/v.json" || true
jq -e '[.findings[] | select(.subject == ".ai/repo/skills/fleet-examplekit/SKILL.md") | .code] | index("untested") != null' \
  "$S/v.json" >/dev/null || { echo "    the fixture skill is not an orphan to the executable, so the waiver proves nothing:"; cat "$S/v.json"; exit 1; }

# ---------------------------------------------------------------- edited since the install
printf '\nA local addition.\n' >> "$F"; commit "a hand edit to the copy"
doctor
expect_grep "^FAIL skill +\.ai/repo/skills/fleet-examplekit/SKILL\.md — (untested|unused): orphan"
expect_no_grep 'proved by the fleet pack'

# ---------------------------------------------------------------- not listed by the stamp
skill fleet-examplekit
skill fleet-other
stamp .ai/repo/skills/fleet-other/SKILL.md
commit "fleet-examplekit is not in the stamp"
doctor
expect_grep "^FAIL skill +\.ai/repo/skills/fleet-examplekit/SKILL\.md — (untested|unused): orphan"
expect_grep "^OK +skill +\.ai/repo/skills/fleet-other/SKILL\.md — proved by the fleet pack"
# no stamp at all is the same as an empty one
git rm -q "$STAMP"; commit "no stamp"
doctor
expect_grep "^FAIL skill +\.ai/repo/skills/fleet-other/SKILL\.md — (untested|unused): orphan"

# ---------------------------------------------------------------- outside the prefix
git rm -rq .ai/repo/skills/fleet-examplekit .ai/repo/skills/fleet-other
skill local-review
stamp .ai/repo/skills/local-review/SKILL.md
commit "a stamp listing a skill that is not a fleet skill"
doctor
expect_grep "^FAIL skill +\.ai/repo/skills/local-review/SKILL\.md — (untested|unused): orphan"
expect_no_grep 'proved by the fleet pack'
git rm -rq .ai/repo/skills/local-review

# ---------------------------------------------------------------- only the stamp the install writes
skill fleet-examplekit
sha="$(sha256_of_file "$F")"
raw_stamp() { mkdir -p "$(dirname "$STAMP")"; printf '%s' "$1" > "$STAMP"; }
# the row as the install writes it proves, and is the control for the rows below
raw_stamp "$(printf '# ai-fleet-stamp/v1\n# version: 1\n%s\t%s\tskill\tskills/examplekit/SKILL.md\n' "$sha" "$F")"
expect_exit 0 bash "$probe" mj_skills_fleet_proved "$F"
# no kind and no pack path: a row added by hand
raw_stamp "$(printf '# ai-fleet-stamp/v1\n# version: 1\n%s\t%s\n' "$sha" "$F")"
expect_exit 1 bash "$probe" mj_skills_fleet_proved "$F"
commit "a two-column stamp row"
doctor
expect_grep "^FAIL skill +\.ai/repo/skills/fleet-examplekit/SKILL\.md — (untested|unused): orphan"
expect_no_grep 'proved by the fleet pack'
# listed as another kind
raw_stamp "$(printf '# ai-fleet-stamp/v1\n# version: 1\n%s\t%s\trule\tskills/examplekit/SKILL.md\n' "$sha" "$F")"
expect_exit 1 bash "$probe" mj_skills_fleet_proved "$F"
# installed from another pack path than the fleet-<id> it stands in
raw_stamp "$(printf '# ai-fleet-stamp/v1\n# version: 1\n%s\t%s\tskill\tskills/other/SKILL.md\n' "$sha" "$F")"
expect_exit 1 bash "$probe" mj_skills_fleet_proved "$F"
# the right row under a file that is not an ai-fleet-stamp/v1 stamp
raw_stamp "$(printf '# version: 1\n%s\t%s\tskill\tskills/examplekit/SKILL.md\n' "$sha" "$F")"
expect_exit 1 bash "$probe" mj_skills_fleet_proved "$F"
git rm -rqf --ignore-unmatch "$STAMP" .ai/repo/skills/fleet-examplekit; rm -rf "$STAMP" .ai/repo/skills/fleet-examplekit
commit "no fleet skill and no stamp"

# ---------------------------------------------------------------- the one-file check
# mj_skills_check_file is what an install calls on a staged pack skill
mkdir -p "$S/pack/skills/examplekit"
P="$S/pack/skills/examplekit/SKILL.md"
skill_text fleet-examplekit > "$P"
expect_exit 0 bash "$probe" mj_skills_check_file "$P" fleet-examplekit
# judged as the directory it will be installed into, not the one it is staged in
expect_exit 1 bash "$probe" mj_skills_check_file "$P"
expect_grep 'id "fleet-examplekit" does not match the directory name "examplekit"'
skill_text examplekit > "$P"
expect_exit 1 bash "$probe" mj_skills_check_file "$P" fleet-examplekit
expect_grep 'id "examplekit" does not match the directory name "fleet-examplekit"'
skill_text fleet-examplekit | sed '/^# Output$/,$d' > "$P"
expect_exit 1 bash "$probe" mj_skills_check_file "$P" fleet-examplekit
expect_grep 'body lacks a non-empty section for: Output'
skill_text fleet-examplekit | awk '{ print } /^status: active$/ { print "kit: examplekit" }' > "$P"
expect_exit 1 bash "$probe" mj_skills_check_file "$P" fleet-examplekit
expect_grep 'unknown front-matter key\(s\): kit'
printf '# no front matter\n' > "$P"
expect_exit 1 bash "$probe" mj_skills_check_file "$P" fleet-examplekit
expect_grep '^no front matter$'
expect_exit 1 bash "$probe" mj_skills_check_file "$S/pack/skills/absent/SKILL.md" fleet-absent
expect_grep '^no file to check$'

# ---------------------------------------------------------------- end to end, from a pack
# The stamp the real install writes is the one the waiver reads: nothing above is hand-made
# here. The pack applies to this fixture through its origin.
git remote add origin https://forge.example.test/owner/app.git
manifest() { # version
  printf 'schema: ai-fleet-pack/v1\nversion: %s\napplies_to:\n  remotes: ["*forge.example.test/owner/*"]\n  paths: []\nexcludes: []\nentries:\n  - { path: skills/examplekit/SKILL.md, kind: skill }\n' \
    "$1" > "$S/pack/manifest.yaml"
}
skill_text fleet-examplekit > "$P"; manifest 1
MAJORDOMUS_FLEET_PACK="$S/pack"; export MAJORDOMUS_FLEET_PACK
expect_exit 0 "$MJ" rules vendor update
expect_grep 'fleet: installed pack v1 into \.ai/repo/rules/vendor/fleet \(1 file\(s\)\)'
cmp -s "$P" "$F" || { echo "    the installed copy is not the pack's bytes"; exit 1; }
commit "the fleet pack, installed"
doctor
expect_no_grep '^FAIL skill '
expect_grep "^OK +skill +\.ai/repo/skills/fleet-examplekit/SKILL\.md — proved by the fleet pack"
# the pack moves on to a skill that fails skills check: the install refuses it, writes nothing
cp "$F" "$S/installed.md"; cp "$STAMP" "$S/stamp.before"
skill_text fleet-examplekit | sed '/^# Output$/,$d' > "$P"; manifest 2
expect_exit 15 "$MJ" rules vendor update
expect_grep 'refused skills/examplekit/SKILL\.md: fails skills check \(body lacks a non-empty section for: Output\)'
cmp -s "$S/installed.md" "$F" && cmp -s "$S/stamp.before" "$STAMP" \
  || { echo "    a refused install changed the installed copy or its stamp"; exit 1; }
doctor
expect_no_grep '^FAIL skill '

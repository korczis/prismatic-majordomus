# majordomus-covers: none
# A layer names the tool version it was written for, and doctor tells version skew from
# defects (I1980). On OSCILLA, installing a newer tool machine-wide turned doctor red with
# 433 failures the upgrade had introduced, and its pre-commit hook refused every commit
# with nothing saying the cause was the version. This case holds:
#
#   1  init writes `tool:` into .ai/manifest.yaml, this tool's major.minor
#   2  a layer written for an older tool, with a failing finding, makes doctor say so first
#      (WARN layer ... version skew) and exit 11, not 10, naming `majordomus migrate --layer`
#   3  `migrate --layer` advances `tool:` to this version and changes nothing else; doctor
#      then exits 10 on the same finding, which is now a defect of the layer
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_BIN="$RB"

mine="$("$MJ" version | sed -n 's/^majordomus \([0-9]*\)\.\([0-9]*\)\..*/\1.\2/p')"
[ -n "$mine" ] || { echo "    'majordomus version' named no version"; exit 1; }

# ---------------------------------------------------------------- 1. init names the tool
# update writes the projections: a missing one is exit 12, which outranks every failure
"$MJ" init >/dev/null; "$MJ" update >/dev/null
grep -qx "tool: \"$mine\"" .ai/manifest.yaml \
  || { echo "    init did not write tool: \"$mine\" into the manifest:"; head -5 .ai/manifest.yaml; exit 1; }
git add -A >/dev/null && git commit -qm "a fresh layer"

# ---------------------------------------------------------------- 2. an older layer is skew
printf 'a_key_no_version_reads: 1\n' >> .ai/repo/policy.yaml
sed 's/^tool:.*/tool: "0.1"/' .ai/manifest.yaml > manifest.tmp && cat manifest.tmp > .ai/manifest.yaml && rm -f manifest.tmp
expect_exit 11 "$MJ" doctor
expect_grep '^WARN layer +version .*written for majordomus 0\.1, read by '"$mine"
expect_grep '^FAIL policy .*unknown keys: a_key_no_version_reads'
expect_grep '^doctor: version skew: .*majordomus migrate --layer'

# ---------------------------------------------------------------- 3. migrate --layer ends it
before="$(grep -v '^tool:' .ai/manifest.yaml)"
expect_exit 0 "$MJ" migrate --layer
expect_grep "^layer: tool 0\.1 -> $mine in \.ai/manifest\.yaml"
grep -qx "tool: \"$mine\"" .ai/manifest.yaml || { echo "    migrate --layer did not advance tool:"; head -5 .ai/manifest.yaml; exit 1; }
[ "$(grep -v '^tool:' .ai/manifest.yaml)" = "$before" ] \
  || { echo "    migrate --layer changed more of the manifest than tool:"; exit 1; }
expect_exit 0 "$MJ" migrate --layer
expect_grep "^layer: already written for majordomus $mine"
expect_exit 10 "$MJ" doctor
expect_grep '^OK +layer +version .*this tool.s version'
expect_grep '^FAIL policy .*unknown keys: a_key_no_version_reads'

echo "    init writes tool: $mine; an older layer's failure exits 11 as version skew naming migrate --layer; after it, the same failure exits 10"

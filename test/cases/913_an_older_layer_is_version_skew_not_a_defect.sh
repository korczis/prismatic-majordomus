# majordomus-covers: doctor update
# A layer names the tool version it was written for (`written_for` in .ai/manifest.yaml), and
# doctor reads an older one as version skew, not as a defect (I1980).
#
# Installing a newer tool machine-wide used to turn an adopter's doctor red: a newer release
# judges the layer by rules it added, and OSCILLA's pre-commit hook, which runs doctor, then
# refused every commit, its peers' included, until the layer was migrated by hand, with
# nothing saying that the upgrade was the cause. An older layer, or one written before the
# key existed, is now a warning naming `majordomus update`: a pre-commit hook does not refuse
# it, and update records the version the layer was brought up to.
. "$ROOT/test/lib.sh"

"$MJ" init >/dev/null
"$MJ" update >/dev/null
for h in pre-commit:doctor pre-push:'finish --check'; do
  printf '#!/bin/sh\n%s %s || exit $?\n' "$MJ" "${h#*:}" > ".git/hooks/${h%%:*}"
  chmod +x ".git/hooks/${h%%:*}"
done
tool="$("$MJ" version | awk '{print $2}')"
written() { sed -n 's/^written_for: *"\{0,1\}\([^"]*\)"\{0,1\} *$/\1/p' .ai/manifest.yaml; }

echo "    init wrote the version the layer is written for: this executable's"
[ "$(written)" = "$tool" ] || { echo "    written_for is '$(written)', not $tool"; exit 1; }
expect_exit 0 "$MJ" doctor
expect_grep "OK +layout +\.ai/manifest\.yaml — written for majordomus $tool, this executable"

echo "    an older layer is skew: a warning naming update, and the exit a hook passes"
sed -i.bak 's/^written_for: .*/written_for: "0.0.0"/' .ai/manifest.yaml && rm -f .ai/manifest.yaml.bak
expect_exit 0 "$MJ" doctor
expect_grep "WARN layout +\.ai/manifest\.yaml — written for majordomus 0\.0\.0, older than this executable \($tool\): a finding this version adds may be version skew"
expect_grep 'reproduce: majordomus update'
git add -A >/dev/null && git commit -qm "an older layer commits" || { echo "    the pre-commit hook refused an older layer"; exit 1; }

echo "    a layer written before the key existed reads the same way"
sed -i.bak '/^written_for:/d' .ai/manifest.yaml && rm -f .ai/manifest.yaml.bak
expect_exit 0 "$MJ" doctor
expect_grep 'WARN layout +\.ai/manifest\.yaml — names no tool version \(written_for\)'

echo "    update says it would stamp the layer, writes nothing on a dry run, and then stamps it"
expect_exit 0 "$MJ" update --dry-run
expect_grep "stamp \.ai/manifest\.yaml written_for \(none\) -> $tool"
[ -z "$(written)" ] || { echo "    a dry run wrote written_for"; exit 1; }
expect_exit 0 "$MJ" update
[ "$(written)" = "$tool" ] || { echo "    update left written_for at '$(written)'"; exit 1; }
# inserted after the schema line, every other line kept
[ "$(sed -n '/^schema:/{n;p;}' .ai/manifest.yaml)" = "written_for: \"$tool\"" ] \
  || { echo "    the stamp is not beside the schema:"; cat .ai/manifest.yaml; exit 1; }
expect_exit 0 "$MJ" doctor
expect_grep "OK +layout +\.ai/manifest\.yaml — written for majordomus $tool, this executable"
expect_exit 0 "$MJ" update
expect_no_grep '^stamp '

echo "    a value that is not a version is a defect of the manifest"
sed -i.bak 's/^written_for: .*/written_for: "soon"/' .ai/manifest.yaml && rm -f .ai/manifest.yaml.bak
expect_exit 10 "$MJ" doctor
expect_grep "FAIL layout +\.ai/manifest\.yaml — written_for 'soon' is not a version"

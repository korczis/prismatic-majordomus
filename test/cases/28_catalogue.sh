# The catalogue describes the tool in terms of the tool, so every reference it makes is
# checked. Each mutation below breaks one kind of reference and must turn doctor red;
# each asserts the probe took before asserting what it caused, because a sed that matches
# nothing is silent and would leave the case proving that a no-op causes no failure.
. "$ROOT/test/lib.sh"
mkdir -p "$T/tool"
cp -R "$ROOT/bin" "$ROOT/lib" "$ROOT/share" "$ROOT/docs" "$T/tool/"
cp -R "$ROOT/test" "$T/tool/"
mkdir -p "$T/tool/.github/workflows"; cp "$ROOT/.github/workflows/validate.yml" "$T/tool/.github/workflows/"
MJ="$T/tool/bin/majordomus"
UC="$T/tool/share/use-cases.yaml"; AP="$T/tool/share/applications.yaml"
cp "$UC" "$UC.orig"; cp "$AP" "$AP.orig"
took() { grep -qF -- "$2" "$1" || { printf '    the probe did not take: %s absent from %s\n' "$2" "$1"; exit 1; }; }
restore() { cp "$UC.orig" "$UC"; cp "$AP.orig" "$AP"; }
wire() {
  mkdir -p .git/hooks
  printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .git/hooks/pre-commit
  printf '#!/usr/bin/env bash\nmajordomus finish --check\n' > .git/hooks/pre-push
  chmod +x .git/hooks/pre-commit .git/hooks/pre-push
  PATH="$(dirname "$MJ"):$PATH"; export PATH
}
"$MJ" init >/dev/null; "$MJ" update >/dev/null; wire

# healthy: the counts are derived, and the line says both directions were checked
expect_exit 0 "$MJ" doctor
expect_grep 'OK +catalogue +[0-9]+ use case\(s\), [0-9]+ application\(s\) .* both directions'

# 1. a step naming a command the binary does not dispatch
sed 's/^      - command: init$/      - command: no_such_command/' "$UC.orig" > "$UC"
took "$UC" "command: no_such_command"
expect_exit 10 "$MJ" doctor
expect_grep "FAIL catalogue .* names command 'no_such_command', which bin/majordomus does not dispatch"
restore

# 2. a rule that is not in the doctrine registry
sed 's/majordomus.scope-integrity, majordomus.state-consistency/majordomus.scope-integrity, no_such_doctrine/' "$UC.orig" > "$UC"
took "$UC" "no_such_doctrine"
expect_exit 10 "$MJ" doctor
expect_grep "FAIL catalogue .* names doctrine 'no_such_doctrine', which is not in the registry"
restore

# 3. a promise that is not a claim
sed 's/claims: \[region-projection,/claims: [invented-claim,/' "$UC.orig" > "$UC"
took "$UC" "invented-claim"
expect_exit 10 "$MJ" doctor
expect_grep "FAIL catalogue .* names claim 'invented-claim', which is not in docs/CLAIMS.yaml"
restore

# 4. a cross-reference to an application that does not exist
sed 's/applications: \[repository-with-authored-governance\]/applications: [no-such-application]/' "$UC.orig" > "$UC"
took "$UC" "no-such-application"
expect_exit 10 "$MJ" doctor
expect_grep "FAIL catalogue .* names application 'no-such-application', which does not exist"
restore

# 5. a one-sided cross-reference: the application exists but does not name it back.
#    This is the failure a single-direction check would miss entirely.
sed 's/^    use_cases: \[adopt-an-existing-repository, prove-a-rule-is-enforced, find-out-what-drifted\]$/    use_cases: [prove-a-rule-is-enforced, find-out-what-drifted]/' "$AP.orig" > "$AP"
grep -q 'use_cases: \[prove-a-rule-is-enforced, find-out-what-drifted\]' "$AP" || { echo "    the probe did not take: application use_cases unchanged"; exit 1; }
expect_exit 10 "$MJ" doctor
expect_grep "FAIL catalogue .* does not name it back"
restore

# 6. an application that declares only what it fits
awk '/^    does_not_fit_when:$/{skip=1; next} skip && /^      - /{next} {skip=0; print}' "$AP.orig" > "$AP"
grep -qE '^    does_not_fit_when:' "$AP" && { echo "    the probe did not take: a does_not_fit_when key is still present"; exit 1; }
expect_exit 10 "$MJ" doctor
expect_grep "FAIL catalogue .* declares no does_not_fit_when"
restore

# healthy again after every mutation is reverted
expect_exit 0 "$MJ" doctor

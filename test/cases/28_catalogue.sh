# The catalogue describes the tool in terms of the tool, so every reference it makes is
# checked. The catalogue is the repository's own: its use cases and applications under
# .ai/repo/, one file each. Each mutation below breaks one kind of reference and must turn
# doctor red; each asserts the probe took before asserting what it caused, because a sed
# that matches nothing is silent and would leave the case proving that a no-op causes no
# failure.
. "$ROOT/test/lib.sh"
mkdir -p "$T/tool"
cp -R "$ROOT/bin" "$ROOT/lib" "$ROOT/share" "$ROOT/docs" "$T/tool/"
cp -R "$ROOT/test" "$T/tool/"
mkdir -p "$T/tool/.github/workflows"; cp "$ROOT/.github/workflows/validate.yml" "$T/tool/.github/workflows/"
MJ="$T/tool/bin/majordomus"
wire() {
  mkdir -p .git/hooks
  printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .git/hooks/pre-commit
  printf '#!/usr/bin/env bash\nmajordomus finish --check\n' > .git/hooks/pre-push
  chmod +x .git/hooks/pre-commit .git/hooks/pre-push
  PATH="$(dirname "$MJ"):$PATH"; export PATH
}
"$MJ" init >/dev/null; "$MJ" update >/dev/null; wire
# this repository's own catalogue, and what it references: the claims, the responsibilities
# and the executable's registry (for the MCP tools)
cp "$ROOT"/.ai/repo/use-cases/*.md "$ROOT"/.ai/repo/use-cases/taxonomy.yaml .ai/repo/use-cases/
cp "$ROOT"/.ai/repo/applications/*.md .ai/repo/applications/
mkdir -p docs/generated
cp "$ROOT/docs/CLAIMS.yaml" "$ROOT/docs/RESPONSIBILITIES.yaml" docs/
cp "$ROOT/docs/generated/registry.json" docs/generated/
git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm catalogue
UC=.ai/repo/use-cases/adopt-an-existing-repository.md
AP=.ai/repo/applications/repository-with-authored-governance.md
cp "$UC" "$T/uc.orig"; cp "$AP" "$T/ap.orig"
took() { grep -qF -- "$2" "$1" || { printf '    the probe did not take: %s absent from %s\n' "$2" "$1"; exit 1; }; }
restore() { cp "$T/uc.orig" "$UC"; cp "$T/ap.orig" "$AP"; }

# healthy: the counts are derived, and the line says both directions were checked
expect_exit 0 "$MJ" doctor
expect_grep 'OK +catalogue +[0-9]+ use case\(s\), [0-9]+ application\(s\) .* both directions'

# 1. a command the binary does not dispatch
sed 's/^commands: \[init, /commands: [no_such_command, /' "$T/uc.orig" > "$UC"
took "$UC" "no_such_command"
expect_exit 10 "$MJ" doctor
expect_grep "FAIL use-case .* names command 'no_such_command', which bin/majordomus does not dispatch"
restore

# 2. a rule that is not in the doctrine registry
sed 's/majordomus.projection-integrity, /no_such_doctrine, /' "$T/uc.orig" > "$UC"
took "$UC" "no_such_doctrine"
expect_exit 10 "$MJ" doctor
expect_grep "FAIL use-case .* names doctrine 'no_such_doctrine', which no rule declares"
restore

# 3. a promise that is not a claim
sed 's/claims: \[region-projection,/claims: [invented-claim,/' "$T/uc.orig" > "$UC"
took "$UC" "invented-claim"
expect_exit 10 "$MJ" doctor
expect_grep "FAIL use-case .* names claim 'invented-claim', which docs/CLAIMS.yaml does not have"
restore

# 4. a cross-reference to an application that does not exist
sed 's/applications: \[repository-with-authored-governance\]/applications: [no-such-application]/' "$T/uc.orig" > "$UC"
took "$UC" "no-such-application"
expect_exit 10 "$MJ" doctor
expect_grep "FAIL use-case .* names application 'no-such-application', which does not exist"
restore

# 5. a one-sided cross-reference: the application exists but does not name it back.
#    This is the failure a single-direction check would miss entirely.
sed -E '/^use_cases: \[/ s/adopt-an-existing-repository, //' "$T/ap.orig" > "$AP"
grep -qE '^use_cases: .*adopt-an-existing-repository' "$AP" && { echo "    the probe did not take: application use_cases unchanged"; exit 1; }
expect_exit 10 "$MJ" doctor
expect_grep "FAIL use-case .* does not name it back"
restore

# 6. an application that declares only what it fits
awk '/^does_not_fit_when:$/{skip=1; next} skip && /^  - /{next} {skip=0; print}' "$T/ap.orig" > "$AP"
grep -qE '^does_not_fit_when:' "$AP" && { echo "    the probe did not take: a does_not_fit_when key is still present"; exit 1; }
expect_exit 10 "$MJ" doctor
expect_grep "FAIL use-case .* declares no does_not_fit_when"
restore

# 7. a category the taxonomy does not declare, and a scenario step that runs a command the
#    use case does not list: both are the catalogue's, both are refused by name
sed 's/^category: adoption$/category: nowhere/' "$T/uc.orig" > "$UC"
took "$UC" "category: nowhere"
expect_exit 10 "$MJ" doctor
expect_grep "FAIL use-case .* category 'nowhere' is not in taxonomy.yaml"
restore
sed "s/run: \['doctor'\]/run: ['watch']/" "$T/uc.orig" > "$UC"
took "$UC" "run: ['watch']"
expect_exit 10 "$MJ" doctor
expect_grep "FAIL use-case .* runs 'watch', which the use case does not list under commands"
restore

# healthy again after every mutation is reverted
expect_exit 0 "$MJ" doctor

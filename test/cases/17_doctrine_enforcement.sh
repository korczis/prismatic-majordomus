# majordomus-covers: check finish
# majordomus-negative: check finish
# A doctrine's class is not a label: it decides whether the command stops. Blocking
# doctrines refuse; the one advisory doctrine reports and lets the command pass. A
# blocker refuses completion and permits `blocked`.
. "$ROOT/test/lib.sh"
# The mutations below rewrite the tool itself, so the case runs against its own copy.
mkdir -p "$T/tool" "$T/tool/.github/workflows"
fixture_repo "$T/tool" docs
cp -R "$ROOT/test" "$T/tool/"
cp "$ROOT/.github/workflows/validate.yml" "$T/tool/.github/workflows/"
MJ="$T/tool/bin/majordomus"
LIBD="$T/tool/lib"
CI="$T/tool/.github/workflows/validate.yml"; RUN="$T/tool/test/run.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
# the registry is the vendored rule package of the repository under test; a mutation edits
# one rule file there and a restore puts the original back
RULES="$T/.ai/repo/rules/vendor/majordomus/rules"
cp -R "$RULES" "$T/rules.orig"
restore() {
  cp "$T/rules.orig"/*.md "$RULES/"
  cp "$ROOT/lib/check.sh" "$LIBD/check.sh"
  cp "$ROOT/.github/workflows/validate.yml" "$CI"
  cp "$ROOT/test/run.sh" "$RUN"
}
mkdir -p lib test && echo a > lib/a && echo t > test/a_test && git add . && git commit -qm base

# --- blocking: an unresolved question refuses completion (blocker_resolution)
"$MJ" start "t1" --scope lib,test --profile implementation >/dev/null
echo b >> lib/a
"$MJ" question add "is this the right table?" >/dev/null
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null
expect_exit 10 "$MJ" finish --outcome completed --verify-command "true"
expect_grep 'FAIL blockers'
expect_grep 'blocking doctrines:'
expect_grep 'majordomus.blocker-resolution'
# the same open question is expected when the outcome is blocked, and does not refuse
expect_exit 0 "$MJ" finish --outcome blocked
expect_grep 'INFO blockers .* outcome is blocked'
expect_grep '^outcome: blocked$' .ai/local/state/current.yaml
# answered before the next section, which is about the advisory class: an unresolved
# question blocks every later task on this branch, and would mask what follows
"$MJ" question resolve "right table" --answer "yes, sessions" >/dev/null

# --- advisory: a stale checkpoint is reported by check and does not fail it
"$MJ" start "t2" --scope lib --profile routine >/dev/null
sed -i.bak 's/^checkpoint_at: .*/checkpoint_at: 2020-01-01T00:00:00Z/' .ai/local/state/current.yaml
rm -f .ai/local/state/current.yaml.bak
expect_exit 0 "$MJ" check
expect_grep 'WARN checkpoint'
expect_no_grep '^FAIL'

# --- the class is what does it: flip checkpoint_freshness to blocking and check fails
sed 's/^class: advisory$/class: blocking/' "$T/rules.orig/checkpoint-freshness.v1.md" > "$RULES/checkpoint-freshness.v1.md"
expect_exit 10 "$MJ" check
expect_grep 'FAIL checkpoint'
restore
expect_exit 0 "$MJ" check


# --- a doctrine whose validator does not exist is a failure, never a silent skip
sed 's/^  validator: scope$/  validator: scope_that_does_not_exist/' "$T/rules.orig/scope-integrity.v1.md" > "$RULES/scope-integrity.v1.md"
expect_exit 10 "$MJ" check
expect_grep 'FAIL doctrine .* no function mj_validate_scope_that_does_not_exist exists'
restore

# --- an unknown class fails closed rather than defaulting to advisory: the rule set does
# not resolve, and nothing is enforced partially
sed 's/^class: blocking$/class: informational/' "$T/rules.orig/scope-integrity.v1.md" > "$RULES/scope-integrity.v1.md"
expect_exit 10 "$MJ" check
expect_grep "rules do not resolve.*class 'informational' is neither blocking nor advisory"
restore

# --- the policy may not name a finish requirement no doctrine defines
NOTE="$(mktemp "${TMPDIR:-/tmp}/mj.note.XXXXXX")"; printf '# Reason\nnothing to find\n' > "$NOTE"
expect_exit 0 "$MJ" finish --outcome no_match --note "$NOTE"
"$MJ" start "t3" --scope lib --profile routine >/dev/null
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null
sed -i.bak 's/^    - note_present$/    - note_present\n    - invented_requirement/' .ai/repo/policy.yaml
rm -f .ai/repo/policy.yaml.bak
expect_exit 10 "$MJ" finish --outcome completed --verify-command "true"
expect_grep "FAIL contract .* 'invented_requirement', which no doctrine defines"

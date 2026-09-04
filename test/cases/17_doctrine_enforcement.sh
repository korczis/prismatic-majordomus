# A doctrine's class is not a label: it decides whether the command stops. Blocking
# doctrines refuse; the one advisory doctrine reports and lets the command pass. A
# blocker refuses completion and permits `blocked`.
. "$ROOT/test/lib.sh"
# The mutations below rewrite the tool itself, so the case runs against its own copy.
mkdir -p "$T/tool" "$T/tool/.github/workflows"
cp -R "$ROOT/bin" "$ROOT/lib" "$ROOT/share" "$ROOT/docs" "$T/tool/"
cp -R "$ROOT/test" "$T/tool/"
cp "$ROOT/.github/workflows/validate.yml" "$T/tool/.github/workflows/"
MJ="$T/tool/bin/majordomus"
REG="$T/tool/share/doctrines.yaml"; LIBD="$T/tool/lib"
CI="$T/tool/.github/workflows/validate.yml"; RUN="$T/tool/test/run.sh"
cp "$REG" "$REG.orig"
restore() {
  cp "$REG.orig" "$REG"
  cp "$ROOT/lib/check.sh" "$LIBD/check.sh"
  cp "$ROOT/.github/workflows/validate.yml" "$CI"
  cp "$ROOT/test/run.sh" "$RUN"
}
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib test && echo a > lib/a && echo t > test/a_test && git add . && git commit -qm base

# --- blocking: an unresolved question refuses completion (blocker_resolution)
"$MJ" start "t1" --scope lib,test --profile implementation >/dev/null
id="$(grep '^id:' .majordomus/state/current.yaml | awk '{print $2}')"
echo b >> lib/a
printf -- '- [unresolved] %s — is this the right table?\n' "$id" >> .majordomus/state/open-questions.md
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null
expect_exit 10 "$MJ" finish --outcome completed --verify-command "true"
expect_grep 'FAIL blockers'
expect_grep 'blocking doctrines:'
expect_grep 'blocker_resolution'
# the same open question is expected when the outcome is blocked, and does not refuse
expect_exit 0 "$MJ" finish --outcome blocked
expect_grep 'INFO blockers .* outcome is blocked'
expect_grep '^outcome: blocked$' .majordomus/state/current.yaml

# --- advisory: a stale checkpoint is reported by check and does not fail it
"$MJ" start "t2" --scope lib --profile routine >/dev/null
sed -i.bak 's/^checkpoint_at: .*/checkpoint_at: 2020-01-01T00:00:00Z/' .majordomus/state/current.yaml
rm -f .majordomus/state/current.yaml.bak
expect_exit 0 "$MJ" check
expect_grep 'WARN checkpoint'
expect_no_grep '^FAIL'

# --- the class is what does it: flip checkpoint_freshness to blocking and check fails
awk '/^  - id: checkpoint_freshness$/{f=1} f&&/^    class: advisory$/{sub(/advisory/,"blocking"); f=0} {print}' "$REG.orig" > "$REG"
expect_exit 10 "$MJ" check
expect_grep 'FAIL checkpoint'
cp "$REG.orig" "$REG"
expect_exit 0 "$MJ" check


# --- a doctrine whose validator does not exist is a failure, never a silent skip
awk '/^  - id: scope_integrity$/{f=1} f&&/^    validator: scope$/{sub(/scope$/,"scope_that_does_not_exist"); f=0} {print}' "$REG.orig" > "$REG"
expect_exit 10 "$MJ" check
expect_grep 'FAIL doctrine .* no function mj_validate_scope_that_does_not_exist exists'
cp "$REG.orig" "$REG"

# --- an unknown class fails closed rather than defaulting to advisory
awk '/^  - id: scope_integrity$/{f=1} f&&/^    class: blocking$/{sub(/blocking/,"informational"); f=0} {print}' "$REG.orig" > "$REG"
expect_exit 10 "$MJ" check
expect_grep "FAIL doctrine .* unknown class 'informational'"
cp "$REG.orig" "$REG"

# --- the policy may not name a finish requirement no doctrine defines
NOTE="$(mktemp "${TMPDIR:-/tmp}/mj.note.XXXXXX")"; printf '# Reason\nnothing to find\n' > "$NOTE"
expect_exit 0 "$MJ" finish --outcome no_match --note "$NOTE"
"$MJ" start "t3" --scope lib --profile routine >/dev/null
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null
sed -i.bak 's/^    - note_present$/    - note_present\n    - invented_requirement/' .majordomus/policy.yaml
rm -f .majordomus/policy.yaml.bak
expect_exit 10 "$MJ" finish --outcome completed --verify-command "true"
expect_grep "FAIL contract .* 'invented_requirement', which no doctrine defines"

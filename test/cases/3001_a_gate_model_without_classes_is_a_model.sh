# majordomus-covers: script:scripts/ci-plan capability:gates.model capability:gates.completion
# A CI model that declares gates and no `classes` is a model: the absent key is the empty
# list, in both readers of `.ai/repo/ci/gates.yaml`.
#
# A repository adopting the layer wrote its first model as a list of nine gates and nothing
# else. The executable answered `.ai/repo/ci/gates.yaml does not parse: missing field
# `classes`` and counted zero gates; `scripts/ci-plan` refused the same file with `declares
# no classes`. Neither is true of the file. A model with no class is the first thing anyone
# writes, and it has a meaning the planner's own rules already give it:
#
#   no changed path        only the gates marked `always` (the rule for an empty change)
#   a changed path         no class covers it, so it is unclassified, the plan names it and
#                          escalates, exactly as an unclassified path does under any model
#
# Nothing here is a rule for "no classes". It is the existing rules over an empty list, and
# in particular an absent `classes` does not mean "every gate applies": with nothing changed
# the gate that is not `always` is left out, and says why.
#
# The two readers must agree on which gates a change selects under such a model, and the
# shell half that records a gate run (lib/gates.sh) must take it too.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"
PLAN="$ROOT/scripts/ci-plan"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj3001.XXXXXX")"; trap 'rm -rf "$S"' EXIT
"$MJ" init >/dev/null; "$MJ" update >/dev/null
printf '# Objective\n\nx\n\n# Current State\n\nx\n\n# Next Action\n\nx\n' > "$S/note.md"

mkdir -p .ai/repo/ci lib
cat > .ai/repo/ci/gates.yaml <<'YAML'
version: 1
gates:
  - id: lint
    job: structure
    always: true
    runs: "true"
    summary: every script parses
  - id: unit
    job: suite
    runs: "true"
    summary: the unit tests pass
YAML
echo 'a() { :; }' > lib/a.sh
git add -A >/dev/null && git commit -qm base
MODEL="$PWD/.ai/repo/ci/gates.yaml"

# ---------------------------------------------------------------- the shell planner
expect_exit 0 "$PLAN" --model "$MODEL" --check
expect_grep '2 gates \(0 on demand\), 0 classes'
expect_no_grep 'declares no classes'
sh_plan() { printf '%s' "$1" | "$PLAN" --model "$MODEL" --files -; }
sh_set() { sh_plan "$1" | jq -r '.selected | sort | join(" ")'; }

# nothing changed: the always gate and nothing else, and the other gate says why it is out
[ "$(sh_set '')" = "lint" ] || { echo "    ci-plan, no changed path: selected '$(sh_set '')', wanted 'lint'"; exit 1; }
[ "$(sh_plan '' | jq -r '.mode')" = affected ] || { echo "    ci-plan: an empty change is not an affected plan"; exit 1; }
[ "$(sh_plan '' | jq -r '.gates[] | select(.id == "unit") | .selected')" = false ] \
  || { echo "    ci-plan: no classes was read as every gate applies"; exit 1; }

# a changed path: unclassified, named, and escalated — the rule for any unclassified path
[ "$(sh_plan 'lib/a.sh' | jq -r '.unclassified | join(" ")')" = "lib/a.sh" ] \
  || { echo "    ci-plan: the changed path was not reported unclassified"; exit 1; }
[ "$(sh_plan 'lib/a.sh' | jq -r '.classes | length')" = 0 ] || { echo "    ci-plan: a class out of nowhere"; exit 1; }
sh_plan 'lib/a.sh' | jq -r '.reason' | grep -q 'in no class of the model: lib/a.sh' \
  || { echo "    ci-plan: the plan does not name the unclassified path"; exit 1; }

# ---------------------------------------------------------------- the executable
BIN="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"
model_report="$("$BIN" run gates.model --input '{}' --quiet --format json --repo . 2>/dev/null | jq -c '.output')"
[ "$(jq -r '.unreadable // "read"' <<<"$model_report")" = read ] \
  || { echo "    the executable could not read the model: $(jq -r '.unreadable' <<<"$model_report")"; exit 1; }
[ "$(jq -r '.count' <<<"$model_report")" = 2 ] \
  || { echo "    the executable counted $(jq -r '.count' <<<"$model_report") gates, not 2"; exit 1; }
[ "$(jq -r '.classes | length' <<<"$model_report")" = 0 ] || { echo "    the executable reports a class"; exit 1; }
# an always gate is invalidated by anything, a gate no class names by nothing it can name
[ "$(jq -c '[.gates[] | {(.id): .inputs}] | add' <<<"$model_report")" = '{"lint":["*"],"unit":[]}' ] \
  || { echo "    the inputs are not what an empty class list derives: $(jq -c '[.gates[] | {(.id): .inputs}] | add' <<<"$model_report")"; exit 1; }

rs_plan() {
  "$BIN" run gates.completion --input "$(printf '%s' "$1" | jq -Rsc '{changed: (split("\n") | map(select(length > 0)))}')" \
    --quiet --format json --repo . 2>/dev/null | jq -c '.output.plan'
}
rs_set() { rs_plan "$1" | jq -r '.selected | keys | sort | join(" ")'; }
[ "$(rs_set '')" = "lint" ] || { echo "    executable, no changed path: selected '$(rs_set '')', wanted 'lint'"; exit 1; }
[ "$(rs_plan '' | jq -r '.excluded | has("unit")')" = true ] \
  || { echo "    executable: no classes was read as every gate applies"; exit 1; }
[ "$(rs_plan 'lib/a.sh' | jq -r '.unclassified | join(" ")')" = "lib/a.sh" ] \
  || { echo "    executable: the changed path was not reported unclassified"; exit 1; }
rs_plan 'lib/a.sh' | jq -r '.reason' | grep -q 'in no class of the model: lib/a.sh' \
  || { echo "    executable: the plan does not name the unclassified path"; exit 1; }

# ---------------------------------------------------------------- one declaration, two readers
for paths in '' 'lib/a.sh' 'README.md
lib/a.sh'; do
  [ -n "$(rs_set "$paths")" ] && [ "$(sh_set "$paths")" = "$(rs_set "$paths")" ] || {
    echo "    the two readers of gates.yaml disagree for: $(printf '%s' "$paths" | tr '\n' ' ')"
    echo "      ci-plan:    $(sh_set "$paths")"
    echo "      executable: $(rs_set "$paths")"
    exit 1
  }
done

# ---------------------------------------------------------------- recording and finishing
# lib/gates.sh reads `classes.N.*` from the flattened model to derive a gate's inputs. With
# none there it derives none, records the run, and the executable reads that run back.
export MAJORDOMUS_BIN="$BIN"
expect_exit 0 "$MJ" start "work under a model with no classes" --scope lib/
echo 'b() { :; }' >> lib/a.sh
expect_exit 0 "$MJ" evidence --gate unit --exit 0 --command true
expect_grep 'gate unit reported exit 0'
expect_exit 0 "$MJ" evidence --gate lint --exit 0 --command true
expect_exit 0 "$MJ" finish --outcome completed --verify-command "test -d .ai" --note "$S/note.md"
expect_no_grep 'does not parse'
expect_grep 'OK   gate +.*every gate the change selects has reported over this tree'
exit 0

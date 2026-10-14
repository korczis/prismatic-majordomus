# majordomus-covers: none
# `majordomus doctor` does not fail an adopter on a check written for this repository
# (I1996). The `ci` and `claims` families of `reasoning check` judge what Majordomus's own
# workflows and documents may say about advisors; they are the executable half of
# project.advisors-are-optional and run only where that rule is declared. An adopter whose
# archived workflow names a model credential and whose own ADR says "requires ollama" —
# what prismatic-platform's doctor failed on — gets no reasoning failure, is told as
# information which checks did not apply, and every hint doctor prints names a command it
# has (`majordomus reasoning check`), never bin/majordomus-cli. A repository declaring the
# rule, as this one does, still has the same two files refused.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, and (for the second half,
# as case 736 does) when node is absent off CI.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

# plant <root>: the two files of the adopter's report
plant() {
  mkdir -p "$1/.github/workflows/archive" "$1/docs/adrs"
  printf 'jobs:\n  review:\n    env:\n      KEY: ${{ secrets.ANTHROPIC_API_KEY }}\n      OTHER: ${{ secrets.OPENAI_API_KEY }}\n' \
    > "$1/.github/workflows/archive/ci.yml"
  printf '# ADR 0023: Ollama model integration\n\nThe assistant requires Ollama on the host.\n' \
    > "$1/docs/adrs/adr-0023-ollama-model-integration-architecture.md"
}

# --- an adopter: a fresh init, its own product files, and nothing of this repository's rules
"$MJ" init >/dev/null
plant "$PWD"
git add -A >/dev/null && git commit -qm "an adopter's own workflow and ADR"

"$RB" reasoning check --repo "$PWD" --format json > check.json 2> check.err \
  || { echo "    reasoning check refused the adopter:"; cat check.json check.err; exit 1; }
jq -e '.ok == true and .findings == [] and (.checks | index("ci") == null) and (.checks | index("claims") == null) and (.checks | index("records") != null)' check.json >/dev/null \
  || { echo "    the adopter is held to this repository's ci and claims checks:"; cat check.json; exit 1; }

doctor="$(MAJORDOMUS_BIN="$RB" "$MJ" doctor 2>&1 || true)"
if grep -q '^FAIL reasoning' <<<"$doctor"; then
  echo "    doctor failed the adopter on a reasoning finding:"; grep '^FAIL reasoning' <<<"$doctor"; exit 1
fi
grep -q '^OK   reasoning .*reasoning check: catalogue, adapters, coupling, records — no finding' <<<"$doctor" \
  || { echo "    doctor did not run the checks the adopter is held to:"; grep -i reasoning <<<"$doctor"; exit 1; }
grep -q '^INFO reasoning .*ci, claims: not run; .*project\.advisors-are-optional is declared' <<<"$doctor" \
  || { echo "    doctor did not say which checks did not apply:"; grep -i reasoning <<<"$doctor"; exit 1; }
if grep -q 'bin/majordomus-cli' <<<"$doctor"; then
  echo "    doctor printed a hint only this repository can run:"; grep 'bin/majordomus-cli' <<<"$doctor"; exit 1
fi

# --- a repository that declares the rule: the same two files are still refused
reasoning_fixture
[ -f "$R/.ai/repo/rules/project/advisors-are-optional.v1.md" ] \
  || { echo "    the fixture does not declare project.advisors-are-optional"; exit 1; }
plant "$R"
expect_exit 10 rz reasoning check
expect_grep 'FAIL ci .*archive/ci\.yml.*CI names the model credential ANTHROPIC_API_KEY'
expect_grep 'FAIL ci .*archive/ci\.yml.*CI names the model credential OPENAI_API_KEY'
expect_grep "FAIL claims .*adr-0023-ollama-model-integration-architecture\.md.*asserts 'requires ollama'"
( cd "$R" && rz_env MAJORDOMUS_BIN="$RB" "$R/bin/majordomus" doctor > "$T/doctor.txt" 2>&1 ) || true
grep -q '^FAIL reasoning .*ci: CI names the model credential ANTHROPIC_API_KEY.*\[reproduce: majordomus reasoning check\]' "$T/doctor.txt" \
  || { echo "    doctor in this repository no longer fails on the credential, or its hint is not runnable:"; grep -i reasoning "$T/doctor.txt"; exit 1; }
exit 0

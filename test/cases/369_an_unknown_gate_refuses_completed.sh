# majordomus-covers: check finish
# majordomus-negative: finish
# An unknown gate verdict refuses the outcome completed (rule majordomus.completion-gates,
# and majordomus.publication-currency for the gate that measures a deployment).
#
# The completion-gates validator used to skip whenever it could not reach a verdict — no
# reader built, the reader erroring or answering nothing, a model that does not parse — and
# a skip is not a failure, so `finish --outcome completed` passed the gate line of a task
# nothing had judged. Its own messages said "unknown, never a pass"; the behaviour was a
# pass. This case makes the reader unavailable in the ways that do not need a toolchain and
# asserts the claim `completed` is refused while every honest outcome is not:
#
#   reader errors (MAJORDOMUS_BIN exits non-zero)   completed REFUSED, check reports only
#   reader absent (MAJORDOMUS_BIN names nothing)    completed REFUSED
#   model present and unreadable                    completed REFUSED
#   publication gate cannot reach its subject       completed REFUSED
#   any of these with outcome blocked               accepted, named "not refused"
#   no CI model declared at all                     accepted: no gate to be unknown about
#   a real reader over a reported gate              accepted (when an executable is at hand)
. "$ROOT/test/lib.sh"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj369.XXXXXX")"; trap 'rm -rf "$S"' EXIT
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
classes:
  - id: src
    paths: [lib/**]
    gates: [lint]
YAML
cp .ai/repo/ci/gates.yaml "$S/gates.yaml"
# the gate that measures a deployment, spliced into the gates list for the publication half
cat > "$S/publication.yaml" <<'YAML'
  - id: publication
    job: pages
    at-finish: true
    runs: "echo 'pages-check: origin/gh-pages does not exist'; exit 12"
    summary: what the public is being served was built from a commit on the trunk
YAML
echo 'a() { :; }' > lib/a.sh
git add -A >/dev/null && git commit -qm base

# the executable the suite was handed, kept for the positive half before it is replaced
OUTER_BIN="${MAJORDOMUS_BIN:-}"
# a reader that is there and cannot answer
printf '#!/bin/sh\necho "reader exploded" >&2\nexit 3\n' > "$S/broken-reader"
chmod +x "$S/broken-reader"
export MAJORDOMUS_BIN="$S/broken-reader"
export MAJORDOMUS_SHARE="$ROOT/share"
state() { sed -n 's/^outcome: //p' .ai/local/state/current.yaml; }

# ------------------------------------------------------------------ the reader errors
expect_exit 0 "$MJ" start "unjudged work" --scope lib/
echo 'b() { :; }' >> lib/a.sh
# check reports, it does not accept work: the unknown is named and nothing is refused
expect_exit 0 "$MJ" check
expect_grep 'gate .*could not answer gates.completion \(exit [0-9]+\).*unknown, never a pass.*not refused'
expect_exit 10 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'FAIL gate .*could not answer gates.completion \(exit 3\)'
expect_grep 'completed needs a verdict that was read'
expect_grep 'majordomus.completion-gates'
[ "$(state)" = active ] || { echo "    a refused finish wrote the outcome anyway: $(state)"; exit 1; }

# ------------------------------------------------------------------ the reader is absent
export MAJORDOMUS_BIN="$S/no-such-reader"
expect_exit 10 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'FAIL gate .*the gate reader is not built'
[ "$(state)" = active ] || { echo "    a refused finish wrote the outcome anyway: $(state)"; exit 1; }

# ------------------------------------------------------------------ the model is unreadable
# declared and not version 1: gates exist that nothing can plan
sed -i.bak 's/^version: 1$/version: 99/' .ai/repo/ci/gates.yaml && rm -f .ai/repo/ci/gates.yaml.bak
expect_exit 10 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'FAIL gate .*CI model at .ai/repo/ci/gates.yaml is not readable'
cp "$S/gates.yaml" .ai/repo/ci/gates.yaml

# ------------------------------------------------------------------ an honest outcome
# blocked is a statement that the work did not complete; an unknown gate never refuses it
export MAJORDOMUS_BIN="$S/broken-reader"
expect_exit 0 "$MJ" finish --outcome blocked --note "$S/note.md"
expect_grep 'could not answer gates.completion.*not refused: the outcome is not completed'
expect_no_grep 'FAIL gate'
[ "$(state)" = blocked ] || { echo "    blocked was not recorded: $(state)"; exit 1; }

# ------------------------------------------------------------------ publication unknown
# A gate marked at-finish that could not reach its subject (exit 12) is unknown as well.
awk '/^classes:$/ { while ((getline l < "'"$S"'/publication.yaml") > 0) print l } { print }' \
  "$S/gates.yaml" > .ai/repo/ci/gates.yaml
sed -i.bak 's/^    - note_present$/    - note_present\n    - publication_current/' .ai/repo/policy.yaml
rm -f .ai/repo/policy.yaml.bak
grep -q '^    - publication_current$' .ai/repo/policy.yaml || { echo "    the fixture policy was not amended"; exit 1; }
git add -A >/dev/null && git commit -qm 'publishes'
expect_exit 0 "$MJ" start "offline publication" --scope lib/
echo 'c() { :; }' >> lib/a.sh
expect_exit 10 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'FAIL gate +publication .*could not be reached \(exit 12\)'
expect_exit 0 "$MJ" finish --outcome blocked --note "$S/note.md"
expect_grep 'skipped for outcome blocked'

# ------------------------------------------------------------------ no model: nothing unknown
git rm -q .ai/repo/ci/gates.yaml && git commit -qm 'no model'
expect_exit 0 "$MJ" start "ungated repository" --scope lib/
echo 'd() { :; }' >> lib/a.sh
expect_exit 0 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'declares no CI model'
expect_no_grep 'FAIL gate'

# ------------------------------------------------------------------ a real reader: accepted
# The positive half needs the executable; without one the negative half above still stands.
if [ -n "$OUTER_BIN" ]; then export MAJORDOMUS_BIN="$OUTER_BIN"; else unset MAJORDOMUS_BIN; fi
BIN="$(rust_bin 2>/dev/null)" || { echo "    skip: positive path (no Rust executable)"; exit 0; }
export MAJORDOMUS_BIN="$BIN"
mkdir -p .ai/repo/ci && cp "$S/gates.yaml" .ai/repo/ci/gates.yaml
git add -A >/dev/null && git commit -qm 'model again'
expect_exit 0 "$MJ" start "judged work" --scope lib/
echo 'e() { :; }' >> lib/a.sh
expect_exit 0 "$MJ" evidence --gate lint --exit 0 --command true
expect_exit 0 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'OK   gate +.*every gate the change selects has reported over this tree'
exit 0

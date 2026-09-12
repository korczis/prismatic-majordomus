# majordomus-covers: finish
# majordomus-negative: finish
# A session does not finish behind its own published site.
#
# Every other gate in the CI model measures a tree, so a path class selects it. A gate the
# model marks `at-finish` measures a deployment, which no change to any file can affect, so
# no class can ever reach it — and the question "is the public still being served a
# projection of the trunk" ends up owned by nobody. `finish` is where it is asked.
#
# The whole risk is in the three-way verdict, so this case builds a disposable repository
# whose publication gate it writes by hand and walks every state it can be in:
#
#   not selected   the policy does not name the requirement          accepted, skipped
#   exit 0         the publication is current                        accepted
#   exit 10        the publication is behind                         REFUSED, task still active
#   exit 12        the gate could not reach its subject              accepted, named unverified
#   outcome != completed, with the gate failing                      accepted
#   no at-finish gate declared at all                                accepted, says so
#
# The two that matter are the last-but-two and the exit 12. A requirement that refuses when
# it merely could not measure anything is one that gets waived within a week; a requirement
# that passes when it could not measure anything is decoration. It must do neither, and say
# which of the two happened.
. "$ROOT/test/lib.sh"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj275.XXXXXX")"; trap 'rm -rf "$S"' EXIT
"$MJ" init >/dev/null; "$MJ" update >/dev/null
printf '# Objective\n\nx\n\n# Current State\n\nx\n\n# Next Action\n\nx\n' > "$S/note.md"

# The gate the model will name. Its exit status is this file, so the case can move the
# publication under the task without touching anything the tree-level gates can see.
mkdir -p .ai/repo/ci scripts/ci lib
cat > scripts/ci/publication <<'GATE'
#!/usr/bin/env bash
case "$(cat "${MJ275_VERDICT:?}")" in
  ok)      echo "ok    the published commit deadbeef is on master (0 commit(s) behind its head)"; exit 0 ;;
  behind)  echo "FAIL  master is 14 commit(s) ahead of the publication, owed for 4210s"; exit 10 ;;
  unknown) echo "pages-check: origin/gh-pages does not exist; nothing has been published"; exit 12 ;;
esac
GATE
chmod +x scripts/ci/publication
verdict() { printf '%s\n' "$1" > "$S/verdict"; }
export MJ275_VERDICT="$S/verdict"

cat > .ai/repo/ci/gates.yaml <<'YAML'
version: 1
gates:
  - id: lint
    job: structure
    always: true
    runs: "true"
    summary: every script parses
  - id: publication
    job: pages
    at-finish: true
    runs: scripts/ci/publication
    summary: what the public is being served was built from a commit on the trunk
classes:
  - id: src
    paths: [lib/**]
    gates: [lint]
YAML
echo 'a() { :; }' > lib/a.sh
git add -A >/dev/null && git commit -qm base

# --------------------------------------------------- the policy is what turns it on
# Declared but not selected: the doctrine is in the registry and the requirement is not in
# the contract, so it must not run. A doctrine that enforced itself regardless of the policy
# would make `finish_requires` decoration.
verdict behind
expect_exit 0 "$MJ" start "unselected" --scope lib/
expect_exit 0 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'not in verification.finish_requires'

sed -i.bak 's/^    - note_present$/    - note_present\n    - publication_current/' .ai/repo/policy.yaml
rm -f .ai/repo/policy.yaml.bak
grep -q '^    - publication_current$' .ai/repo/policy.yaml || {
  echo "    the fixture policy was not amended; finish_requires reads:"; sed -n '/finish_requires/,/^[a-z]/p' .ai/repo/policy.yaml; exit 1; }
git add -A >/dev/null && git commit -qm select

# --------------------------------------------------- behind: refused, and the task survives
expect_exit 0 "$MJ" start "publishing work" --scope lib/
expect_exit 10 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'FAIL gate +publication'
expect_grep 'the published site is not a projection of the trunk'
expect_grep '14 commit\(s\) ahead of the publication'
expect_grep 'blocking doctrines'
expect_grep 'majordomus.publication-currency'
# refused means written nothing: the task is still active and can still be finished
[ "$(sed -n 's/^outcome: //p' .ai/local/state/current.yaml)" = active ] || {
  echo "    a refused finish wrote the outcome anyway: $(sed -n 's/^outcome: //p' .ai/local/state/current.yaml)"; exit 1; }

# --------------------------------------------------- an honest outcome is never refused
# A task reporting itself blocked is telling the truth; refusing that teaches a worker to
# claim completed instead. The gate is still failing here.
expect_exit 0 "$MJ" finish --outcome blocked --note "$S/note.md"
expect_grep 'skipped for outcome blocked'

# --------------------------------------------------- current: accepted
verdict ok
expect_exit 0 "$MJ" start "more work" --scope lib/
echo 'b() { :; }' >> lib/a.sh
expect_exit 0 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'OK   gate +publication'
expect_grep 'is on master'

# --------------------------------------------------- unreachable: named, never a pass
# No network, no published branch, no hosting API. This is the state the design turns on:
# it is reported by name with its reason and it refuses nothing, because a session that
# could not measure the site is not evidence that the site is stale.
verdict unknown
expect_exit 0 "$MJ" start "offline work" --scope lib/
echo 'c() { :; }' >> lib/a.sh
expect_exit 0 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'could not be reached \(exit 12\)'
expect_grep 'unverified and never a pass'
expect_no_grep 'OK   gate +publication'

# --------------------------------------------------- a repository that publishes nothing
# The mechanism names no gate of its own. Remove the marker and the requirement has nothing
# to run, says exactly that, and refuses nothing.
verdict behind
sed -i.bak '/^    at-finish: true$/d' .ai/repo/ci/gates.yaml
rm -f .ai/repo/ci/gates.yaml.bak
git add -A >/dev/null && git commit -qm 'publishes nothing'
expect_exit 0 "$MJ" start "no publication" --scope lib/
echo 'd() { :; }' >> lib/a.sh
expect_exit 0 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'no gate in this repository.s CI model is marked at-finish'

# --------------------------------------------------- a marker with no command is a defect
# The model can be wrong in one more way: marked at-finish and declaring nothing to run.
# Silence there would be a requirement that is on and does nothing.
sed -i.bak 's|^    runs: scripts/ci/publication$||' .ai/repo/ci/gates.yaml
sed -i.bak2 's|^  - id: publication$|  - id: publication\n    at-finish: true|' .ai/repo/ci/gates.yaml
rm -f .ai/repo/ci/gates.yaml.bak .ai/repo/ci/gates.yaml.bak2
git add -A >/dev/null && git commit -qm 'marked, unrunnable'
expect_exit 0 "$MJ" start "broken model" --scope lib/
echo 'e() { :; }' >> lib/a.sh
expect_exit 10 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'declares no command to run it'

# --------------------------------------------------------------- this repository is wired
# The mechanism working in a fixture proves nothing about whether it is switched on here.
# Both halves of the wiring are asserted against the real tree.
grep -q '^    - publication_current$' "$ROOT/.ai/repo/policy.yaml" || {
  echo "    this repository does not select publication_current in verification.finish_requires"; exit 1; }
grep -q '^    at-finish: true$' "$ROOT/.ai/repo/ci/gates.yaml" || {
  echo "    this repository marks no gate at-finish, so finish asks nothing about the published site"; exit 1; }
awk '/^  - id: pages-live$/{f=1} f && /^    at-finish: true$/{print "wired"; exit}' "$ROOT/.ai/repo/ci/gates.yaml" | grep -q wired || {
  echo "    pages-live is not the gate marked at-finish"; exit 1; }
exit 0

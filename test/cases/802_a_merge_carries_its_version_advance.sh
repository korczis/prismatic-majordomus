# majordomus-covers: none
# A merge into the trunk that carries work carries its version advance (ADR 0106).
#
# `finish` advances the version for a worker who uses it; this is the enforcement that does
# not depend on that. The gate is run exactly as the CI model declares it — the `runs` line
# of `version-obligation` in .ai/repo/ci/gates.yaml, read here rather than restated — over
# merge commits built the way CI sees them: the merge is HEAD, and its first parent is the
# trunk it lands on.
#
#   a merge of work that advanced                     passes
#   a merge of work that did not advance (bypass)     REFUSED, exit 10
#   a second branch that reused the first's version   REFUSED, exit 10
#   a merge of a release record and its projections   passes: evidence owes nothing
#   a merge of projections only                       passes
#   the gate is always planned                        every change set runs it
. "$ROOT/test/lib.sh"
BIN="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_BIN="$BIN" MAJORDOMUS_SHARE="$ROOT/share"

GATES="$ROOT/.ai/repo/ci/gates.yaml"
RUNS="$(awk '/^  - id: version-obligation$/{g=1; next} g && /^  - id:/{exit} g && /^    runs:/{sub(/^    runs: */, ""); print; exit}' "$GATES")"
[ -n "$RUNS" ] || { echo "    .ai/repo/ci/gates.yaml declares no version-obligation gate"; exit 1; }
awk '/^  - id: version-obligation$/{g=1; next} g && /^  - id:/{exit} g && /^    always: true$/{f=1} END{exit !f}' "$GATES" \
  || { echo "    the version-obligation gate is not always planned"; exit 1; }
case "$RUNS" in
  "bin/majordomus-cli release obligation --base HEAD^1") ;;
  *) echo "    the gate runs '$RUNS', not the obligation against the merge's first parent"; exit 1 ;;
esac
# The fixture has no bin/majordomus-cli launcher of its own, so the gate's command is run
# with the launcher's executable in its place: the arguments are the declaration's.
gate() { "$BIN" ${RUNS#bin/majordomus-cli } --repo .; }

"$MJ" init >/dev/null
mkdir -p apps/majordomus-cli lib docs/generated .ai/repo/releases
printf '[package]\nname = "majordomus-cli"\nversion = "2.3.0"\n' > apps/majordomus-cli/Cargo.toml
printf 'docs/generated/** merge=derived\n' > .gitattributes
echo '{}' > docs/generated/x.json
echo 'a() { :; }' > lib/a.sh
printf '\nrelease:\n  cadence: minor\n' >> .ai/repo/policy.yaml
git add -A >/dev/null && git commit -qm base
TRUNK="$(git rev-parse HEAD)"
git update-ref refs/remotes/origin/master "$TRUNK"
land() { git checkout -q -B landing "$1"; git merge -q --no-ff --no-edit "$2"; }

# --------------------------------------------------- work that advanced
git checkout -q -b feature/a "$TRUNK"
echo 'b() { :; }' >> lib/a.sh
"$BIN" release advance --repo . >/dev/null
git add -A >/dev/null && git commit -qm "feat: a"
land "$TRUNK" feature/a
expect_exit 0 gate
expect_grep 'state +satisfied'
AFTER_A="$(git rev-parse HEAD)"

# --------------------------------------------------- work that bypassed the advance
git checkout -q -b feature/bypass "$TRUNK"
echo 'c() { :; }' > lib/c.sh
git add -A >/dev/null && git commit -qm "feat: c, merged by hand"
land "$AFTER_A" feature/bypass
expect_exit 10 gate
expect_grep 'state +owed'
expect_grep 'minimum +2\.5\.0'

# --------------------------------------------------- a second branch reusing the first's version
git checkout -q -b feature/b "$TRUNK"
echo 'd() { :; }' > lib/d.sh
"$BIN" release advance --repo . >/dev/null
git add -A >/dev/null && git commit -qm "feat: b"
land "$AFTER_A" feature/b
expect_exit 10 gate
expect_grep 'declared +2\.4\.0'
expect_grep 'trunk +HEAD\^1 [0-9a-f]+ declares 2\.4\.0'

# --------------------------------------------------- a release record and its projections
git checkout -q -b release/record-v2.4.0 "$AFTER_A"
printf 'version: 2.4.0\n' > .ai/repo/releases/v2.4.0.yaml
echo '{"released":"2.4.0"}' > docs/generated/x.json
git add -A >/dev/null && git commit -qm "chore(release): record v2.4.0"
land "$AFTER_A" release/record-v2.4.0
expect_exit 0 gate
expect_grep 'carries +release-evidence'
expect_grep 'state +not-owed'

# --------------------------------------------------- projections only
git checkout -q -b chore/derive "$AFTER_A"
echo '{"derived":true}' > docs/generated/x.json
git add -A >/dev/null && git commit -qm "chore(derive): projections"
land "$AFTER_A" chore/derive
expect_exit 0 gate
expect_grep 'carries +generated-sync'

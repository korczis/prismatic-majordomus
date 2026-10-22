# majordomus-covers: none
# The version is measured from the public contract, against the real executable in a
# disposable repository with a real history and real published releases.
#
# This case was a set of bare-git fixtures driven through the shell gate, when the gate was
# where the comparison lived. The comparison is now a typed engine (ADR 0051) and the gate is
# an adapter over it, so the fixtures are real repositories and the assertions are about the
# engine's decisions — with the last section proving the adapter still says exactly what the
# engine does.
#
# The unit tests in apps/majordomus-cli/src/release/compat/tests.rs own the compatibility
# matrix — every row of the policy, both modes, both directions of the schema comparison.
# What they cannot own is the thing this case exists for: that the measurement and the
# *writer* are the same answer. Before ADR 0051 they were not. A gate compared the surface
# and a writer counted commit subjects, and the writer won, because the writer runs when a
# person raises the version and the gate ran afterwards, if at all. Every assertion below is
# about that seam:
#
#   an additive change            the analysis refuses, and `release bump` is what fixes it
#   an undershooting override     refused, with both version sites untouched
#   an overshooting override      allowed, and reported as an override rather than silently
#   an unchanged contract         nothing owed, so the gate is not a thing to route around
#   a removal                     named as breaking whatever the policy charges for it
#   an unreadable baseline        refused as unreadable, never as an empty diff
#
# The last is the one a green pipeline hides. "I could not tell" and "nothing changed" are
# different facts, and a check that reports the first as the second is a check that has gone
# blind without saying so.
#
# The baseline's surface is the registry committed at its tag; this tree's surface is the
# live registry of the executable under test. So the fixture controls the comparison by
# writing what each tag published, and `generate registry` gives it the executable's own
# registry to start from rather than a hand-written stand-in that would drift.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is not installed"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-112.XXXXXX")"
trap 'rm -rf "$S"' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

# ---------------------------------------------------------------- a repository that publishes
awk '/^  deployments: repo\/deployments$/{print; print "  releases: repo/releases"; next} {print}' \
  .ai/manifest.yaml > "$S/manifest.yaml" && mv "$S/manifest.yaml" .ai/manifest.yaml
grep -q '^  releases: repo/releases$' .ai/manifest.yaml || {
  echo "    the manifest did not gain a releases section; the skeleton's shape changed"; exit 1; }

cat >> .ai/repo/knowledge/sources.yaml <<'Y'

  - id: release
    kind: release-record
    discovery: vcs
    pathspec: ':(glob).ai/repo/releases/*.yaml'
    required: false
Y

mkdir -p .ai/repo/releases apps/majordomus-cli bin docs/generated
cat > .ai/repo/releases/README.md <<'Y'
---
schema: context/v1
id: ai.repo.releases
kind: context
title: Published releases
description: One record per published release, written by the release pipeline and read by every projection of the release metadata.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 75
---

# Published releases

One file per release, `v<major>.<minor>.<patch>.yaml`, contract `release/v1`.
Y

# The two version sites. Each carries the version a second time where the writer must not
# touch it — a dependency pinned at it, and a comment naming it — so a writer that rewrote
# the file rather than the one line it owns is caught.
version_sites() {  # version_sites VERSION
  cat > apps/majordomus-cli/Cargo.toml <<Y
[package]
name = "fixture"
version = "$1"
edition = "2021"

[dependencies]
serde = { version = "$1" }
Y
  cat > bin/majordomus <<Y
#!/usr/bin/env bash
# this tool was MJ_VERSION $1 when it was written
MJ_VERSION="$1"
echo "fixture \$MJ_VERSION"
Y
  chmod +x bin/majordomus
}
declared_version() { sed -n 's/^MJ_VERSION="\([^"]*\)".*/\1/p' bin/majordomus | head -n 1; }
crate_version()    { awk '/^\[package\]/{p=1;next} /^\[/{p=0} p && /^version *=/{gsub(/[^0-9.]/,"",$3); print $3; exit}' apps/majordomus-cli/Cargo.toml; }

# record VERSION TAG COMMIT
record() {
  cat > ".ai/repo/releases/$2.yaml" <<Y
schema: release/v1
version: "$1"
tag: $2
channel: stable
commit: $3
published_at: "2026-0$4-01T00:00:00Z"
artifacts:
  - target: macos-aarch64
    name: fixture-$2-aarch64-apple-darwin.tar.gz
    url: https://github.com/example/fixture/releases/download/$2/fixture-$2.tar.gz
    sha256: 1111111111111111111111111111111111111111111111111111111111111111
    size: 1024
Y
}

version_sites 1.1.0
git add -A >/dev/null && git commit -qm "chore(fixture): the releases section and the two version sites"

# The executable's own registry, which is what this tree's surface will be measured as.
"$RB" generate registry >/dev/null 2>&1 || { echo "    generate registry failed in the fixture"; exit 1; }
[ -f docs/generated/registry.json ] || { echo "    generate registry wrote no registry"; exit 1; }
cp docs/generated/registry.json "$S/exact.json"
total="$(jq '.capabilities | length' "$S/exact.json")"
[ "$total" -gt 4 ] || { echo "    the executable declares only $total capabilities; this case needs more"; exit 1; }

# The two capabilities the baseline will not have published, named from the registry itself
# so this case never hard-codes an id that a refactor could rename out from under it.
gained_a="$(jq -r '[.capabilities[] | select((.visibility // "public") == "public") | .id] | sort | .[0]' "$S/exact.json")"
gained_b="$(jq -r '[.capabilities[] | select((.visibility // "public") == "public") | .id] | sort | .[1]' "$S/exact.json")"

# ---------------------------------------------------------------- v1.1.0: two capabilities short
#
# What this tag published is the executable's registry minus two capabilities, so the tree
# under test has gained exactly those two and nothing else. An additive change, measured.
jq --arg a "$gained_a" --arg b "$gained_b" \
   '.capabilities |= map(select(.id != $a and .id != $b))' "$S/exact.json" > docs/generated/registry.json
git add -A >/dev/null && git commit -qm "chore(release): the surface v1.1.0 published"
record 1.1.0 v1.1.0 "$(git rev-parse HEAD)" 1
git add -A >/dev/null && git commit -qm "docs(release): the record for v1.1.0"
git tag v1.1.0 HEAD~1

# ---------------------------------------------------------------- an additive change is refused
#
# The version the tree declares is the version the baseline published, and the contract has
# gained two capabilities since. That is a minor owed, and nothing about the commit subjects
# above says so — they are `chore` and `docs`.
expect_exit 10 "$RB" release analyze
expect_grep 'BLOCKED'
expect_grep 'required      minor'
expect_grep 'next minimum  1\.2\.0'
expect_grep "[+] capability $gained_a"
expect_grep "[+] capability $gained_b"

# the human account and the measured one, side by side: the commits claim less than the
# contract did, and the analysis says so rather than deferring to them
expect_exit 10 "$RB" release analyze --explain
expect_grep 'WARNING the commits since v1\.1\.0 classify themselves as'

# the canonical value, which every other surface renders; the text above is a rendering of
# exactly this and never a second computation
"$RB" release analyze --format json > "$S/plan.json" 2>/dev/null || true
[ "$(jq -r .status "$S/plan.json")" = blocked ]           || { echo "    the plan is not blocked"; cat "$S/plan.json"; exit 1; }
[ "$(jq -r .required "$S/plan.json")" = minor ]           || { echo "    required is not minor"; exit 1; }
[ "$(jq -r .required_version "$S/plan.json")" = 1.2.0 ]   || { echo "    required_version is not 1.2.0"; exit 1; }
[ "$(jq -r .baseline.reference "$S/plan.json")" = v1.1.0 ] || { echo "    the baseline is not the recorded release"; exit 1; }
[ "$(jq -r .baseline.recorded "$S/plan.json")" = true ]   || { echo "    the baseline is not the recorded one"; exit 1; }
[ "$(jq -r .policy.schema "$S/plan.json")" = majordomus/version-policy/v1 ] || {
  echo "    the plan does not carry the policy it was decided under"; exit 1; }
[ "$(jq -r '.baseline.fingerprint | startswith("sha256:")' "$S/plan.json")" = true ] || {
  echo "    the baseline carries no surface fingerprint"; exit 1; }

# ------------------------------------------------------- an override may not go under the floor
#
# The assertion the whole case exists for. A minor is owed; a patch is asked for; nothing is
# written. An override that could undershoot would make every measurement above decorative.
expect_exit 10 "$RB" release bump --level patch
expect_grep 'REFUSED'
[ "$(declared_version)" = 1.1.0 ] || { echo "    a refused bump wrote to bin/majordomus anyway: $(declared_version)"; exit 1; }
[ "$(crate_version)" = 1.1.0 ]    || { echo "    a refused bump wrote to the manifest anyway: $(crate_version)"; exit 1; }

# and neither may it go down
expect_exit 10 "$RB" release bump --exact 1.0.0
expect_grep 'REFUSED'
[ "$(declared_version)" = 1.1.0 ] || { echo "    a refused downgrade wrote anyway"; exit 1; }

# --dry-run says what it would do and writes nothing
expect_exit 0 "$RB" release bump --dry-run
expect_grep '1\.1\.0 -> 1\.2\.0'
[ "$(declared_version)" = 1.1.0 ] || { echo "    --dry-run wrote to bin/majordomus"; exit 1; }
[ "$(crate_version)" = 1.1.0 ]    || { echo "    --dry-run wrote to the manifest"; exit 1; }

# ---------------------------------------------------------------- the one writer applies the plan
expect_exit 0 "$RB" release bump
expect_grep '1\.1\.0 -> 1\.2\.0'
[ "$(declared_version)" = 1.2.0 ] || { echo "    the bump did not reach bin/majordomus: $(declared_version)"; exit 1; }
[ "$(crate_version)" = 1.2.0 ]    || { echo "    the bump did not reach the manifest: $(crate_version)"; exit 1; }
# the one line each file owns, and nothing else
grep -q 'serde = { version = "1.1.0" }' apps/majordomus-cli/Cargo.toml || {
  echo "    the writer rewrote a dependency's version"; sed -n '1,10p' apps/majordomus-cli/Cargo.toml; exit 1; }
grep -q '# this tool was MJ_VERSION 1.1.0 when it was written' bin/majordomus || {
  echo "    the writer rewrote a comment"; sed -n '1,5p' bin/majordomus; exit 1; }

# and now the same analysis passes, from the same engine that refused it
expect_exit 0 "$RB" release analyze
expect_grep 'OK'

# raising again to the same version writes nothing and is not an error
expect_exit 0 "$RB" release bump
expect_grep 'already 1\.2\.0'

# ------------------------------------------------------- an override above the floor is allowed
expect_exit 0 "$RB" release bump --level major
expect_grep 'explicit override'
[ "$(declared_version)" = 2.0.0 ] || { echo "    the deliberate major bump did not apply: $(declared_version)"; exit 1; }
[ "$(crate_version)" = 2.0.0 ]    || { echo "    the deliberate major bump did not reach the manifest"; exit 1; }
version_sites 1.2.0   # back to the measured minimum for what follows
git add -A >/dev/null && git commit -qm "chore(fixture): back to the measured version"

# ---------------------------------------------------------------- an unchanged contract owes nothing
#
# Most commits are behind the boundary. A gate that demanded a bump for each of them would be
# a thing to work around within a day, so this is as much the point as the refusal above.
cp "$S/exact.json" docs/generated/registry.json
git add -A >/dev/null && git commit -qm "chore(release): the surface v1.2.0 published"
record 1.2.0 v1.2.0 "$(git rev-parse HEAD)" 2
git add -A >/dev/null && git commit -qm "docs(release): the record for v1.2.0"
git tag v1.2.0 HEAD~1

expect_exit 0 "$RB" release analyze
expect_grep 'no bump is owed'
"$RB" release analyze --format json > "$S/none.json" 2>/dev/null || true
[ "$(jq -r .implied "$S/none.json")" = none ] || {
  echo "    an unchanged contract did not measure as none"; cat "$S/none.json"; exit 1; }

# an internal change that moves no public atom still owes nothing
echo "internal" > internal.txt
git add -A >/dev/null && git commit -qm "refactor(core): structure, with the contract unchanged"
expect_exit 0 "$RB" release analyze
expect_grep 'no bump is owed'

# ---------------------------------------------------------------- a removal is named as breaking
#
# What this tag published includes a capability the executable does not have, so the tree
# under test has lost it. Below 1.0.0 the policy charges a minor for that and still names it;
# at 1.0.0 and above it is a major. This baseline is 1.2.1, so it is a major.
jq '.capabilities += [{"id": "fixture.withdrawn", "kind": "query", "visibility": "public",
      "exposure": {"mcp": {"tool": "majordomus_withdrawn"}, "http": {"method": "GET", "path": "/api/v1/withdrawn"}}}]' \
   "$S/exact.json" > docs/generated/registry.json
git add -A >/dev/null && git commit -qm "chore(release): a surface that carried one more thing"
git tag v1.2.1 HEAD

expect_exit 10 "$RB" release analyze --since v1.2.1 --explain
expect_grep 'BREAKING'
expect_grep '[-] capability fixture[.]withdrawn'
expect_grep 'required      major'
"$RB" release analyze --since v1.2.1 --format json > "$S/breaking.json" 2>/dev/null || true
[ "$(jq -r .breaking "$S/breaking.json")" = true ]          || { echo "    a removal did not set breaking"; exit 1; }
[ "$(jq -r .policy.mode "$S/breaking.json")" = semver ]     || { echo "    a 1.x baseline is not semver mode"; exit 1; }
[ "$(jq -r .required "$S/breaking.json")" = major ]         || { echo "    a removal at 1.x is not a major"; exit 1; }
# the removal is named as a capability, not only counted
[ "$(jq -r '[.changes[] | select(.impact == "major")] | length' "$S/breaking.json")" -ge 1 ] || {
  echo "    no change was classified major"; exit 1; }

# ---------------------------------- an unreadable baseline refuses rather than reporting no change
#
# The failure a green pipeline hides: a baseline whose surface cannot be read must not come
# back as an empty diff. HEAD~N here is a commit from before the registry was ever written.
first="$(git rev-list --max-parents=0 HEAD | head -n 1)"
expect_exit 12 "$RB" release analyze --since "$first"
expect_grep 'carries no docs/generated/registry\.json'

expect_exit 12 "$RB" release analyze --since v9.9.9
expect_grep 'not a commit in this repository'

# ---------------------------------------------------------------- a tag nobody recorded is reported
#
# v1.2.1 was tagged above and never recorded. The baseline is the record, not the newest
# thing that looked like a tag — and the disagreement is reported rather than absorbed.
expect_exit 0 "$RB" release analyze
expect_grep 'v1\.2\.1 is tagged and the layer holds no release record'
"$RB" release analyze --format json > "$S/base.json" 2>/dev/null || true
[ "$(jq -r .baseline.reference "$S/base.json")" = v1.2.0 ] || {
  echo "    the baseline was taken from a tag rather than from the record"; cat "$S/base.json"; exit 1; }

# ---------------------------------------------------------------- the gate is this command
#
# scripts/ci/version-matches-surface holds no semantics of its own: it runs the analyser and
# maps the exit code. If it ever grows a second opinion, these two disagree.
# `cmd; status=$?` aborts under `set -e` before the assignment ever runs, so the status of a
# command expected to be non-zero is captured on the failure branch instead.
gate=0
# Executed, not handed to `sh`: the gate declares `#!/usr/bin/env bash` and uses
# `set -o pipefail`, which dash refuses — and `/bin/sh` is dash on Linux and bash in
# sh-mode on macOS, so `sh <script>` passes here and fails in CI for a reason that has
# nothing to do with what this case is testing.
MJ_ROOT="$PWD" MAJORDOMUS_BIN="$RB" "$ROOT/scripts/ci/version-matches-surface" >"$S/gate.txt" 2>&1 || gate=$?
engine=0
"$RB" release analyze >/dev/null 2>&1 || engine=$?
[ "$gate" = "$engine" ] || {
  echo "    the gate exited $gate and the engine $engine; the adapter has grown an opinion"
  cat "$S/gate.txt"; exit 1; }

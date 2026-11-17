# majordomus-covers: none
# A pre-release version is parsed, ordered by precedence and never resolved as the stable release.
#
# `Version::parse` read three numbers and nothing else, so `release bump --exact 1.0.0-alpha.1`
# was refused as "not a version", the compatibility gate called a tree declaring it malformed,
# and a tag v1.0.0-alpha.1 was invisible to the baseline. The rules themselves — precedence,
# Display, the raise from a pre-release, the floor a pre-release reaches — are the crate's unit
# tests in apps/majordomus-cli/src/release/version.rs. What this case holds is the seam those
# tests cannot reach: the real executable, against a repository with real tags and records,
# writing a pre-release, passing its own gate with it, and then ordering it against a stable
# 0.x release published after it:
#
#   release bump --exact 1.0.0-alpha.1      written; junk pre-releases refused with exit 2
#   release analyze on the bumped tree      ok, the declared pre-release is what is required
#   a later stable v0.29.1                  does not outrank v1.0.0-alpha.1 as the baseline
#   distribution releases                   v0.29.1 is the stable pointer, never the alpha
#   from the alpha                          alpha.2 is accepted, alpha.0 refused, a patch is 1.0.0
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is not installed"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-3212.XXXXXX")"
trap 'rm -rf "$S"' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

# ---------------------------------------------------------------- a repository that published 0.29.0
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

mkdir -p .ai/repo/releases apps/majordomus-cli share scripts
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

One file per release, `v<version>.yaml`, contract `release/v1`.
Y

MANIFEST=apps/majordomus-cli/Cargo.toml
cat > "$MANIFEST" <<'Y'
[package]
name = "fixture"
version = "0.29.0"
edition = "2021"
Y
printf '# GENERATED FILE — DO NOT EDIT DIRECTLY\nversion=0.29.0\n' > share/version.txt
cp "$ROOT/scripts/release-version" scripts/release-version

"$RB" generate registry >/dev/null 2>&1 || { echo "    generate registry failed in the fixture"; exit 1; }
[ -f docs/generated/registry.json ] || { echo "    generate registry wrote no registry"; exit 1; }

# record <version> <channel> <commit>: the record the release pipeline writes, and its tag
record() {
  cat > ".ai/repo/releases/v$1.yaml" <<Y
schema: release/v1
version: "$1"
tag: v$1
channel: $2
commit: $3
published_at: "2026-01-01T00:00:00Z"
artifacts:
  - target: macos-aarch64
    name: fixture-v$1-aarch64-apple-darwin.tar.gz
    url: https://github.com/example/fixture/releases/download/v$1/fixture-v$1.tar.gz
    sha256: 1111111111111111111111111111111111111111111111111111111111111111
    size: 1024
Y
  git add -A >/dev/null && git commit -qm "docs(release): the record for v$1"
  git tag "v$1" "$3"
}

git add -A >/dev/null && git commit -qm "chore(release): the surface v0.29.0 published"
record 0.29.0 stable "$(git rev-parse HEAD)"

# ---------------------------------------------------------------- junk is still not a version
before="$(sha256_of_file "$MANIFEST")"
for junk in 1.0.0-alpha.01 1.0.0-alpha..1 1.0.0- 1.0.0.0 01.0.0-alpha.1; do
  expect_exit 2 "$RB" release bump --exact "$junk" --dry-run
  expect_grep "'$junk' is not a version; it must be a semantic version"
done
[ "$(sha256_of_file "$MANIFEST")" = "$before" ] || { echo "    a refused bump wrote to $MANIFEST"; exit 1; }

# ---------------------------------------------------------------- an explicit pre-release target
expect_exit 0 "$RB" release bump --exact 1.0.0-alpha.1 --dry-run
expect_grep '0\.29\.0 -> 1\.0\.0-alpha\.1'
expect_no_grep 'REFUSED'
[ "$(sha256_of_file "$MANIFEST")" = "$before" ] || { echo "    --dry-run wrote to $MANIFEST"; exit 1; }

expect_exit 0 "$RB" release bump --exact 1.0.0-alpha.1
expect_grep "$MANIFEST now declares 1\.0\.0-alpha\.1"
grep -q '^version = "1\.0\.0-alpha\.1"$' "$MANIFEST" || {
  echo "    the manifest does not declare 1.0.0-alpha.1:"; cat "$MANIFEST"; exit 1; }
# what scripts/derive would project, so the gate below reads a tree whose writers agree
printf '# GENERATED FILE — DO NOT EDIT DIRECTLY\nversion=1.0.0-alpha.1\n' > share/version.txt
git add -A >/dev/null && git commit -qm "chore(release): 1.0.0-alpha.1"

# the compatibility gate accepts the tree that declares it, and names it as what is required
"$RB" release analyze --format json > "$S/alpha.json" 2>/dev/null || true
[ "$(jq -r '.status' "$S/alpha.json")" = ok ] || {
  echo "    release analyze did not accept a tree declaring 1.0.0-alpha.1:"
  jq '{status, declared_version, required_version, diagnostics}' "$S/alpha.json"; exit 1; }
[ "$(jq -r '.declared_version' "$S/alpha.json")" = 1.0.0-alpha.1 ] || {
  echo "    the plan does not read the declared pre-release"; jq .declared_version "$S/alpha.json"; exit 1; }
[ "$(jq -r '.required_version' "$S/alpha.json")" = 1.0.0-alpha.1 ] || {
  echo "    the declared pre-release does not satisfy the contract"; jq .required_version "$S/alpha.json"; exit 1; }
[ "$(jq -r '[(.diagnostics // [])[] | select(.id == "declared-version-malformed")] | length' "$S/alpha.json")" = 0 ] || {
  echo "    1.0.0-alpha.1 was called malformed"; jq .diagnostics "$S/alpha.json"; exit 1; }

# ---------------------------------------------------------------- published, then a later stable 0.x
alpha="$(git rev-parse HEAD)"
record 1.0.0-alpha.1 prerelease "$alpha"
# a maintenance release of the 0.x line, cut after the alpha from the old line's commit
git tag maintenance "$(git rev-parse v0.29.0)"
record 0.29.1 stable "$(git rev-parse maintenance)"
git tag -d maintenance >/dev/null

# the baseline is the highest version the layer records: the alpha, not the later 0.29.1
"$RB" release analyze --format json > "$S/after.json" 2>/dev/null || true
[ "$(jq -r '.baseline.reference' "$S/after.json")" = v1.0.0-alpha.1 ] || {
  echo "    a later stable 0.29.1 outranked 1.0.0-alpha.1 as the baseline:"
  jq '.baseline' "$S/after.json"; exit 1; }
"$RB" release version --format json > "$S/version.json" 2>/dev/null || true
[ "$(jq -r '.last_release' "$S/version.json")" = 1.0.0-alpha.1 ] || {
  echo "    release version's last release is not the alpha:"; jq .last_release "$S/version.json"; exit 1; }

# and the stable pointer is 0.29.1: the alpha outranks it by version and is never resolved
"$RB" distribution releases --format json > "$S/releases.json" 2>/dev/null || {
  echo "    distribution releases failed"; cat "$S/releases.json"; exit 1; }
[ "$(jq -r '.latest.tag' "$S/releases.json")" = v0.29.1 ] || {
  echo "    the stable pointer is not v0.29.1:"; jq .latest "$S/releases.json"; exit 1; }
[ "$(jq -r '[.releases[].tag] | join(" ")' "$S/releases.json")" = "v1.0.0-alpha.1 v0.29.1 v0.29.0" ] || {
  echo "    the releases are not in precedence order:"; jq '[.releases[].tag]' "$S/releases.json"; exit 1; }

# ---------------------------------------------------------------- the next step from the alpha
expect_exit 0 "$RB" release bump --exact 1.0.0-alpha.2 --dry-run
expect_grep '1\.0\.0-alpha\.1 -> 1\.0\.0-alpha\.2'
expect_no_grep 'REFUSED'
expect_exit 10 "$RB" release bump --exact 1.0.0-alpha.0 --dry-run
expect_grep 'REFUSED 1\.0\.0-alpha\.0 is below'
# a raise from a pre-release lands on its own final, as semantic versioning says
expect_exit 0 "$RB" release bump --level patch --dry-run
expect_grep '1\.0\.0-alpha\.1 -> 1\.0\.0 '
grep -q '^version = "1\.0\.0-alpha\.1"$' "$MANIFEST" || { echo "    a dry run wrote to $MANIFEST"; exit 1; }

echo "    1.0.0-alpha.1 is bumped to, gated, ordered above 0.29.1 and never the stable pointer"

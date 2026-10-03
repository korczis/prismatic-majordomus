# majordomus-covers: none
# A tag older than the record practice is a note; a newer tag with no record is the defect.
#
# `release analyze` warned about every tag the layer held no release record for. In this
# repository that was v0.1.0, v0.2.0 and v0.3.0: tags cut before recording began, which
# `.ai/repo/release-record-baseline.txt` already carries as debt and for which no record can
# be written without inventing the digests a record is made of. A warning that is always
# present is a warning nobody reads, and the one that matters — v0.7.0, published with its
# record never landed — wears the same shape.
#
# The decision is a pure function of the tag and the earliest record the layer holds, and its
# unit tests in apps/majordomus-cli/src/release/compat.rs own its table. What this case holds
# is the seam the unit tests cannot reach: that the executable, against a real repository with
# real tags and a real record, prints the two cases differently and that a note never moves the
# verdict:
#
#   a tag below the earliest record     NOTE, id tag-predates-records, exit unchanged
#   a tag at or above it, unrecorded    WARNING, id tag-without-record, as before
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is not installed"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-763.XXXXXX")"
trap 'rm -rf "$S"' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install
# the oldest tag: cut before this layer kept any record
git tag v1.0.0 HEAD

# ---------------------------------------------------------------- a repository that records
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

One file per release, `v<major>.<minor>.<patch>.yaml`, contract `release/v1`.
Y

cat > apps/majordomus-cli/Cargo.toml <<'Y'
[package]
name = "fixture"
version = "1.1.0"
edition = "2021"
Y
printf '# GENERATED FILE — DO NOT EDIT DIRECTLY\nversion=1.1.0\n' > share/version.txt
cp "$ROOT/scripts/release-version" scripts/release-version

"$RB" generate registry >/dev/null 2>&1 || { echo "    generate registry failed in the fixture"; exit 1; }
[ -f docs/generated/registry.json ] || { echo "    generate registry wrote no registry"; exit 1; }
git add -A >/dev/null && git commit -qm "chore(release): the surface v1.1.0 published"
published="$(git rev-parse HEAD)"
cat > .ai/repo/releases/v1.1.0.yaml <<Y
schema: release/v1
version: "1.1.0"
tag: v1.1.0
channel: stable
commit: $published
published_at: "2026-01-01T00:00:00Z"
artifacts:
  - target: macos-aarch64
    name: fixture-v1.1.0-aarch64-apple-darwin.tar.gz
    url: https://github.com/example/fixture/releases/download/v1.1.0/fixture-v1.1.0.tar.gz
    sha256: 1111111111111111111111111111111111111111111111111111111111111111
    size: 1024
Y
git add -A >/dev/null && git commit -qm "docs(release): the record for v1.1.0"
git tag v1.1.0 "$published"

# ---------------------------------------------------------------- older than the practice: a note
expect_exit 0 "$RB" release analyze
expect_grep 'NOTE    v1\.0\.0 is tagged and the layer holds no release record for it, because it predates v1\.1\.0'
expect_no_grep 'WARNING'
"$RB" release analyze --format json > "$S/old.json" 2>/dev/null || true
[ "$(jq -r '[.diagnostics[] | select(.id == "tag-predates-records" and .severity == "note")] | length' "$S/old.json")" = 1 ] || {
  echo "    the plan did not carry v1.0.0 as a tag-predates-records note"; jq .diagnostics "$S/old.json"; exit 1; }
[ "$(jq -r '[.diagnostics[] | select(.id == "tag-without-record")] | length' "$S/old.json")" = 0 ] || {
  echo "    a tag that predates every record was reported as tag-without-record"; jq .diagnostics "$S/old.json"; exit 1; }

# ---------------------------------------------------------------- newer and unrecorded: the defect
echo "internal" > internal.txt
git add -A >/dev/null && git commit -qm "refactor(core): structure, with the contract unchanged"
git tag v1.1.1 HEAD
expect_exit 0 "$RB" release analyze
expect_grep 'WARNING v1\.1\.1 is tagged and the layer holds no release record for it; the changelog'
expect_grep 'NOTE    v1\.0\.0 is tagged'
"$RB" release analyze --format json > "$S/new.json" 2>/dev/null || true
[ "$(jq -r '[.diagnostics[] | select(.id == "tag-without-record" and .severity == "warning")] | length' "$S/new.json")" = 1 ] || {
  echo "    an unrecorded tag newer than the earliest record was not a tag-without-record warning"
  jq .diagnostics "$S/new.json"; exit 1; }
echo "    a tag older than the record practice is a note; a newer unrecorded tag is still a warning"

# majordomus-covers: none
# `release version` answers what the next version is with the public contract, and carries
# what the commit subjects imply as evidence (ADR 0051).
#
# Before this case the version report computed `next` by raising the *declared* version by
# the bump the conventional commits implied. Two defects in one line: the commits, which ADR
# 0051 demotes to evidence, decided the answer, and a version already raised for the window
# was raised again by the same commits. So a tree whose commits said BREAKING and whose
# surface had only grown printed `next 1.0.0` while `release analyze` — the judge that holds
# the 0.x floor — required `0.11.0`.
#
# The fixture is exactly that disagreement: a published 0.10.0 whose committed registry lacks
# two capabilities the executable has, so the contract only grew, and a `feat!` commit after
# it, so the subjects say major. Asserted:
#
#   no release yet           the contract cannot be measured; `next` is the commit inference,
#                            labelled `decided_by: commits`, with the reason — never silently
#   commits BREAKING, surface only grew
#                            `next` is `release analyze`'s required version (0.11.0), decided
#                            by the contract; the commits' major is shown as evidence
#   the version raised to it `next` stays 0.11.0: nothing is counted twice
#   no version line          the analysis can only guess, so the commits answer, labelled,
#                            with the reason — a guess is not passed off as the contract's
#   v0.11.0 published, then only a `fix:` and an unchanged surface
#                            the contract's minimum is the release already made; `next` is a
#                            patch above it (0.11.1), never 0.11.0 again
#   0.12.0 declared above the minimum
#                            `next` is the declared 0.12.0, and the commit inference raises the
#                            last release (0.11.1), not the declared version (0.12.1) — a patch
#                            tells the two bases apart where a major, which resets, cannot
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-743.XXXXXX")"
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

mkdir -p .ai/repo/releases apps/majordomus-cli docs/generated share
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

version_sites() {  # version_sites VERSION
  printf '[package]\nname = "fixture"\nversion = "%s"\nedition = "2021"\n' "$1" \
    > apps/majordomus-cli/Cargo.toml
  printf '# GENERATED FILE — DO NOT EDIT DIRECTLY\nversion=%s\n' "$1" > share/version.txt
}

# record VERSION TAG COMMIT
record() {
  cat > ".ai/repo/releases/$2.yaml" <<Y
schema: release/v1
version: "$1"
tag: $2
channel: stable
commit: $3
published_at: "2026-09-01T00:00:00Z"
artifacts:
  - target: macos-aarch64
    name: fixture-$2-aarch64-apple-darwin.tar.gz
    url: https://github.com/example/fixture/releases/download/$2/fixture-$2.tar.gz
    sha256: 1111111111111111111111111111111111111111111111111111111111111111
    size: 1024
Y
}

version_sites 0.10.0
git add -A >/dev/null && git commit -qm "chore(fixture): the releases section, the manifest and its projection"

# ------------------------------------------- no release yet: the commits answer, and say so
"$RB" release version --format json > "$S/none.json" 2>/dev/null || true
[ "$(jq -r .decided_by "$S/none.json")" = commits ] || {
  echo "    with no published baseline the report does not say the commits answered:"; cat "$S/none.json"; exit 1; }
[ -n "$(jq -r '.contract_unreadable // empty' "$S/none.json")" ] || {
  echo "    the commit fallback carries no reason the contract could not answer:"; cat "$S/none.json"; exit 1; }
[ "$(jq -r .next "$S/none.json")" = "$(jq -r .commits_imply "$S/none.json")" ] || {
  echo "    the fallback's next is not the commit inference it names:"; cat "$S/none.json"; exit 1; }
"$RB" release version > "$S/none.txt" 2>&1 || true
grep -q '(decided by the commits)' "$S/none.txt" || {
  echo "    the text rendering hides that the commits answered:"; cat "$S/none.txt"; exit 1; }
grep -q 'the contract could not be measured' "$S/none.txt" || {
  echo "    the text rendering does not say why the contract did not answer:"; cat "$S/none.txt"; exit 1; }

# ---------------------------------------------------------------- v0.10.0: two capabilities short
"$RB" generate registry >/dev/null 2>&1 || { echo "    generate registry failed in the fixture"; exit 1; }
[ -f docs/generated/registry.json ] || { echo "    generate registry wrote no registry"; exit 1; }
cp docs/generated/registry.json "$S/exact.json"
gained_a="$(jq -r '[.capabilities[] | select((.visibility // "public") == "public") | .id] | sort | .[0]' "$S/exact.json")"
gained_b="$(jq -r '[.capabilities[] | select((.visibility // "public") == "public") | .id] | sort | .[1]' "$S/exact.json")"
[ -n "$gained_a" ] && [ -n "$gained_b" ] || { echo "    the registry names no public capability"; exit 1; }
jq --arg a "$gained_a" --arg b "$gained_b" \
   '.capabilities |= map(select(.id != $a and .id != $b))' "$S/exact.json" > docs/generated/registry.json
git add -A >/dev/null && git commit -qm "chore(release): the surface v0.10.0 published"
record 0.10.0 v0.10.0 "$(git rev-parse HEAD)"
git add -A >/dev/null && git commit -qm "docs(release): the record for v0.10.0"
git tag v0.10.0 HEAD~1

# The commits now say BREAKING; the surface only grew (the two capabilities the baseline did
# not publish). The subjects and the contract disagree, which is the point.
printf 'a note\n' > NOTE.md
git add -A >/dev/null && git commit -qm "feat(fixture)!: a change its author called breaking"

# ------------------------------------------------ the contract answers; the commits are evidence
"$RB" release analyze --format json > "$S/plan.json" 2>/dev/null || true
required="$(jq -r .required_version "$S/plan.json")"
[ "$required" = 0.11.0 ] || {
  echo "    release analyze requires $required, not 0.11.0 (the 0.x floor of an additive change):"
  cat "$S/plan.json"; exit 1; }

"$RB" release version --format json > "$S/version.json" 2>/dev/null || true
[ "$(jq -r .next "$S/version.json")" = "$required" ] || {
  echo "    release version's next is not release analyze's required version ($required):"
  cat "$S/version.json"; exit 1; }
[ "$(jq -r .decided_by "$S/version.json")" = contract ] || {
  echo "    release version does not say the contract decided:"; cat "$S/version.json"; exit 1; }
[ "$(jq -r .bump "$S/version.json")" = major ] || {
  echo "    the commit evidence is not carried as major:"; cat "$S/version.json"; exit 1; }
[ "$(jq -r .commits_imply "$S/version.json")" = 1.0.0 ] || {
  echo "    the commit inference is not 1.0.0 from the last release:"; cat "$S/version.json"; exit 1; }
[ "$(jq -r '.contract_unreadable // "absent"' "$S/version.json")" = absent ] || {
  echo "    a measured answer carries a reason it could not be measured:"; cat "$S/version.json"; exit 1; }

"$RB" release version > "$S/version.txt" 2>&1 || true
grep -q '^next         0\.11\.0 (decided by the contract)$' "$S/version.txt" || {
  echo "    the text rendering does not print the contract's next:"; cat "$S/version.txt"; exit 1; }
grep -q 'commits imply major -> 1\.0\.0 (evidence; the contract decides)' "$S/version.txt" || {
  echo "    the text rendering does not show the commit bump as evidence:"; cat "$S/version.txt"; exit 1; }
grep -q '^next         1\.0\.0' "$S/version.txt" && {
  echo "    the commits still decide next:"; cat "$S/version.txt"; exit 1; }

# --------------------------------------------------- raised to it, nothing is counted twice
version_sites 0.11.0
git add -A >/dev/null && git commit -qm "chore(release): 0.11.0"
"$RB" release version --format json > "$S/raised.json" 2>/dev/null || true
[ "$(jq -r .declared "$S/raised.json")" = 0.11.0 ] || { echo "    the fixture did not declare 0.11.0"; exit 1; }
[ "$(jq -r .next "$S/raised.json")" = 0.11.0 ] || {
  echo "    a version already raised to the requirement was raised again:"; cat "$S/raised.json"; exit 1; }
[ "$(jq -r .commits_imply "$S/raised.json")" = 1.0.0 ] || {
  echo "    the commit evidence since v0.10.0 is no longer a major:"; cat "$S/raised.json"; exit 1; }

# ------------------------------- no version line: a guess is not passed off as the contract's
cp apps/majordomus-cli/Cargo.toml "$S/Cargo.toml"
printf '[package]\nname = "fixture"\nedition = "2021"\n' > apps/majordomus-cli/Cargo.toml
"$RB" release version --format json > "$S/unversioned.json" 2>/dev/null || true
cp "$S/Cargo.toml" apps/majordomus-cli/Cargo.toml
[ "$(jq -r .decided_by "$S/unversioned.json")" = commits ] || {
  echo "    a manifest with no version line still reads as the contract's answer:"
  cat "$S/unversioned.json"; exit 1; }
[ -n "$(jq -r '.contract_unreadable // empty' "$S/unversioned.json")" ] || {
  echo "    the fallback for an unreadable version carries no reason:"; cat "$S/unversioned.json"; exit 1; }
[ "$(jq -r .next "$S/unversioned.json")" = 1.0.0 ] || {
  echo "    the fallback's next is not the commit inference from v0.10.0:"
  cat "$S/unversioned.json"; exit 1; }

# ------------------------------------- v0.11.0 published, then only a fix behind the boundary
cp "$S/exact.json" docs/generated/registry.json
git add -A >/dev/null && git commit -qm "chore(release): the surface v0.11.0 published"
record 0.11.0 v0.11.0 "$(git rev-parse HEAD)"
git add -A >/dev/null && git commit -qm "docs(release): the record for v0.11.0"
git tag v0.11.0 HEAD~1
printf 'a repair\n' >> NOTE.md
git add -A >/dev/null && git commit -qm "fix(fixture): a repair behind the public boundary"

"$RB" release analyze --format json > "$S/fix-plan.json" 2>/dev/null || true
[ "$(jq -r .required_version "$S/fix-plan.json")" = 0.11.0 ] || {
  echo "    the premise is gone: an unchanged surface over v0.11.0 does not require 0.11.0:"
  cat "$S/fix-plan.json"; exit 1; }
"$RB" release version --format json > "$S/fix.json" 2>/dev/null || true
[ "$(jq -r .last_release "$S/fix.json")" = 0.11.0 ] || {
  echo "    the last release is not v0.11.0:"; cat "$S/fix.json"; exit 1; }
[ "$(jq -r .next "$S/fix.json")" != "$(jq -r .last_release "$S/fix.json")" ] || {
  echo "    next is the release already made:"; cat "$S/fix.json"; exit 1; }
[ "$(jq -r .next "$S/fix.json")" = 0.11.1 ] || {
  echo "    next is not the patch above v0.11.0:"; cat "$S/fix.json"; exit 1; }
[ "$(jq -r .decided_by "$S/fix.json")" = contract_and_commits ] || {
  echo "    the patch is not labelled as the contract's floor and the commits:"
  cat "$S/fix.json"; exit 1; }
"$RB" release version > "$S/fix.txt" 2>&1 || true
grep -q '^next         0\.11\.1 (decided by the contract and the commits)$' "$S/fix.txt" || {
  echo "    the text rendering does not name both answers:"; cat "$S/fix.txt"; exit 1; }

# ------------------------- declared above the minimum: the inference raises the last release
version_sites 0.12.0
git add -A >/dev/null && git commit -qm "chore(release): 0.12.0"
"$RB" release version --format json > "$S/above.json" 2>/dev/null || true
[ "$(jq -r .next "$S/above.json")" = 0.12.0 ] || {
  echo "    a declared version above the minimum is not the contract's answer:"
  cat "$S/above.json"; exit 1; }
[ "$(jq -r .decided_by "$S/above.json")" = contract ] || {
  echo "    the declared 0.12.0 is not decided by the contract:"; cat "$S/above.json"; exit 1; }
[ "$(jq -r .commits_imply "$S/above.json")" = 0.11.1 ] || {
  echo "    the commit inference raised the declared version rather than the last release:"
  cat "$S/above.json"; exit 1; }

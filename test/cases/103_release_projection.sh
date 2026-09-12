# The release as a projection, against the real executable in a disposable repository: the
# changelog is composed from the layer's own release records, the decisions whose file first
# appears in each release's tree and the commits in its range; the version is one fact written into the two
# sites that must state it by one writer; and every surface renders one value.
#
# The repository is built here rather than borrowed from the checkout, because every question
# this case asks is a question about a history: which commits fall in which release's range,
# which decision belongs to which release, and what the commits since the last release imply.
# A fixture whose history is real is the only one those answers can be checked against.
#
# The assertion the whole case exists for is the last but one: a release record added to the
# layer becomes a section of the changelog with nothing else edited, and a commit whose
# subject follows no convention is carried rather than dropped. A changelog that silently
# omits what it cannot classify lies by omission, which is the failure this shape prevents.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# Scratch outside the repository under test: $T is the repository itself, and a case that
# assured itself the working tree was clean while writing its own captures into it would be
# asserting nothing. Every file this case reads back lives here.
S="$(mktemp -d "${TMPDIR:-/tmp}/mj-103.XXXXXX")"
trap 'rm -rf "$S"' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

# ---------------------------------------------------------------- a repository that publishes
#
# Three declarations and nothing else: the section the records live in, the source class that
# discovers them, and the two files that state the version. Everything the case asserts below
# is derived from these by the executable.

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

mkdir -p .ai/repo/releases apps/majordomus-cli bin
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

One file per release, `v<major>.<minor>.<patch>.yaml`, contract `release/v1`. A record is
evidence of what was published, never a plan.
Y

# The two version sites. Each carries the version a second time in a place the writer must
# not touch — a dependency pinned at it, and a comment naming it — so that a writer which
# rewrote the file rather than the one line it owns is caught below.
cat > apps/majordomus-cli/Cargo.toml <<'Y'
[package]
name = "fixture"
version = "1.0.0"
edition = "2021"

[dependencies]
serde = { version = "1.0.0" }
Y
cat > bin/majordomus <<'Y'
#!/usr/bin/env bash
# this tool was MJ_VERSION 1.0.0 when it was written
MJ_VERSION="1.0.0"
echo "fixture $MJ_VERSION"
Y
chmod +x bin/majordomus
git add -A >/dev/null && git commit -qm "chore(fixture): the releases section and the two version sites"

# decision DDDD SLUG DATE
decision() {
  cat > ".ai/repo/adrs/$1-$2.md" <<Y
---
schema: adr/v1
id: adr-$1
kind: adr
title: A decision dated $3
status: accepted
date: $3
tags: [fixture]
provenance:
  origin: authored
---

# A decision dated $3

## Context

A decision the changelog must place by the tree that first holds its file, and by
nothing else: the date below is when it was decided, not which release carried it.

## Decision

It is dated $3.

## Consequences

It appears in the section of the release whose tree first holds this file, and in no other.
Y
}
# The decision the first release carries: its file is in v1.0.0's tree and not in v0.9.0's,
# which is the only fact the changelog reads. Its date is deliberately inside the old date
# window too, so a regression to date arithmetic would pass here and fail on the one below.
decision 0001 the-first-release-carries 2026-01-15
git add -A >/dev/null && git commit -qm "docs(adr): a decision the first release carries"


FIRST="$(git rev-parse HEAD~2)"     # what v0.9.0 was published from
SECOND="$(git rev-parse HEAD)"      # what v1.0.0 was published from

# record VERSION TAG COMMIT PUBLISHED_AT
record() {
  cat > ".ai/repo/releases/$2.yaml" <<Y
schema: release/v1
version: "$1"
tag: $2
channel: stable
commit: $3
published_at: "$4"
artifacts:
  - target: macos-aarch64
    name: fixture-$2-aarch64-apple-darwin.tar.gz
    url: https://github.com/example/fixture/releases/download/$2/fixture-$2-aarch64-apple-darwin.tar.gz
    sha256: 1111111111111111111111111111111111111111111111111111111111111111
    size: 1024
Y
}
record 0.9.0 v0.9.0 "$FIRST" "2026-01-01T00:00:00Z"
record 1.0.0 v1.0.0 "$SECOND" "2026-02-01T00:00:00Z"

decision 0002 after-the-last-release 2026-03-01    # added after v1.0.0 was cut: unreleased

git add -A >/dev/null && git commit -qm "docs(release): the records and the decisions the fixture reads"
echo a > alpha.txt; git add -A >/dev/null; git commit -qm "feat(alpha): a capability nobody had before"
echo b > beta.txt;  git add -A >/dev/null; git commit -qm "fix(beta): behaviour that was wrong is not any more"
echo c > gamma.txt; git add -A >/dev/null; git commit -qm "a subject that follows no convention at all"

# ---------------------------------------------------------------- the changelog is composed
#
# One section per record, newest first, with the unreleased work leading; each release's
# decisions are the ones whose file first appears in its tree and each release's changes are the commits
# in its range.
expect_exit 0 "$RB" release
expect_grep '^# Changelog'
expect_grep '^## Unreleased'
expect_grep '^## v1\.0\.0 '
expect_grep '^## v0\.9\.0 '
expect_grep 'Current version: \*\*1\.0\.0\*\*'

# the count is the records' count plus the unreleased section, read off the layer rather
# than written here: a record added to the fixture must not have to be counted again
"$RB" release changelog --format json > "$S/changelog.json" 2> "$S/changelog.err" || {
  echo "    release changelog --format json failed:"; cat "$S/changelog.err"; exit 1; }
records="$(ls .ai/repo/releases/*.yaml | wc -l | tr -d ' ')"
sections="$(grep -c '"unreleased":' "$S/changelog.json" | tr -d ' ')"
[ "$sections" = "$((records + 1))" ] || {
  echo "    $records record(s) and one unreleased section should be $((records + 1)) sections; the changelog has $sections"
  exit 1; }

# a decision belongs to the release whose tree first holds its file, and to no other
"$RB" release changelog v1.0.0 > "$S/one.txt" 2>/dev/null
grep -q 'adr-0001\|A decision dated 2026-01-15' "$S/one.txt" || {
  echo "    the decision v1.0.0's tree first holds is not in its section:"; cat "$S/one.txt"; exit 1; }
grep -q '2026-03-01' "$S/one.txt" && {
  echo "    a decision added after the release appears in it:"; cat "$S/one.txt"; exit 1; }
# and a version nothing carries is refused rather than answered emptily
expect_exit 12 "$RB" release changelog v7.7.7
expect_grep "no release section carries the version 'v7.7.7'"

# --- a commit that follows no convention is carried, not dropped
expect_exit 0 "$RB" release
expect_grep 'a subject that follows no convention at all'
expect_grep '^### Other'
# The sections partition the history — everything up to the first release, then each range,
# then what follows the last — so the changes they carry are every commit and no more. A
# changelog that dropped what it could not classify would be short by exactly the commits
# nobody spelled conventionally, which is the failure this counts.
commits="$(git rev-list --count --no-merges HEAD | tr -d ' ')"
carried="$(grep -c '"subject":' "$S/changelog.json" | tr -d ' ')"
[ "$carried" = "$commits" ] || {
  echo "    the history has $commits commit(s) and the changelog carries $carried change(s)"; exit 1; }

# ---------------------------------------------------------------- one value, three renderings
#
# The command line, the JSON and the generated document are projections of one composed
# value. The proof is byte equality, not a shared phrase.
grep -q '"schema": "majordomus/changelog/v1"' "$S/changelog.json" || {
  echo "    the JSON changelog names no schema"; exit 1; }
grep -q '"current": "1.0.0"' "$S/changelog.json" || {
  echo "    the JSON changelog does not carry the version the tree declares"; exit 1; }
grep -q '"sections":' "$S/changelog.json" || {
  echo "    the JSON changelog carries no sections"; exit 1; }

# `release` with nothing after it is `release changelog`
"$RB" release > "$S/bare.md" 2>/dev/null
"$RB" release changelog > "$S/sub.md" 2>/dev/null
cmp -s "$S/bare.md" "$S/sub.md" || {
  echo "    'release' and 'release changelog' rendered different documents:"
  diff "$S/bare.md" "$S/sub.md" | head -10; exit 1; }

expect_exit 0 "$RB" generate changelog
for f in docs/generated/changelog.json docs/generated/changelog.yaml docs/generated/changelog.md; do
  expect_file "$f"
done
# the generated document, past the provenance header its encoding allows, is what the
# command line renders — the same value, written twice, never composed twice — minus the one
# section no committed file may carry. The unreleased section is `<last release>..HEAD`, and
# a file inside a commit cannot describe the commit it is in, so the committed form stops at
# the newest release while the command line, asked at request time, leads with it.
sed '1,/-->/d' docs/generated/changelog.md > "$S/generated.md"
awk '/^## Unreleased$/ { skip = 1; next } /^## / { skip = 0 } !skip' "$S/bare.md" > "$S/published.md"
grep -q '^## Unreleased$' "$S/bare.md" || { echo "    the command line, asked at HEAD, renders no unreleased section"; exit 1; }
grep -q '^## Unreleased$' "$S/generated.md" && { echo "    the committed changelog carries the unreleased section, which goes stale at the next commit"; exit 1; }
cmp -s "$S/generated.md" "$S/published.md" || {
  echo "    the generated changelog and the rendered one disagree past the unreleased section:"
  diff "$S/generated.md" "$S/published.md" | head -10; exit 1; }

# --- and it is drift-checked: current now, and still current after a commit that publishes
# nothing. That is what the committed form is for — a document that went stale at every
# commit made every commit owe a derive — and what moves it is a release, proved below.
expect_exit 0 "$RB" generate changelog --check
expect_grep 'in sync'
git add -A >/dev/null && git commit -qm "chore(generated): the changelog document"
expect_exit 0 "$RB" generate changelog --check
expect_grep 'in sync'

# ---------------------------------------------------------------- the version, and its writer
#
# Both sites are reported, the tree is clean so they agree, and the bump is arithmetic over
# the commit types rather than a number anyone chose: a feature is in the range, so minor.
expect_exit 0 "$RB" release version
expect_grep 'declared     1\.0\.0'
expect_grep 'tool         1\.0\.0'
expect_grep 'agree        yes'
expect_grep 'last release 1\.0\.0'
expect_grep 'bump         minor'
expect_grep 'next         1\.1\.0'

"$RB" release version --format json > "$S/version.json" 2>/dev/null
for field in '"declared": "1.0.0"' '"tool": "1.0.0"' '"agree": true' '"bump": "minor"' '"next": "1.1.0"'; do
  grep -qF "$field" "$S/version.json" || {
    echo "    the version report does not carry $field:"; cat "$S/version.json"; exit 1; }
done

# one site moved by hand is the failure the report exists to name, with the exit code the
# release pipeline's own check gives
MANIFEST=apps/majordomus-cli/Cargo.toml
ENTRY=bin/majordomus
before_manifest="$(sha256_of_file "$MANIFEST")"
before_entry="$(sha256_of_file "$ENTRY")"
awk '{ if ($0 ~ /^MJ_VERSION=/) print "MJ_VERSION=\"1.0.1\""; else print }' "$ENTRY" > "$S/entry" && mv "$S/entry" "$ENTRY"
grep -q 'MJ_VERSION="1.0.1"' "$ENTRY" || { echo "    the drift the case introduces did not take"; exit 1; }
expect_exit 10 "$RB" release version
expect_grep 'agree        NO'
git checkout -- "$ENTRY"
[ "$(sha256_of_file "$ENTRY")" = "$before_entry" ] || { echo "    the entry was not restored"; exit 1; }

# ---------------------------------------------------------------- the one writer
#
# This fixture publishes release *records* and never commits a registry, so the public
# contract of its releases cannot be read — and the writer says so rather than inventing a
# floor. That is the honest answer and it is the one ADR 0051 requires: before it, the bump
# was derived from the commit subjects here, which is the authority that was removed.
expect_exit 12 "$RB" release bump --dry-run
expect_grep 'the public contract cannot be measured here'
expect_grep 'name the version deliberately'
[ "$(sha256_of_file "$MANIFEST")" = "$before_manifest" ] || { echo "    a refused bump wrote to $MANIFEST"; exit 1; }
[ "$(sha256_of_file "$ENTRY")" = "$before_entry" ] || { echo "    a refused bump wrote to $ENTRY"; exit 1; }

# A version named deliberately is still written: refusing would make the command unusable in
# exactly the repositories that most need to cut a first release. --dry-run writes nothing.
expect_exit 0 "$RB" release bump --level minor --dry-run
expect_grep 'would raise 1\.0\.0 -> 1\.1\.0'
expect_grep "$MANIFEST"
expect_grep "$ENTRY"
[ "$(sha256_of_file "$MANIFEST")" = "$before_manifest" ] || { echo "    --dry-run wrote to $MANIFEST"; exit 1; }
[ "$(sha256_of_file "$ENTRY")" = "$before_entry" ] || { echo "    --dry-run wrote to $ENTRY"; exit 1; }
[ -z "$(git status --porcelain)" ] || {
  echo "    --dry-run dirtied the working tree:"; git status --porcelain; exit 1; }

# a version that is not three numbers is refused before anything is written
expect_exit 2 "$RB" release bump --exact 1.2
[ "$(sha256_of_file "$MANIFEST")" = "$before_manifest" ] || { echo "    a refused bump still wrote"; exit 1; }
expect_exit 2 "$RB" release bump --level enormous

# --exact writes both sites, and they agree afterwards
expect_exit 0 "$RB" release bump --exact 9.9.9
expect_grep '1\.0\.0 -> 9\.9\.9'
expect_grep "$MANIFEST written"
expect_grep "$ENTRY written"
expect_grep 'both writers agree'
expect_exit 0 "$RB" release version
expect_grep 'declared     9\.9\.9'
expect_grep 'tool         9\.9\.9'
expect_grep 'agree        yes'

# and nothing else in either file moved: one line each, and the two decoys are untouched
git diff --numstat -- "$MANIFEST" "$ENTRY" > "$S/numstat"
[ "$(wc -l < "$S/numstat" | tr -d ' ')" = 2 ] || {
  echo "    the bump touched files other than the two version sites:"; cat "$S/numstat"; exit 1; }
while read -r added removed path; do
  [ "$added" = 1 ] && [ "$removed" = 1 ] || {
    echo "    the bump rewrote $path ($added added, $removed removed) instead of its one version line"
    git diff -- "$path" | head -20; exit 1; }
done < "$S/numstat"
grep -q 'serde = { version = "1.0.0" }' "$MANIFEST" || { echo "    a dependency pinned at the old version was rewritten"; exit 1; }
grep -q 'MJ_VERSION 1.0.0 when it was written' "$ENTRY" || { echo "    a comment naming the old version was rewritten"; exit 1; }

# writing the version that is already there writes nothing, so a bump is safe to repeat
expect_exit 0 "$RB" release bump --exact 9.9.9
expect_grep 'already 9\.9\.9; nothing written'

git checkout -- "$MANIFEST" "$ENTRY"

# ---------------------------------------------------------------- adding a release edits nothing
#
# The invariant the architecture is for: a record put into the layer becomes a section, with
# no document, no template and no list edited anywhere.
before_sections="$sections"
record 1.1.0 v1.1.0 "$(git rev-parse HEAD)" "2026-04-01T00:00:00Z"
git add -A >/dev/null && git commit -qm "docs(release): record v1.1.0"
# a release is the one thing that moves the committed changelog: stale now, current once
# regenerated, and the case leaves it current so that nothing below inherits the drift
expect_exit 10 "$RB" generate changelog --check
expect_grep 'changelog'
expect_exit 0 "$RB" generate changelog
git add -A >/dev/null && git commit -qm "chore(generated): the changelog follows the record"
expect_exit 0 "$RB" generate changelog --check
"$RB" release changelog --format json > "$S/changelog2.json" 2>/dev/null
after_sections="$(grep -c '"unreleased":' "$S/changelog2.json" | tr -d ' ')"
[ "$after_sections" = "$((before_sections + 1))" ] || {
  echo "    a record added to the layer did not become a section ($before_sections -> $after_sections)"; exit 1; }
expect_exit 0 "$RB" release
expect_grep '^## v1\.1\.0 '

# ---------------------------------------------------------------- where the release is projected
#
# The read half is a capability, so it is on the machine surfaces; the writer is a repository
# mutation, so the exposure policy withholds it from every one of them and says why. Neither
# is configured: both follow from what running the command changes.
expect_exit 0 "$RB" capabilities describe release.changelog
expect_grep 'GET /api/v1/changelog'
expect_grep 'majordomus://changelog'
expect_exit 0 "$RB" capabilities describe release.version
expect_grep 'GET /api/v1/release/version'

expect_exit 0 "$RB" commands show executable.release.bump
expect_grep 'repository-mutation'
expect_grep 'above the machine ceiling'
# the machine rows are empty, and that is the whole point of the row above
"$RB" commands show executable.release.bump --format json > "$S/bump.json" 2>/dev/null
grep -qi 'api/v1/release/bump' "$S/bump.json" && { echo "    the writer is projected onto HTTP"; exit 1; }
grep -qi 'majordomus_release_bump' "$S/bump.json" && { echo "    the writer is projected as an MCP tool"; exit 1; }

# ---------------------------------------------------------------- the links are derived, or absent
#
# Every address on a changelog entry comes from the crate manifest's own repository URL, so a
# fork carries them and nothing is written twice. This fixture's repository is not a forge the
# tool understands, which makes it the interesting case: an unknown host must yield NO links
# rather than a guessed one, because a wrong link cannot be told from a right one until it is
# followed.
"$RB" release changelog --format json > "$S/links.json" 2>/dev/null
invented="$(jq -r '[.sections[].groups[].changes[].url // empty] | map(select(startswith("https://github.com/") | not)) | .[0] // empty' "$S/links.json")"
[ -z "$invented" ] || { echo "    a commit link was invented for an unknown forge: $invented"; exit 1; }
notbad="$(jq -r '[.sections[].notes_url // empty] | map(select(startswith("http") | not)) | .[0] // empty' "$S/links.json")"
[ -z "$notbad" ] || { echo "    a notes_url was rewritten into something that is not a URL: $notbad"; exit 1; }

# --- a reference is resolved against the layer, never merely matched
#
# The commit below names an issue the layer has and one it does not. Only the first may
# appear: an id that matches the shape and names nothing is not a link, because a reader
# cannot tell a dead link from a live one until they follow it.
# The issue is valid under the repository's own schema (majordomus.issue/v1), because the
# index drops a record the schema refuses, and a dropped record resolves nothing — the
# same silence pj_issue in test/lib.sh was written to avoid.
mkdir -p .ai/repo/project/issues
cat > .ai/repo/project/issues/I4242.yaml <<'Y'
id: I4242
milestone: M000
title: An issue the changelog may link to
slug: an-issue-the-changelog-may-link-to
priority: p2
profile: routine
parallel_safe: true
objective: "Exists so that a commit naming it produces a reference, and one naming I9999 does not."
scope:
  - src/I4242
acceptance_criteria:
  - A commit naming it carries a reference to it
validation:
  - "true"
evidence_required:
  - proof
Y
git add -A >/dev/null 2>&1
git commit -qm "fix(link): resolves I4242 and mentions I9999, which the layer does not have"
"$RB" release changelog --format json > "$S/refs.json" 2>/dev/null
jq -e '[.sections[].groups[].changes[].references[]? | select(.id == "I4242")] | length == 1' "$S/refs.json" >/dev/null \
  || { echo "    an issue the layer holds produced no reference"; exit 1; }
jq -e '[.sections[].groups[].changes[].references[]? | select(.id == "I9999")] | length == 0' "$S/refs.json" >/dev/null \
  || { echo "    an id that names nothing was carried as a reference"; exit 1; }
jq -e '[.sections[].groups[].changes[].references[]? | select(.id == "I4242" and .title == "An issue the changelog may link to")] | length == 1' "$S/refs.json" >/dev/null \
  || { echo "    the reference does not carry the title from the record it resolved to"; exit 1; }

# ---------------------------------------------------------------- no secret reaches a projection
#
# Nothing in the release reads the environment. The assertion is cheap and the proof is the
# interesting part: every surface is asked while two sentinels are set, and every byte of
# every answer, and of every file the generation wrote, is searched for them.
export OPENAI_API_KEY=SENTINEL_OPENAI_DO_NOT_LEAK
export ANTHROPIC_API_KEY=SENTINEL_ANTHROPIC_DO_NOT_LEAK
"$RB" generate changelog >/dev/null 2>&1
{
  "$RB" release
  "$RB" release changelog --format json
  "$RB" release changelog v1.0.0
  "$RB" release version
  "$RB" release version --format json
  # Its exit code is not what this section asserts — only its output is scanned — and in
  # this fixture it refuses, because no release here committed a registry to measure against.
  "$RB" release bump --dry-run || true
  "$RB" capabilities describe release.changelog
  "$RB" capabilities describe release.version
  "$RB" commands show executable.release.bump
  cat docs/generated/changelog.json
  cat docs/generated/changelog.yaml
  cat docs/generated/changelog.md
} > "$S/everything.txt" 2>&1
if grep -q 'SENTINEL_' "$S/everything.txt"; then
  echo "    a sentinel reached a release projection:"; grep -n 'SENTINEL_' "$S/everything.txt" | head -5; exit 1
fi
unset OPENAI_API_KEY ANTHROPIC_API_KEY

# ---------------------------------------------------------------- the gate over this repository
#
# The gate is run against the checkout it belongs to, which is where its subject lives: the
# two version sites are one fact, and no changelog is authored anywhere.
[ -x "$ROOT/scripts/ci/release-check" ] || { echo "    scripts/ci/release-check is missing or not executable"; exit 1; }
( cd "$ROOT" && ./scripts/ci/release-check > "$S/gate.out" 2>&1 ) || {
  echo "    the release gate failed on this checkout:"; cat "$S/gate.out"; exit 1; }

# and it can fail: a copy in which the two writers disagree is rejected, exit 10
PROBE="$S/probe"; mkdir -p "$PROBE/scripts/ci" "$PROBE/apps/majordomus-cli" "$PROBE/bin" "$PROBE/.ai/repo/releases"
cp "$ROOT/scripts/ci/release-check" "$PROBE/scripts/ci/"
cp "$ROOT/scripts/release-version" "$PROBE/scripts/"
cp "$ROOT/apps/majordomus-cli/Cargo.toml" "$PROBE/apps/majordomus-cli/"
mkdir -p "$PROBE/apps/majordomus-cli/src/release"
cp "$ROOT/apps/majordomus-cli/src/release/version.rs" "$PROBE/apps/majordomus-cli/src/release/"
printf '#!/usr/bin/env bash\nMJ_VERSION="0.0.0-probe"\n' > "$PROBE/bin/majordomus"
( cd "$PROBE" && ./scripts/ci/release-check > "$S/probe.out" 2>&1; echo $? > "$S/probe.code" ) || true
[ "$(cat "$S/probe.code")" = 10 ] || {
  echo "    a tree whose two version sites disagree was not rejected (exit $(cat "$S/probe.code")):"
  cat "$S/probe.out"; exit 1; }

# ...and an authored changelog is rejected too, by the check that exists to reject it
printf '# Changelog\n\n## 1.0.0\n\n- something somebody remembered to write down\n' > "$PROBE/CHANGELOG.md"
printf 'MJ_VERSION="%s"\n' "$("$ROOT/scripts/release-version")" > "$PROBE/bin/majordomus"
( cd "$PROBE" && ./scripts/ci/release-check > "$S/probe2.out" 2>&1 ) || true
grep -q 'CHANGELOG.md' "$S/probe2.out" || {
  echo "    the gate did not reject an authored changelog:"; cat "$S/probe2.out"; exit 1; }

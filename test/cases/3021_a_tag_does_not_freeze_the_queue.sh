# majordomus-covers: none
# A tag does not turn every other pull request red until its record lands (I2054,
# evidence a_tag_does_not_freeze_the_queue).
#
# scripts/ci/release-check refuses a published tag the layer holds no release record for —
# the v0.7.0 state, where the publish job could not propose the record and the installer
# went on serving the previous version (case 103 holds that). The record reaches master
# through a pull request from release/record-<tag>, which needs a full run of its own, and
# until it merged every other head failed on that one finding: four pull requests on
# 2026-10-08 (v0.17.0), every branch between 13:15Z and 14:17Z on 2026-10-14 (v0.25.0).
#
# A fixture with a real origin publishes a tag and proposes its record on the branch the
# release workflow pushes, and the gate is asked from five heads:
#
#   1  a branch that changes no release file, while the record is proposed: passes, and
#      names the tag as pending and the branch that proposes it
#   2  the record's own branch: passes as recorded, nothing pending
#   3  a branch named release/record-<tag> whose tip does not carry the record proposes
#      nothing, and the tag is refused
#   4  a proposal older than the grace is refused: a branch left behind by a closed pull
#      request must not excuse the tag for ever
#   5  the proposal withdrawn — nothing proposes the record any more — refused at once,
#      naming the tag, which is the v0.7.0 state again
. "$ROOT/test/lib.sh"

P="$T/published"; O="$T/origin.git"
mkdir -p "$P/scripts/ci" "$P/apps/majordomus-cli/src/release" "$P/share" "$P/.ai/repo/releases"
cp "$ROOT/scripts/ci/release-check" "$P/scripts/ci/"
cp "$ROOT/scripts/release-version" "$P/scripts/"
cp "$ROOT/apps/majordomus-cli/Cargo.toml" "$ROOT/apps/majordomus-cli/Cargo.lock" "$P/apps/majordomus-cli/"
cp "$ROOT/apps/majordomus-cli/src/release/version.rs" "$P/apps/majordomus-cli/src/release/"
cp -R "$ROOT/bin" "$ROOT/lib" "$P/"
# the version this checkout declares, so the fixture's one release is the one its crate
# states and every other check of the gate stays clean: what is measured is the tag against
# its record, and nothing else
V="$("$ROOT/scripts/release-version")"
TAG="v$V"
printf 'version=%s\n' "$V" > "$P/share/version.txt"
printf 'nothing about releases\n' > "$P/README.md"

g() { git -C "$P" "$@"; }
git init -q --bare "$O"
g init -q -b master
g config user.email t@example.com
g config user.name T
g remote add origin "$O"
g add -A
g commit -qm "the tree the tag was cut from"
g tag "$TAG"
g push -q origin master "$TAG"

record() {
  mkdir -p "$P/.ai/repo/releases"
  cat > "$P/.ai/repo/releases/$TAG.yaml" <<Y
schema: release/v1
version: "$V"
tag: $TAG
channel: stable
Y
}

# the publish job: the record committed on release/record-<tag> and pushed, nothing on master
g checkout -q -b "release/record-$TAG"
record
g add -A
g commit -qm "chore(release): record $TAG"
g push -q origin "release/record-$TAG"
g checkout -q master
g branch -q -D "release/record-$TAG"
# a fetch, as actions/checkout with fetch-depth 0 makes: every branch as refs/remotes/origin/*
g fetch -q origin

gate() { ( cd "$P" && ./scripts/ci/release-check ); }

# ---------------------------------------------------------------- 1. an unrelated head passes
g checkout -q -b feature/unrelated
printf 'a change about something else\n' >> "$P/README.md"
g commit -qam "docs: something else"
[ ! -f "$P/.ai/repo/releases/$TAG.yaml" ] || { echo "    the fixture's unrelated head carries the record"; exit 1; }
expect_exit 0 gate || { echo "    a head that changes no release file was refused while $TAG's record is proposed"; exit 1; }
expect_grep "^pending $TAG is published and its record is proposed on remotes/origin/release/record-$TAG" \
  || { echo "    the pending tag is not named with the branch that proposes its record"; exit 1; }
expect_grep '1 pending' || { echo "    the summary passes a proposed record off as a recorded one"; exit 1; }
expect_no_grep 'every published tag this checkout carries has a release record' \
  || { echo "    a tree whose record has not landed was reported clean"; exit 1; }

# ---------------------------------------------------------------- 2. the record's own head
g checkout -q -B record-head "origin/release/record-$TAG"
expect_exit 0 gate || { echo "    the record's own branch was refused"; exit 1; }
expect_grep 'every published tag this checkout carries has a release record' \
  || { echo "    the record's own branch did not say every tag is recorded"; exit 1; }
expect_no_grep '^pending ' || { echo "    the record's own branch reported its tag as pending"; exit 1; }
g checkout -q feature/unrelated

# ---------------------------------------------------------------- 3. a branch without the record
g branch -q "release/record-$TAG" master
g push -q -f origin "release/record-$TAG"
g branch -q -D "release/record-$TAG"
g fetch -q origin
expect_exit 10 gate || { echo "    a record branch whose tip carries no record excused the tag"; exit 1; }
expect_grep "FAIL  $TAG is published and the layer holds no release record" \
  || { echo "    the refusal does not name the tag"; exit 1; }

# ---------------------------------------------------------------- 4. a proposal past its grace
g checkout -q -b "release/record-$TAG" master
record
g add -A
GIT_COMMITTER_DATE="$(date -u -r "$(( $(date -u +%s) - 3 * 86400 ))" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null \
  || date -u -d "@$(( $(date -u +%s) - 3 * 86400 ))" +%Y-%m-%dT%H:%M:%SZ)" \
  g commit -qm "chore(release): record $TAG, three days ago"
g push -q -f origin "release/record-$TAG"
g checkout -q feature/unrelated
g branch -q -D "release/record-$TAG"
g fetch -q origin
expect_exit 10 gate || { echo "    a proposal three days old still excused the tag"; exit 1; }
expect_grep "FAIL  $TAG is published and its record has waited on remotes/origin/release/record-$TAG for [0-9]+h, longer than the 24h grace" \
  || { echo "    the stale proposal is not named with its age and the grace"; exit 1; }
# the grace is the one knob, and it is read: a wider one accepts the same proposal
expect_exit 0 env MJ_RELEASE_RECORD_GRACE_HOURS=100 "$P/scripts/ci/release-check" \
  || { echo "    MJ_RELEASE_RECORD_GRACE_HOURS was not read"; exit 1; }
expect_exit 12 env MJ_RELEASE_RECORD_GRACE_HOURS=soon "$P/scripts/ci/release-check" \
  || { echo "    a grace that is not a number was not refused as unusable"; exit 1; }

# ---------------------------------------------------------------- 5. nothing proposes it
g push -q origin --delete "release/record-$TAG"
g fetch -q --prune origin
expect_exit 10 gate || { echo "    a published tag with no record and nothing proposing one was not refused"; exit 1; }
expect_grep "FAIL  $TAG is published and the layer holds no release record for it .* and no release/record-$TAG branch proposes one" \
  || { echo "    the refusal does not say that nothing proposes the record"; exit 1; }

echo "    a head that changes no release file passes while $TAG's record is proposed, naming it pending; the record's branch passes as recorded; a branch without the record, a proposal past its grace and no proposal at all are refused"

# majordomus-covers: none
# release-check treats a tag whose release is still in flight as pending, not as a missing
# record (I2712, evidence tag_in_flight_is_pending_case).
#
# On 2026-10-14 18:37Z the structure job of #1146 failed scripts/ci/release-check with "v0.26.0
# is published and the layer holds no release record for it ... and no
# release/record-v0.26.0 branch proposes one" while release run 38613749519 for that tag was
# still building and no GitHub release existed. Case 3021 holds the window after the record
# branch exists; this holds the window before it, which only the forge can see. A fixture with
# a tag and neither record nor record branch asks a stub `gh` on PATH (MJ_RELEASE_CHECK_REPO
# names the repository, so nothing reaches the network):
#
#   1  the tag's release run in progress, or queued: pending, naming the run, exit 0
#   2  no run listed yet and no GitHub release, inside the grace: pending, exit 0
#   3  the same past the grace: the finding
#   4  the run concluded (success or failure) with no record and no branch: the finding,
#      with the run named in a note before it
#   5  a GitHub release with no run listed: the finding
#   6  the forge cannot be read, or no GitHub repository is known: the finding, and a note
#      saying the run was not asked — what could not be read never excuses a tag
#   7  a record branch proposing the record answers first: the forge is not asked at all
. "$ROOT/test/lib.sh"

P="$T/published"; O="$T/origin.git"; FX="$T/fx"; LOG="$T/gh.log"
mkdir -p "$P/scripts/ci" "$P/apps/majordomus-cli/src/release" "$P/share" "$P/.ai/repo/releases" "$FX" "$T/bin"
cp "$ROOT/scripts/ci/release-check" "$P/scripts/ci/"
cp "$ROOT/scripts/release-version" "$P/scripts/"
cp "$ROOT/apps/majordomus-cli/Cargo.toml" "$ROOT/apps/majordomus-cli/Cargo.lock" "$P/apps/majordomus-cli/"
cp "$ROOT/apps/majordomus-cli/src/release/version.rs" "$P/apps/majordomus-cli/src/release/"
cp -R "$ROOT/bin" "$ROOT/lib" "$P/"
# the version this checkout declares, so every other check of the gate stays clean and what is
# measured is the tag against its record and its release run
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
g fetch -q origin

cat > "$T/bin/gh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$GH_STUB_LOG"
[ ! -f "$GH_STUB_FX/down" ] || { echo "HTTP 503" >&2; exit 1; }
a="$*"
case "$a" in
  *"/actions/workflows/release.yml/runs?branch="*)
    if [ -f "$GH_STUB_FX/runs.json" ]; then cat "$GH_STUB_FX/runs.json"; else echo '{"workflow_runs":[]}'; fi ;;
  *"/releases/tags/"*)
    [ -f "$GH_STUB_FX/release" ] || { echo '{"message":"Not Found"}'; echo "gh: Not Found (HTTP 404)" >&2; exit 1; }
    echo '{"tag_name":"x"}' ;;
  *) echo "stub gh: no answer for $a" >&2; exit 1 ;;
esac
STUB
chmod +x "$T/bin/gh"

reset() { rm -f "$FX"/*; : > "$LOG"; }
release_run() { # <status> [conclusion]
  local c=null
  [ -z "${2:-}" ] || c="\"$2\""
  printf '{"workflow_runs":[{"id":555,"status":"%s","conclusion":%s,"created_at":"2026-10-14T18:00:00Z","html_url":"https://example.invalid/runs/555"}]}' \
    "$1" "$c" > "$FX/runs.json"
}
gate() { ( cd "$P" && env PATH="$T/bin:$PATH" GH_STUB_LOG="$LOG" GH_STUB_FX="$FX" MJ_RELEASE_CHECK_REPO=o/r "$@" ./scripts/ci/release-check ); }
FINDING="FAIL  $TAG is published and the layer holds no release record for it .* and no release/record-$TAG branch proposes one"

# ---------------------------------------------------------------- 1. the run is going
for st in in_progress queued; do
  reset; release_run "$st"
  expect_exit 0 gate || { echo "    a tag whose release run is $st was refused"; exit 1; }
  expect_grep "^pending $TAG is tagged and its release is still in flight: its release run 555 \($st\) https://example.invalid/runs/555 is still going" \
    || { echo "    the pending tag does not name its $st run"; exit 1; }
  expect_grep '\(1 pending\)$' || { echo "    the summary passes an in-flight release off as recorded"; exit 1; }
  expect_grep 'except the 1 whose release is still in flight' || { echo    "the except clause does not name the in-flight tag"; exit 1; }
  expect_no_grep '^FAIL' || exit 1
  grep -q "repos/o/r/actions/workflows/release.yml/runs?branch=$TAG" "$LOG" || { echo "    the tag's own runs were not asked:"; cat "$LOG"; exit 1; }
done

# ---------------------------------------------------------------- 2. no run yet, no release, inside the grace
reset
expect_exit 0 gate || { echo "    a tag with no run listed yet and no GitHub release was refused"; exit 1; }
expect_grep "^pending $TAG is tagged and its release is still in flight: no release run of it is listed yet and GitHub holds no release of it \([0-9]+h old, grace 24h\)" \
  || { echo "    the run-less, release-less tag is not named pending with its age"; exit 1; }

# ---------------------------------------------------------------- 3. past the grace
reset
expect_exit 10 gate MJ_RELEASE_RECORD_GRACE_HOURS=0 || { echo "    a tag past the grace with no run and no release was excused"; exit 1; }
expect_grep "^note  $TAG: no release run of it was ever listed and GitHub holds no release of it" || exit 1
expect_grep "$FINDING" || { echo "    the finding is not the one case 3021 reads"; exit 1; }

# ---------------------------------------------------------------- 4. the run concluded
for c in success failure; do
  reset; release_run completed "$c"
  expect_exit 10 gate || { echo "    a tag whose release run concluded $c with no record was excused"; exit 1; }
  expect_grep "^note  $TAG: its release run 555 \($c\) has concluded" || { echo "    the concluded run is not named"; exit 1; }
  expect_grep "$FINDING" || exit 1
done

# ---------------------------------------------------------------- 5. a release and no run listed
reset; : > "$FX/release"
expect_exit 10 gate || { echo "    a published GitHub release with no record was excused"; exit 1; }
expect_grep "^note  $TAG: GitHub holds a release of $TAG and no release run of it is listed" || exit 1

# ---------------------------------------------------------------- 6. the forge cannot be asked
reset; : > "$FX/down"
expect_exit 10 gate || { echo "    an unreadable forge excused the tag"; exit 1; }
expect_grep "^note  $TAG: the release runs of $TAG in o/r could not be read" || { echo "    the unread forge is not said"; exit 1; }
reset; release_run in_progress
# no repository named, and origin is a path: nothing is asked, and the tag is not excused
expect_exit 10 env -u MJ_RELEASE_CHECK_REPO PATH="$T/bin:$PATH" GH_STUB_LOG="$LOG" GH_STUB_FX="$FX" "$P/scripts/ci/release-check" \
  || { echo "    a tag was excused with no GitHub repository known"; exit 1; }
expect_grep "^note  $TAG: no GitHub repository is known" || exit 1
[ ! -s "$LOG" ] || { echo "    the forge was asked with no repository known:"; cat "$LOG"; exit 1; }

# ---------------------------------------------------------------- 7. a proposal answers first
reset; release_run completed success
g checkout -q -b "release/record-$TAG"
cat > "$P/.ai/repo/releases/$TAG.yaml" <<Y
schema: release/v1
version: "$V"
tag: $TAG
channel: stable
Y
g add -A; g commit -qm "chore(release): record $TAG"
g push -q origin "release/record-$TAG"
g checkout -q master; g branch -q -D "release/record-$TAG"; g fetch -q origin
expect_exit 0 gate || { echo "    a proposed record was refused"; exit 1; }
expect_grep "^pending $TAG is published and its record is proposed on remotes/origin/release/record-$TAG" || exit 1
[ ! -s "$LOG" ] || { echo "    the forge was asked although the record branch answered:"; cat "$LOG"; exit 1; }

echo "    a tag whose release run is queued or running, or that has neither run nor release yet inside the grace, is pending with its run named; a concluded run, a published release or an unreadable forge leave the finding standing, with a note saying why"

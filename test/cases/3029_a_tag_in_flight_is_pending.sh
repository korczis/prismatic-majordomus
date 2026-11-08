# majordomus-covers: none
# release-check treats a tag whose release is still in flight as pending, not as a missing
# record (I2712, evidence tag_in_flight_is_pending_case), and excuses a tag without a record
# until its grace ends, whatever the job fetched and whenever the release run concluded (I4341).
#
# On 2026-10-14 18:37Z the structure job of #1146 failed scripts/ci/release-check with "v0.26.0
# is published and the layer holds no release record for it ... and no
# release/record-v0.26.0 branch proposes one" while release run 38613749519 for that tag was
# still building and no GitHub release existed. Case 3021 holds the window after the record
# branch exists; this holds the window before it, which only the forge can see. On 2026-10-28
# master validate run 40676150754 went red on v1.0.0-alpha.9: its jobs fetched after the tag and
# before the record branch was pushed, and judged after the release run had concluded, so
# neither excuse covered it. A fixture with a tag and neither record nor fetched record branch
# asks a stub `gh` on PATH (MJ_RELEASE_CHECK_REPO names the repository, so nothing reaches the
# network), and a second clone plays the release job pushing release/record-<tag> to origin:
#
#   1  the tag's release run in progress, or queued: pending, naming the run, exit 0
#   2  no run listed yet and no GitHub release, inside the grace: pending, exit 0
#   3  the same past the grace: the finding
#   4  the run concluded (success or failure) with no record, no branch and no GitHub
#      release: the finding, with the run named in a note before it
#   5  a GitHub release younger than the grace is pending whatever its run did; one older
#      than the grace is the finding
#   6  the forge cannot be read, or no GitHub repository is known: the finding, and a note
#      saying the run was not asked — what could not be read never excuses a tag
#   7  every ordering of {judged before the record branch's push, fetched before it, fetched
#      after it} x {release run in_progress, success, cancelled} inside the grace: pending,
#      exit 0; a branch pushed after the fetch is asked of origin and fetched
#   8  past the grace, a tag without a record is the finding in each ordering
#   9  origin unreachable: the branch it holds is not seen, the finding stands, and the note
#      names the remote
#  10  a record branch proposing the record answers first: the forge is not asked at all
. "$ROOT/test/lib.sh"

P="$T/published"; O="$T/origin.git"; W="$T/publisher"; FX="$T/fx"; LOG="$T/gh.log"
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
w() { git -C "$W" "$@"; }
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

# the release job's own clone: what it pushes reaches origin and not this checkout's refs
git clone -q -b master "$O" "$W"
w config user.email r@example.com
w config user.name R
w checkout -q -b "release/record-$TAG"
mkdir -p "$W/.ai/repo/releases"
cat > "$W/.ai/repo/releases/$TAG.yaml" <<Y
schema: release/v1
version: "$V"
tag: $TAG
channel: stable
Y
w add -A
w commit -qm "chore(release): record $TAG"

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
    cat "$GH_STUB_FX/release" ;;
  *) echo "stub gh: no answer for $a" >&2; exit 1 ;;
esac
STUB
chmod +x "$T/bin/gh"

RECORD_REF="refs/remotes/origin/release/record-$TAG"
# nothing on origin or in this checkout proposes the record, and the forge holds nothing
reset() {
  rm -f "$FX"/*; : > "$LOG"
  g update-ref -d "$RECORD_REF" 2>/dev/null || true
  if [ -n "$(git -C "$O" for-each-ref "refs/heads/release/record-$TAG")" ]; then
    w push -q origin --delete "release/record-$TAG"
  fi
}
release_run() { # <status> [conclusion]
  local c=null
  [ -z "${2:-}" ] || c="\"$2\""
  printf '{"workflow_runs":[{"id":555,"status":"%s","conclusion":%s,"created_at":"2026-10-14T18:00:00Z","html_url":"https://example.invalid/runs/555"}]}' \
    "$1" "$c" > "$FX/runs.json"
}
iso_ago() { # <hours>: an ISO 8601 UTC time that many hours before now, as GitHub writes it
  local e=$(( $(date -u +%s) - $1 * 3600 ))
  date -u -r "$e" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -d "@$e" +%Y-%m-%dT%H:%M:%SZ
}
github_release() { # <hours ago>: GitHub published a release of the tag that long ago
  printf '{"tag_name":"%s","published_at":"%s","created_at":"%s"}' "$TAG" "$(iso_ago "$1")" "$(iso_ago "$1")" > "$FX/release"
}
push_record() { w push -q origin "release/record-$TAG"; }
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

# ---------------------------------------------------------------- 4. the run concluded, nothing published
for c in success failure; do
  reset; release_run completed "$c"
  expect_exit 10 gate || { echo "    a tag whose release run concluded $c with no record and no GitHub release was excused"; exit 1; }
  expect_grep "^note  $TAG: its release run 555 \($c\) has concluded and GitHub holds no release of it" || { echo "    the concluded run is not named"; exit 1; }
  expect_grep "$FINDING" || exit 1
done

# ---------------------------------------------------------------- 5. a GitHub release, young and old
reset; github_release 1
expect_exit 0 gate || { echo "    a GitHub release an hour old with no run listed was refused inside the grace"; exit 1; }
expect_grep "^pending $TAG is tagged and its release is still in flight: GitHub published its release of $TAG 1h ago \(grace 24h\) and no release run of it is listed" \
  || { echo "    the young release is not named pending with its age"; exit 1; }
reset; github_release 30; release_run completed success
expect_exit 10 gate || { echo "    a GitHub release thirty hours old with no record was excused"; exit 1; }
expect_grep "^note  $TAG: GitHub published its release of $TAG 30h ago, longer than the 24h grace, and its release run 555 \(success\) has concluded" || exit 1
expect_grep "$FINDING" || exit 1

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

# ---------------------------------------------------------------- 7. every ordering, inside the grace
# the release run's state: in_progress before it publishes; success and cancelled after it
# published the GitHub release an hour ago (a cancel after the publish job is what leaves
# a release without a record and a run that is no longer going)
forge_state() { # <run state>
  case "$1" in
    in_progress) release_run in_progress ;;
    *) release_run completed "$1"; github_release 1 ;;
  esac
}
for st in in_progress success cancelled; do
  # (a) judged before the record branch is pushed: only the forge can answer
  reset; forge_state "$st"
  expect_exit 0 gate || { echo "    [$st, judged before the push] the tag was refused inside the grace"; exit 1; }
  expect_grep "^pending $TAG is tagged and its release is still in flight: " \
    || { echo "    [$st, judged before the push] the tag is not pending on its release"; exit 1; }
  expect_no_grep '^FAIL' || exit 1

  # (b) fetched before the push, judged after it: origin is asked for the one branch
  reset; forge_state "$st"
  push_record
  [ -z "$(g for-each-ref "$RECORD_REF")" ] || { echo "    the fixture's checkout already holds the record branch"; exit 1; }
  expect_exit 0 gate || { echo "    [$st, fetched before the push] the tag was refused inside the grace"; exit 1; }
  expect_grep "^pending $TAG is published and its record is proposed on remotes/origin/release/record-$TAG \(0h old, grace 24h\)" \
    || { echo "    [$st, fetched before the push] the branch pushed after the fetch was not seen"; exit 1; }
  [ -n "$(g for-each-ref "$RECORD_REF")" ] || { echo "    [$st] the record branch was not fetched into $RECORD_REF"; exit 1; }
  [ ! -s "$LOG" ] || { echo "    [$st, fetched before the push] the forge was asked although origin answered:"; cat "$LOG"; exit 1; }
  expect_no_grep '^FAIL' || exit 1

  # (c) fetched after the push: the fetched ref answers, origin is not needed
  reset; forge_state "$st"
  push_record; g fetch -q origin
  expect_exit 0 gate || { echo "    [$st, fetched after the push] the tag was refused inside the grace"; exit 1; }
  expect_grep "^pending $TAG is published and its record is proposed on remotes/origin/release/record-$TAG" || exit 1
  [ ! -s "$LOG" ] || { echo "    [$st, fetched after the push] the forge was asked although the record branch answered:"; cat "$LOG"; exit 1; }
done

# ---------------------------------------------------------------- 8. every ordering, past the grace
# a run still going is in flight at any age (case 1): what ends with the grace is the excuse
# of a release that has concluded, whichever way
for st in success cancelled; do
  reset; release_run completed "$st"; github_release 1
  expect_exit 10 gate MJ_RELEASE_RECORD_GRACE_HOURS=0 || { echo "    [$st, judged before the push] a release past the grace was excused"; exit 1; }
  expect_grep "$FINDING" || exit 1
  reset; release_run completed "$st"; github_release 1; push_record
  expect_exit 10 gate MJ_RELEASE_RECORD_GRACE_HOURS=0 || { echo "    [$st, fetched before the push] a proposal past the grace was excused"; exit 1; }
  expect_grep "^FAIL  $TAG is published and its record has waited on remotes/origin/release/record-$TAG for 0h, longer than the 0h grace" || exit 1
done

# ---------------------------------------------------------------- 9. origin cannot be asked
reset; release_run completed cancelled; push_record
GONE="$T/origin-moved.git"
g remote set-url origin "$GONE"
expect_exit 10 gate || { echo "    a tag was excused although origin could not be asked"; exit 1; }
expect_grep "^note  $TAG: origin \($GONE\) could not be asked whether release/record-$TAG proposes its record" \
  || { echo "    the note does not name the remote that was not reached"; exit 1; }
expect_grep "$FINDING" || exit 1
g remote set-url origin "$O"

# ---------------------------------------------------------------- 10. a proposal answers first
reset; release_run completed success
push_record; g fetch -q origin
expect_exit 0 gate || { echo "    a proposed record was refused"; exit 1; }
expect_grep "^pending $TAG is published and its record is proposed on remotes/origin/release/record-$TAG" || exit 1
[ ! -s "$LOG" ] || { echo "    the forge was asked although the record branch answered:"; cat "$LOG"; exit 1; }

echo "    a tag whose release run is queued or running, whose GitHub release is younger than the grace, or that has neither run nor release yet inside the grace, is pending; a record branch pushed after the fetch is asked of origin; in every ordering of fetch, push and conclusion the tag is pending inside the grace and the finding past it; an unreadable forge or origin leaves the finding standing, with a note naming what was not reached"

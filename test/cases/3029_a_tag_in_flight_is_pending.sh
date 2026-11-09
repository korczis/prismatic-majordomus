# majordomus-covers: none
# release-check treats a tag whose release is still in flight as pending, not as a missing
# record (I2712, evidence tag_in_flight_is_pending_case), and keeps it pending until its grace
# ends whatever the job fetched and whenever the release run concluded (I4341).
#
# On 2026-10-14 18:37Z the structure job of #1146 failed scripts/ci/release-check with "v0.26.0
# is published and the layer holds no release record for it ... and no
# release/record-v0.26.0 branch proposes one" while release run 38613749519 for that tag was
# still building and no GitHub release existed. On 2026-10-28 master validate run 40676150754
# went red the same way on v1.0.0-alpha.9: it had fetched before the record branch was pushed
# and judged after the release run ended; on 2026-11-09 validate run 42562442194 did on
# v1.0.1, whose release run had failed and was being rerun. Case 3021 holds the window after
# the record branch is fetched; this holds every other one. A fixture with a tag and no record
# asks a stub `gh` on PATH (MJ_RELEASE_CHECK_REPO names the repository, so nothing reaches
# the network) and a real bare origin:
#
#   1  the tag's release run in progress, or queued: pending, naming the run, exit 0
#   2  no run listed yet and no GitHub release, inside the grace: pending, exit 0
#   3  the same past the grace: the finding
#   4  every ordering inside the grace — the record branch {not pushed yet, pushed after the
#      checkout fetched, pushed before it} x the release run {in_progress, success, failure,
#      cancelled}: pending, exit 0; a branch pushed after the fetch is asked of origin and
#      that one ref fetched, and the forge is not asked once a branch answers
#   5  a concluded run past the grace, with no record and no branch: the finding, with the run
#      named in a note before it; a record branch past the grace: the stale-proposal finding
#   6  a GitHub release published inside the grace is pending whatever its run concluded; one
#      published longer ago than the grace is the finding
#   7  the forge cannot be read, or no GitHub repository is known: the finding, and a note
#      saying the run was not asked — what could not be read never excuses a tag
#   8  origin cannot be asked: the record branch it holds is not seen, the finding stands when
#      the forge cannot excuse the tag either, and a note names the remote
#   9  a record branch proposing the record answers first: the forge is not asked at all
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
    if [ -s "$GH_STUB_FX/release" ]; then cat "$GH_STUB_FX/release"; else echo '{"tag_name":"x"}'; fi ;;
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

# the record the publish job would push, kept under a ref no gate reads, so each ordering can
# put it on origin's release/record-<tag> or take it away
g checkout -q -b fixture-record
cat > "$P/.ai/repo/releases/$TAG.yaml" <<Y
schema: release/v1
version: "$V"
tag: $TAG
channel: stable
Y
g add -A; g commit -qm "chore(release): record $TAG"
g update-ref refs/fixture/record HEAD
g checkout -q master; g branch -q -D fixture-record
[ ! -f "$P/.ai/repo/releases/$TAG.yaml" ] || { echo "    the fixture's master carries the record"; exit 1; }
REC="refs/remotes/origin/release/record-$TAG"
unpush_record() {
  g push -q origin --delete "release/record-$TAG" 2>/dev/null || true
  g update-ref -d "$REC" 2>/dev/null || true
}
push_record() { g push -q -f origin "refs/fixture/record:refs/heads/release/record-$TAG"; }
iso_ago() { # <seconds ago>
  local at=$(( $(date -u +%s) - $1 ))
  date -u -r "$at" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -d "@$at" +%Y-%m-%dT%H:%M:%SZ
}

# ---------------------------------------------------------------- 4. every ordering inside the grace
for order in unpushed after-fetch before-fetch; do
  for st in in_progress success failure cancelled; do
    reset; unpush_record
    if [ "$st" = in_progress ]; then release_run in_progress; else release_run completed "$st"; fi
    case "$order" in
      unpushed) ;;
      # the checkout fetched before the publish job pushed: origin has the branch, this
      # checkout does not
      # (a push updates the pusher's own tracking ref, so it is taken away again)
      after-fetch) push_record; g update-ref -d "$REC" ;;
      before-fetch) push_record; g fetch -q origin ;;
    esac
    expect_exit 0 gate || { echo "    record branch $order, release run $st: a tag inside its grace was refused"; exit 1; }
    expect_no_grep '^FAIL' || exit 1
    expect_grep '\(1 pending\)$' || { echo "    record branch $order, release run $st: the summary passes the tag off as recorded"; exit 1; }
    case "$order" in
      unpushed)
        if [ "$st" = in_progress ]; then
          expect_grep "^pending $TAG is tagged and its release is still in flight: its release run 555 \(in_progress\)" || exit 1
        else
          expect_grep "^pending $TAG is tagged and its release is still in flight: its release run 555 \($st\) has concluded and GitHub holds no release of it yet \([0-9]+h old, grace 24h\)" \
            || { echo "    a run that concluded $st inside the grace is not named pending"; exit 1; }
        fi ;;
      *)
        expect_grep "^pending $TAG is published and its record is proposed on remotes/origin/release/record-$TAG \([0-9]+h old, grace 24h\)" \
          || { echo "    record branch $order, release run $st: the proposal on origin was not read"; exit 1; }
        [ "$(g rev-parse -q --verify "$REC" || true)" = "$(g rev-parse refs/fixture/record)" ] \
          || { echo "    record branch $order: origin's release/record-$TAG was not fetched as that one ref"; exit 1; }
        [ ! -s "$LOG" ] || { echo "    record branch $order: the forge was asked although origin's branch answered:"; cat "$LOG"; exit 1; } ;;
    esac
  done
done
unpush_record

# ---------------------------------------------------------------- 5. a concluded run past the grace
for c in success failure cancelled; do
  reset; release_run completed "$c"
  expect_exit 10 gate MJ_RELEASE_RECORD_GRACE_HOURS=0 || { echo "    a tag past its grace whose release run concluded $c with no record was excused"; exit 1; }
  expect_grep "^note  $TAG: its release run 555 \($c\) has concluded and GitHub holds no release of it, [0-9]+h after the tag, longer than the 0h grace" \
    || { echo "    the concluded run is not named"; exit 1; }
  expect_grep "$FINDING" || exit 1
done
# and a record branch past the grace, found only on origin, is the stale-proposal finding
reset; release_run completed success; push_record
expect_exit 10 gate MJ_RELEASE_RECORD_GRACE_HOURS=0 || { echo "    a record branch past the grace excused the tag"; exit 1; }
expect_grep "FAIL  $TAG is published and its record has waited on remotes/origin/release/record-$TAG for [0-9]+h, longer than the 0h grace" || exit 1
unpush_record

# ---------------------------------------------------------------- 6. a GitHub release
for c in success failure; do
  reset; release_run completed "$c"
  printf '{"tag_name":"%s","published_at":"%s"}' "$TAG" "$(iso_ago 3600)" > "$FX/release"
  expect_exit 0 gate || { echo "    a GitHub release published an hour ago, run $c, was refused"; exit 1; }
  expect_grep "^pending $TAG is tagged and its release is still in flight: GitHub published its release 1h ago \(grace 24h\) and its release run 555 \($c\) has concluded" \
    || { echo "    the fresh GitHub release is not named pending"; exit 1; }
done
# a release with no run listed is pending too, counted from the tag when it states no date
reset; : > "$FX/release"
expect_exit 0 gate || { echo "    a fresh GitHub release with no run listed was refused"; exit 1; }
expect_grep "^pending $TAG is tagged and its release is still in flight: GitHub published its release 0h ago \(grace 24h\) and no release run of it is listed" || exit 1
# a release older than the grace is the finding, whatever its run did
reset; release_run completed success
printf '{"tag_name":"%s","published_at":"%s"}' "$TAG" "$(iso_ago $(( 3 * 86400 )))" > "$FX/release"
expect_exit 10 gate || { echo "    a GitHub release three days old with no record was excused"; exit 1; }
expect_grep "^note  $TAG: GitHub holds a release of $TAG, published 72h ago, longer than the 24h grace, and its release run 555 \(success\) has concluded" || exit 1
expect_grep "$FINDING" || exit 1

# ---------------------------------------------------------------- 7. the forge cannot be asked
reset; : > "$FX/down"
expect_exit 10 gate || { echo "    an unreadable forge excused the tag"; exit 1; }
expect_grep "^note  $TAG: the release runs of $TAG in o/r could not be read" || { echo "    the unread forge is not said"; exit 1; }
reset; release_run in_progress
# no repository named, and origin is a path: nothing is asked, and the tag is not excused
expect_exit 10 env -u MJ_RELEASE_CHECK_REPO PATH="$T/bin:$PATH" GH_STUB_LOG="$LOG" GH_STUB_FX="$FX" "$P/scripts/ci/release-check" \
  || { echo "    a tag was excused with no GitHub repository known"; exit 1; }
expect_grep "^note  $TAG: no GitHub repository is known" || exit 1
[ ! -s "$LOG" ] || { echo "    the forge was asked with no repository known:"; cat "$LOG"; exit 1; }

# ---------------------------------------------------------------- 8. origin cannot be asked
# origin holds the record branch, this checkout never fetched it, and origin is unreachable:
# the branch is not seen, the unreadable forge cannot excuse the tag, and the note says which
# remote was not asked
reset; push_record; g update-ref -d "$REC"; : > "$FX/down"
g remote set-url origin "$T/unreachable.git"
expect_exit 10 gate || { echo "    a tag was excused with origin unreachable and the forge unreadable"; exit 1; }
expect_grep "^note  $TAG: origin \($T/unreachable.git\) could not be asked for release/record-$TAG" \
  || { echo "    the note does not name the remote that could not be asked"; exit 1; }
expect_grep "$FINDING" || exit 1
g remote set-url origin "$O"
unpush_record

# ---------------------------------------------------------------- 9. a proposal answers first
reset; release_run completed success
push_record; g fetch -q origin
expect_exit 0 gate || { echo "    a proposed record was refused"; exit 1; }
expect_grep "^pending $TAG is published and its record is proposed on remotes/origin/release/record-$TAG" || exit 1
[ ! -s "$LOG" ] || { echo "    the forge was asked although the record branch answered:"; cat "$LOG"; exit 1; }

echo "    inside its grace a tag without a record is pending in every ordering of the checkout's fetch, the record branch's push and the release run's conclusion — origin asked for the branch the fetch missed, a concluded run or a fresh GitHub release named; past the grace, with the forge unreadable or with origin unreachable the finding stands, with a note saying why"

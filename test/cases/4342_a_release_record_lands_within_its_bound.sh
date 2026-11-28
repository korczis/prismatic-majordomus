# majordomus-covers: none
# A release's record is not left waiting past the release seat's bound (I4342, evidence test).
#
# v1.3.0 was tagged at 14:10Z on 2026-11-27 and published at 14:54Z. Its record pull request
# #3202 was opened at 15:05Z and landed at 15:40Z only because a person merged it, and until
# then the homepage offered 1.2.0 while releases/latest.json named 1.3.0. The 24h grace of
# scripts/ci/release-check is how long an unrelated head may ignore a proposed record, not how
# long the release may; nothing told the release seat its record was waiting.
#
# A fixture with a real origin publishes a tag and proposes its record on release/record-<tag>,
# its tip dated as the publish job would have committed it, and a stub `gh` on PATH answers the
# forge's pull requests offline:
#
#   1  without --seat the gate reports the record's age in minutes and stays pending; it never
#      asks the forge about a proposal git already answered (case 3029 holds that too)
#   2  --seat: a record pull request 2 hours and 1 minute old is a finding, exit 10, naming
#      the pull request and its age; one 119 minutes old passes, pending, naming both
#   3  the pull request's own age counts, not its branch's tip: an old tip with a fresh pull
#      request passes, a fresh tip with an old pull request fails
#   4  a forge that cannot be asked still bounds the record, by its branch's age, and a note
#      says why the pull request was not named
#   5  MJ_RELEASE_RECORD_SEAT_MINUTES is the one knob and is read; a bound that is not a
#      number, and an argument that is not --seat, are unusable (exit 12)
. "$ROOT/test/lib.sh"
# the gate honours MJ_ROOT, and a caller that exports it would have it measure that tree
unset MJ_ROOT

P="$T/published"; O="$T/origin.git"; FX="$T/fx"; LOG="$T/gh.log"
mkdir -p "$P/scripts/ci" "$P/apps/majordomus-cli/src/release" "$P/share" "$P/.ai/repo/releases" \
  "$T/bin" "$FX"
cp "$ROOT/scripts/ci/release-check" "$P/scripts/ci/"
cp "$ROOT/scripts/release-version" "$P/scripts/"
cp "$ROOT/apps/majordomus-cli/Cargo.toml" "$ROOT/apps/majordomus-cli/Cargo.lock" "$P/apps/majordomus-cli/"
cp "$ROOT/apps/majordomus-cli/src/release/version.rs" "$P/apps/majordomus-cli/src/release/"
cp -R "$ROOT/bin" "$ROOT/lib" "$P/"
# the version this checkout declares, so every other check of the gate stays clean and what is
# measured is the record's wait
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

ago() { # <seconds ago> -> ISO 8601 UTC
  local at=$(( $(date -u +%s) - $1 ))
  date -u -r "$at" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -d "@$at" +%Y-%m-%dT%H:%M:%SZ
}
# the publish job: the record committed on release/record-<tag>, its tip <seconds> old, pushed,
# and fetched as actions/checkout with fetch-depth 0 fetches every branch
propose() { # <seconds ago>
  g checkout -q -B "release/record-$TAG" master
  mkdir -p "$P/.ai/repo/releases"
  cat > "$P/.ai/repo/releases/$TAG.yaml" <<Y
schema: release/v1
version: "$V"
tag: $TAG
channel: stable
Y
  g add -A
  GIT_COMMITTER_DATE="$(ago "$1")" g commit -qm "chore(release): record $TAG"
  g push -q -f origin "release/record-$TAG"
  g checkout -q master
  g branch -q -D "release/record-$TAG"
  g fetch -q origin
}
# the forge's open pull request from the record branch, opened <seconds> ago
pull_request() { # <seconds ago>
  printf '[{"number":3202,"html_url":"https://example.invalid/pull/3202","created_at":"%s","head":{"ref":"release/record-%s"}}]' \
    "$(ago "$1")" "$TAG" > "$FX/pulls.json"
}

cat > "$T/bin/gh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$GH_STUB_LOG"
[ ! -f "$GH_STUB_FX/down" ] || { echo "HTTP 503" >&2; exit 1; }
case "$*" in
  "api repos/o/r/pulls?head=o:release/record-"*"&state=open&per_page=5")
    if [ -f "$GH_STUB_FX/pulls.json" ]; then cat "$GH_STUB_FX/pulls.json"; else echo '[]'; fi ;;
  *) echo "stub gh: no answer for $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$T/bin/gh"
reset() { rm -f "$FX"/*; : > "$LOG"; }
gate() { ( cd "$P" && env PATH="$T/bin:$PATH" GH_STUB_LOG="$LOG" GH_STUB_FX="$FX" MJ_RELEASE_CHECK_REPO=o/r "$@" ./scripts/ci/release-check ); }
seat() { ( cd "$P" && env PATH="$T/bin:$PATH" GH_STUB_LOG="$LOG" GH_STUB_FX="$FX" MJ_RELEASE_CHECK_REPO=o/r "$@" ./scripts/ci/release-check --seat ); }

# ---------------------------------------------------------------- 1. the age, reported
reset; propose $(( 30 * 60 )); pull_request $(( 30 * 60 ))
expect_exit 0 gate || { echo "    a record proposed half an hour ago was refused without --seat"; exit 1; }
expect_grep "^pending $TAG is published and its record is proposed on remotes/origin/release/record-$TAG \(0h old, grace 24h\) and not yet on this head, unlanded for (29|30|31) min" \
  || { echo "    release-check does not report the unlanded record's age"; exit 1; }
[ ! -s "$LOG" ] || { echo "    without --seat the forge was asked about a proposal git answered:"; cat "$LOG"; exit 1; }
# and without --seat a record past the seat's bound is still only pending: the structure job
# judges unrelated heads, which are not the record's to land
reset; propose $(( 121 * 60 )); pull_request $(( 121 * 60 ))
expect_exit 0 gate || { echo "    without --seat a record two hours old refused an unrelated head"; exit 1; }
expect_grep "unlanded for 12[01] min" || exit 1

# ---------------------------------------------------------------- 2. the seat's bound
# 2 hours and 1 minute: the finding, naming the pull request and its age
reset; propose $(( 121 * 60 )); pull_request $(( 121 * 60 ))
expect_exit 10 seat || { echo "    a record pull request 2h01m old passed the release seat's check"; exit 1; }
expect_grep "^FAIL  $TAG's record pull request #3202 \(https://example.invalid/pull/3202\) has waited 121 min unlanded, longer than the release seat's 120 min bound" \
  || { echo "    the seat's finding does not name the pull request and its age"; exit 1; }
expect_grep "approve its waiting validate run, or close and reopen it" || { echo "    the finding does not name the act that starts the record's run"; exit 1; }
expect_grep 'release-check: 1 finding\(s\)' || exit 1
grep -qF "repos/o/r/pulls?head=o:release/record-$TAG&state=open&per_page=5" "$LOG" \
  || { echo "    the record's pull request was not asked of the forge:"; cat "$LOG"; exit 1; }
# 119 minutes: inside the bound, pending, named
reset; propose $(( 119 * 60 )); pull_request $(( 119 * 60 ))
expect_exit 0 seat || { echo "    a record pull request 119 min old failed the release seat's check"; exit 1; }
expect_grep "^pending $TAG is published and its record pull request #3202 \(https://example.invalid/pull/3202\) has waited 119 min unlanded, inside the release seat's 120 min bound" \
  || { echo "    the pending record is not named with its pull request and age"; exit 1; }
expect_grep '\(1 pending\)$' || { echo "    the summary passes a waiting record off as recorded"; exit 1; }
expect_no_grep '^FAIL' || exit 1

# ---------------------------------------------------------------- 3. the pull request's own age
reset; propose $(( 5 * 3600 )); pull_request $(( 10 * 60 ))
expect_exit 0 seat || { echo "    a pull request opened 10 min ago failed for its branch's older tip"; exit 1; }
expect_grep "has waited (9|10|11) min unlanded" || exit 1
reset; propose $(( 10 * 60 )); pull_request $(( 3 * 3600 ))
expect_exit 10 seat || { echo "    a pull request three hours old passed for its branch's fresh tip"; exit 1; }
expect_grep "#3202 \(https://example.invalid/pull/3202\) has waited 1(79|80|81) min unlanded" || exit 1

# ---------------------------------------------------------------- 4. the forge cannot be asked
reset; propose $(( 121 * 60 )); : > "$FX/down"
expect_exit 10 seat || { echo "    an unreadable forge excused a record two hours old"; exit 1; }
expect_grep "^note  $TAG: no open pull request from release/record-$TAG could be read in o/r" || exit 1
expect_grep "^FAIL  $TAG's record pull request on remotes/origin/release/record-$TAG has waited 121 min unlanded" \
  || { echo    "the record's branch is not named when the forge cannot name its pull request"; exit 1; }
# no pull request open: the same, by its branch
reset; propose $(( 119 * 60 ))
expect_exit 0 seat || { echo "    a record branch 119 min old with no pull request listed failed"; exit 1; }
expect_grep "^pending $TAG is published and its record pull request on remotes/origin/release/record-$TAG has waited 119 min" || exit 1
# no repository known, and no gh: nothing is asked, the branch still bounds it
reset; propose $(( 121 * 60 ))
expect_exit 10 env -u MJ_RELEASE_CHECK_REPO PATH="$T/bin:$PATH" GH_STUB_LOG="$LOG" GH_STUB_FX="$FX" "$P/scripts/ci/release-check" --seat \
  || { echo    "a record two hours old passed with no GitHub repository known"; exit 1; }
expect_grep "^note  $TAG: no GitHub repository is known, so its pull request was not asked" || exit 1
[ ! -s "$LOG" ] || { echo "    the forge was asked with no repository known"; exit 1; }
mkdir -p "$T/nogh"
for tool in git jq sed awk grep sort date tr cut head cat dirname basename mkdir rm env bash sh printf; do
  w="$(command -v "$tool" 2>/dev/null || true)"; [ -z "$w" ] || ln -sf "$w" "$T/nogh/$tool"
done
expect_exit 10 env PATH="$T/nogh" MJ_RELEASE_CHECK_REPO=o/r "$P/scripts/ci/release-check" --seat \
  || { echo "    a record two hours old passed with gh missing"; exit 1; }
expect_grep "^note  $TAG: gh is not installed, so its pull request in o/r was not asked" || exit 1

# ---------------------------------------------------------------- 5. the knob, and what is unusable
reset; propose $(( 121 * 60 )); pull_request $(( 121 * 60 ))
expect_exit 0 seat MJ_RELEASE_RECORD_SEAT_MINUTES=180 || { echo "    MJ_RELEASE_RECORD_SEAT_MINUTES was not read"; exit 1; }
expect_grep "inside the release seat's 180 min bound" || exit 1
expect_exit 12 seat MJ_RELEASE_RECORD_SEAT_MINUTES=soon || { echo "    a bound that is not a number was not refused"; exit 1; }
expect_grep "MJ_RELEASE_RECORD_SEAT_MINUTES must be a whole number of minutes, not 'soon'" || exit 1
expect_exit 12 "$P/scripts/ci/release-check" --sat || { echo "    an unknown argument was not refused"; exit 1; }
expect_grep "unknown argument '--sat'" || exit 1

echo "    release-check reports an unlanded record's age; --seat fails a record pull request 2h01m old, naming it and its age, passes one 119 min old, counts the pull request's own age, and bounds it by its branch when the forge cannot be asked"

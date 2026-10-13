# majordomus-covers: none
# A release is cut when one can be cut, by an actor that asks on a clock, and an hour with
# nothing to cut says why (project.a-release-is-cut-on-the-hour, ADR 0151).
#
# The repository had every judge a release needs — the version's one authority, the `ci`
# verdict, the release workflow — and no actor that looked at the trunk and created the tag.
# `scripts/release-cadence` is that actor. It is driven here against a repository this case
# builds and a stub `gh` that answers the check-runs of exactly the commits it is told about,
# records every write it is asked for, and fails anything else:
#
#   cut          the newest green commit declares a version above the last release: the tag is
#                created at THAT commit — not at a newer red or unjudged tip — and the
#                release workflow is dispatched with it
#   dry run      the same decision, and no write at all
#   no verdict   nothing since the last release is green: nothing is cut, and the counts say so
#   not raised   the newest green commit still declares the released version: nothing is cut
#   tagged       the version already has a tag: nothing is cut, and no second tag is asked for
#   current      the trunk is the last release itself
#   record owed  the newest green commit lacks the last release's record: nothing is cut
#   lost race    the tag appeared while this run was deciding: said, not an error
#   unread       an API that cannot be read, a tag or a dispatch that is refused: exit 12
#   the workflow asks every hour, with the permissions to do this and none to change a branch
. "$ROOT/test/lib.sh"
ACTOR="$ROOT/scripts/release-cadence"
[ -x "$ACTOR" ] || { echo "    scripts/release-cadence is missing or not executable"; exit 1; }
command -v jq >/dev/null 2>&1 || skip "jq is required"

# A trunk: a release v1.0.0, then three landings. `land <version>` commits a manifest that
# declares it, on the first parent, and prints the commit.
R="$T/repo"; mkdir -p "$R/apps/majordomus-cli" "$R/scripts/ci"
cp "$ROOT/scripts/release-cadence" "$R/scripts/release-cadence"
cp "$ROOT/scripts/ci/release-verdict" "$R/scripts/ci/release-verdict"
gitq() { git -C "$R" -c user.email=t@e -c user.name=t -c commit.gpgsign=false -c core.hooksPath=/dev/null "$@"; }
gitq init -q -b master .
land() {   # each call is its own subshell, so the count is kept where they all see it
  local n; n=$(( $(cat "$T/landings" 2>/dev/null || echo 0) + 1 )); echo "$n" > "$T/landings"
  printf '[package]\nname = "majordomus-cli"\nversion = "%s"\n\n[dependencies]\nversion = "9.9.9"\n' "$1" > "$R/apps/majordomus-cli/Cargo.toml"
  echo "$n" > "$R/landing"
  # a landing holds the record of every release cut before it, unless it is told to be the
  # one that landed between a tag and its record
  rm -rf "$R/.ai/repo/releases"
  if [ -z "${NO_RECORD:-}" ]; then
    mkdir -p "$R/.ai/repo/releases"
    for t in $(gitq tag --list 'v*'); do echo "tag: $t" > "$R/.ai/repo/releases/$t.yaml"; done
  fi
  gitq add -A && gitq commit -qm "feat: landing $n" && gitq rev-parse HEAD
}
C0="$(land 1.0.0)"; gitq tag v1.0.0
C1="$(land 1.1.0)"   # raised, and green below
C2="$(land 1.1.0)"   # red
C3="$(land 1.1.0)"   # not judged yet: the tip

# The forge. A check-runs read is answered from runs/<sha>.json when this case wrote one, and
# is an empty list otherwise (a commit validate never ran on). The two writes are recorded and
# answered from writes/<n>, in order. Anything else is a call this actor has no business
# making.
mkdir -p "$T/bin" "$T/runs"
cat > "$T/bin/gh" <<STUB
#!/usr/bin/env bash
[ -f "$T/gh-fails" ] && { echo "HTTP 403: Resource not accessible by integration" >&2; exit 1; }
case "\$1 \$2" in
  "api repos/example/repo/commits/"*)
    sha="\${2#repos/example/repo/commits/}"; sha="\${sha%%/*}"
    if [ -f "$T/runs/\$sha.json" ]; then cat "$T/runs/\$sha.json"; else echo '{"total_count":0,"check_runs":[]}'; fi
    exit 0 ;;
  "api -X"|"workflow run")
    echo "\$*" >> "$T/writes.log"
    k="\$(wc -l < "$T/writes.log" | tr -d ' ')"
    if [ -f "$T/refuse-\$k" ]; then cat "$T/refuse-\$k" >&2; exit 1; fi
    exit 0 ;;
esac
echo "stub gh: unexpected call: \$*" >&2
exit 1
STUB
chmod +x "$T/bin/gh"
verdict() {   # <sha> <status> <conclusion-json>
  printf '{"total_count":1,"check_runs":[{"id":1,"name":"ci","status":"%s","conclusion":%s,"html_url":"https://example.invalid/run/1"}]}\n' "$2" "$3" > "$T/runs/$1.json"
}
act() { ( cd "$R" && PATH="$T/bin:$PATH" GITHUB_REPOSITORY=example/repo GITHUB_OUTPUT="$T/out" scripts/release-cadence --trunk master "$@" ); }
fresh() { rm -f "$T/writes.log" "$T/out" "$T"/refuse-* "$T/gh-fails"; }
writes() { [ -f "$T/writes.log" ] && wc -l < "$T/writes.log" | tr -d ' ' || echo 0; }

verdict "$C1" completed '"success"'
verdict "$C2" completed '"failure"'
verdict "$C3" in_progress null

echo "  a dry run decides and writes nothing"
fresh
expect_exit 0 act --dry-run || exit 1
expect_grep "would cut v1\.1\.0 at ${C1:0:12} \(green ci, 1\.1\.0 above v1\.0\.0\)" || exit 1
[ "$(writes)" = 0 ] || { echo "    a dry run wrote to the forge:"; cat "$T/writes.log"; exit 1; }
grep -qx "decision=cut" "$T/out" && grep -qx "reason=dry-run" "$T/out" && grep -qx "commit=$C1" "$T/out" \
  || { echo "    the dry run's outputs do not say what it decided:"; cat "$T/out"; exit 1; }

echo "  the newest green commit is tagged — not the red one after it, nor the unjudged tip"
fresh
expect_exit 0 act || exit 1
expect_grep "cut v1\.1\.0 at ${C1:0:12} .* and dispatched release\.yml with it" || exit 1
[ "$(sed -n 1p "$T/writes.log")" = "api -X POST repos/example/repo/git/refs -f ref=refs/tags/v1.1.0 -f sha=$C1" ] \
  || { echo "    the tag was not asked for at the green commit:"; cat "$T/writes.log"; exit 1; }
[ "$(sed -n 2p "$T/writes.log")" = "workflow run release.yml -R example/repo -f tag=v1.1.0" ] \
  || { echo "    the release workflow was not dispatched with the tag:"; cat "$T/writes.log"; exit 1; }
[ "$(writes)" = 2 ] || { echo "    $(writes) writes, not the two a cut is:"; cat "$T/writes.log"; exit 1; }
grep -qx "tag=v1.1.0" "$T/out" && grep -qx "reason=released" "$T/out" || { echo "    the outputs do not name the cut:"; cat "$T/out"; exit 1; }
if grep -q -e "$C2" -e "$C3" "$T/writes.log"; then echo "    a red or an unjudged commit was written to the forge"; exit 1; fi

echo "  nothing green since the release: nothing is cut, and the counts say why"
fresh
verdict "$C1" completed '"cancelled"'
expect_exit 0 act || exit 1
expect_grep "nothing to cut: none of the 3 newest commit\(s\) of master since v1\.0\.0 has a green ci \(2 concluded otherwise, 1 not judged yet\)" || exit 1
[ "$(writes)" = 0 ] || { echo "    a release was asked for with no green commit"; cat "$T/writes.log"; exit 1; }
grep -qx "reason=no-verdict" "$T/out" || { echo "    the reason is not no-verdict:"; cat "$T/out"; exit 1; }
# a commit validate never ran on is not judged, and is counted so
rm "$T/runs/$C2.json"
expect_exit 0 act || exit 1
expect_grep "\(1 concluded otherwise, 2 not judged yet\)" || exit 1
verdict "$C2" completed '"failure"'; verdict "$C1" completed '"success"'

echo "  a green commit that still declares the released version is not a release"
fresh
gitq tag v1.1.0 "$C1"   # the cut above, as the forge would now hold it
C4="$(land 1.1.0)"; verdict "$C4" completed '"success"'
expect_exit 0 act || exit 1
expect_grep "nothing to cut: the newest green commit ${C4:0:12} declares 1\.1\.0 and the last release is v1\.1\.0" || exit 1
grep -qx "reason=not-raised" "$T/out" || { echo "    the reason is not not-raised:"; cat "$T/out"; exit 1; }
[ "$(writes)" = 0 ] || { echo "    a release was asked for with no raised version"; exit 1; }

echo "  a version that already has a tag is not cut twice"
fresh
C5="$(land 1.2.0)"; verdict "$C5" completed '"success"'
# the tag is held by a commit of another line, which is no release of this trunk
side="$(gitq commit-tree -p "$C0" -m "feat: another line" "$(gitq rev-parse "$C0^{tree}")")"
gitq tag v1.2.0 "$side"
expect_exit 0 act || exit 1
expect_grep "nothing to cut: v1\.2\.0 already exists" || exit 1
grep -qx "reason=tagged" "$T/out" || { echo "    the reason is not tagged:"; cat "$T/out"; exit 1; }
[ "$(writes)" = 0 ] || { echo "    a second tag was asked for"; exit 1; }
gitq tag -d v1.2.0 >/dev/null

echo "  the tag appearing while the run decides is someone else's cut, not an error"
fresh
echo 'HTTP 422: Reference already exists (https://example.invalid)' > "$T/refuse-1"
expect_exit 0 act || exit 1
expect_grep "nothing to cut: v1\.2\.0 was created by someone else while this run was deciding" || exit 1
[ "$(writes)" = 1 ] || { echo "    the release was dispatched for a tag this run did not create"; cat "$T/writes.log"; exit 1; }

echo "  a refused tag, a refused dispatch and an unreadable forge are 12, each named"
fresh
echo 'HTTP 403: Resource not accessible by integration' > "$T/refuse-1"
expect_exit 12 act || exit 1
expect_grep "the tag v1\.2\.0 could not be created at ${C5:0:12}" || exit 1
fresh
echo 'HTTP 403: Resource not accessible by integration' > "$T/refuse-2"
expect_exit 12 act || exit 1
expect_grep "v1\.2\.0 is created at ${C5:0:12}, and release\.yml could not be dispatched with it" || exit 1
expect_grep "gh workflow run release\.yml -R example/repo -f tag=v1\.2\.0" || exit 1
fresh
: > "$T/gh-fails"
expect_exit 12 act || exit 1
expect_grep "the verdict of ${C5:0:12} could not be read, so nothing is cut" || exit 1
[ "$(writes)" = 0 ] || { echo "    something was written with the forge unreadable"; exit 1; }

echo "  the trunk that is the last release itself has nothing to cut"
fresh
gitq tag v1.2.0 "$C5"
expect_exit 0 act || exit 1
expect_grep "nothing to cut: master is v1\.2\.0 itself" || exit 1
grep -qx "reason=current" "$T/out" || { echo "    the reason is not current:"; cat "$T/out"; exit 1; }

echo "  a commit that landed between a tag and its record is not the next release"
fresh
C6="$(NO_RECORD=1 land 1.3.0)"; verdict "$C6" completed '"success"'
expect_exit 0 act || exit 1
expect_grep "nothing to cut: the newest green commit ${C6:0:12} declares 1\.3\.0, and the record of v1\.2\.0 \(\.ai/repo/releases/v1\.2\.0\.yaml\) is not in it" || exit 1
grep -qx "reason=record-owed" "$T/out" || { echo "    the reason is not record-owed:"; cat "$T/out"; exit 1; }
[ "$(writes)" = 0 ] || { echo "    a release was cut on top of an unrecorded one"; cat "$T/writes.log"; exit 1; }
# the record lands, green: that commit is the release
C7="$(land 1.3.0)"; verdict "$C7" completed '"success"'
expect_exit 0 act --dry-run || exit 1
expect_grep "would cut v1\.3\.0 at ${C7:0:12} \(green ci, 1\.3\.0 above v1\.2\.0\)" || exit 1

echo "  usage, and a trunk that is not there"
expect_exit 2 act --max 0 || exit 1
expect_exit 2 act --nonsense || exit 1
expect_exit 12 act --trunk no-such-ref || exit 1

# ---------------------------------------------------------------- the workflow
W="$ROOT/.github/workflows/cadence.yml"
[ -f "$W" ] || { echo "    no .github/workflows/cadence.yml: nothing asks on a clock"; exit 1; }
grep -qE "^    - cron: '[0-9]+ \* \* \* \*'$" "$W" || { echo "    the cadence does not ask every hour"; exit 1; }
grep -q 'scripts/release-cadence ' "$W" || { echo "    the workflow does not run the actor"; exit 1; }
for p in 'contents: write' 'actions: write' 'checks: read'; do
  grep -qE "^  $p\$" "$W" || { echo "    the workflow lacks '$p'"; exit 1; }
done
if grep -qE '^  (pull-requests|pages|id-token|deployments): ' "$W"; then echo "    the cadence holds a permission it has no use for"; exit 1; fi
if grep -qE 'git push|gh pr merge|release bump' "$W" "$ACTOR"; then
  # the actor names `release bump` once, in the sentence that says a landing raises the version
  [ "$(grep -cE 'git push|gh pr merge' "$W" "$ACTOR" | awk -F: '{s+=$2} END {print s}')" = 0 ] \
    || { echo "    the cadence pushes or merges; it may only tag and dispatch"; exit 1; }
fi
grep -q 'cancel-in-progress: false' "$W" || { echo "    a waiting decision would be cancelled"; exit 1; }
echo "    asked every hour; tags a green commit and dispatches; touches no branch"
exit 0

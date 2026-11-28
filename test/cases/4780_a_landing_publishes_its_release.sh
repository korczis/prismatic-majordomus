# majordomus-covers: none
# A landing publishes its release without a person (ADR 0240, I4811): scripts/ci/release-autotag
# over a fixture repository whose records the real plan engine reads, a real bare origin the
# tags are pushed to, and a stub `gh` on PATH, so nothing reaches GitHub.
#
#   1  publication off in .ai/repo/ci/release-auto.yaml: nothing, whatever landed
#   2  an issue DONE on a green commit, the crate declaring the unreleased 1.4.0: a dry run
#      names v1.4.0-dev.1 and writes nothing; --apply pushes exactly that annotated tag, naming
#      the issue, and dispatches release.yml for it once
#   3  a rerun over the tagged commit tags nothing and dispatches nothing; with no release run
#      listed (a dispatch that failed) it dispatches once, and only once; a published release
#      whose runs are not listed is not dispatched again
#   4  a red commit with a landed issue: held, no tag, no dispatch; once its ci is green it is
#      v1.4.0-dev.2, the counter read from the tags
#   5  a commit where nothing landed: none
#   6  a milestone DONE: the minor, v1.4.0, stable; an older untagged commit is behind it
#   7  an issue landing on a tree that declares the released 1.4.0: no tag, version.yml asked
#      to propose 1.5.0 unless release/bump-1.5.0 is already open; once the tree declares
#      1.5.0 the work since v1.4.0 is v1.5.0-dev.1
#   8  a verdict that cannot be read refuses (exit 12) and tags nothing
. "$ROOT/test/lib.sh"
mj_runs jq --version || skip "no jq ($(mj_runs_why jq))"
unset MJ_ROOT GITHUB_OUTPUT

"$MJ" init >/dev/null
git branch -M master
O="$T/.git/fixture-origin.git"; FX="$T/.git/fixture-gh"; LOG="$FX.log"; BIN="$T/.git/fixture-bin"
mkdir -p "$FX" "$BIN" apps/majordomus-cli .ai/repo/ci
git init -q --bare "$O"
git remote add origin "$O"
pj_init
pj_milestone M1 1
pj_milestone M2 2
pj_issue I0001 M1
pj_issue I0002 M1
pj_issue I0003 M2
crate() { printf '[package]\nname = "majordomus-cli"\nversion = "%s"\nedition = "2021"\n' "$1" > apps/majordomus-cli/Cargo.toml; }
switch() { printf 'version: 1\npublication: %s\n' "$1" > .ai/repo/ci/release-auto.yaml; }
commit() { git add -A >/dev/null && git commit -qm "$1" && git rev-parse HEAD; }
# land <issue>: the issue reaches DONE. start, verify and evidence are the plan's own shell
# transitions; `plan done` is the executable's, and a case never builds one (ADR 0224), so the
# completion stamp is sealed here exactly as plan.rs seal() and lib/project.sh mj_pj_seal_rows
# do (case 581 holds the seal; case 41 the transition). The engine then derives DONE itself.
land() {
  local at f="$FX/seal" i=".ai/repo/project/issues/$1.yaml"
  "$MJ" plan start "$1" >/dev/null && "$MJ" plan verify "$1" >/dev/null \
    && "$MJ" plan evidence "$1" --covers proof --type test --command true --result ok >/dev/null \
    || { echo "    the fixture could not move $1 to VERIFY"; exit 1; }
  at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf 'majordomus.plan-event/v1\nplan_done\n%s\n%s\n' "$1" "$at" > "$f"
  awk -v a="completed_at: $at" -v e="completed_event: sha256:$(sha256_of_file "$f")" \
    '/^evidence:/ && !d { print a; print e; d = 1 } { print }' "$i" > "$f.yaml" && mv "$f.yaml" "$i"
  [ "$("$MJ" --json plan list | jq -r --arg i "$1" '.issues[] | select(.id == $i) | .status')" = DONE ] \
    || { echo "    the fixture could not land $1:"; "$MJ" plan validate; exit 1; }
}
crate 1.3.0; switch automatic
A="$(commit "the tree v1.3.0 was cut from")"
git tag v1.3.0
git push -q origin master v1.3.0

cat > "$BIN/gh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$GH_STUB_LOG"
a="$*"
case "$a" in
  "api repos/o/r/commits/"*"/check-runs"*)
    [ ! -f "$GH_STUB_FX/down" ] || { echo "HTTP 503" >&2; exit 1; }
    sha="${a#api repos/o/r/commits/}"; sha="${sha%%/*}"
    c="success"; [ ! -f "$GH_STUB_FX/red-$sha" ] || c="failure"
    printf '{"check_runs":[{"id":7,"name":"ci","status":"completed","conclusion":"%s","html_url":"https://example.invalid/ci"}]}\n' "$c" ;;
  "api repos/o/r/actions/workflows/release.yml/runs?branch="*)
    t="${a#*branch=}"; t="${t%%&*}"
    cat "$GH_STUB_FX/runs-$t" 2>/dev/null || echo 0 ;;
  "api repos/o/r/releases/tags/"*)
    t="${a#api repos/o/r/releases/tags/}"; t="${t%% *}"
    [ -f "$GH_STUB_FX/published-$t" ] || { echo "gh: Not Found (HTTP 404)" >&2; exit 1; }
    echo "$t" ;;
  "workflow run release.yml --repo o/r --ref "*)
    [ ! -f "$GH_STUB_FX/no-dispatch" ] || exit 1
    t="${a#workflow run release.yml --repo o/r --ref }"; t="${t%% *}"
    echo 1 > "$GH_STUB_FX/runs-$t" ;;
  "pr list --repo o/r --head release/bump-"*)
    v="${a#*release/bump-}"; v="${v%% *}"
    cat "$GH_STUB_FX/open-$v" 2>/dev/null || echo 0 ;;
  "workflow run version.yml --repo o/r --ref master -f to="*) ;;
  *) echo "stub gh: no answer for $a" >&2; exit 1 ;;
esac
STUB
chmod +x "$BIN/gh"
autotag() { env PATH="$BIN:$PATH" GH_STUB_LOG="$LOG" GH_STUB_FX="$FX" GITHUB_REPOSITORY=o/r GITHUB_OUTPUT="${AUTOTAG_OUTPUT:-}" \
  "$ROOT/scripts/ci/release-autotag" "$@"; }
remote_tags() { git ls-remote --tags "$O" | sed -n 's|.*refs/tags/\([^^]*\)$|\1|p' | LC_ALL=C sort | tr '\n' ' '; }
dispatches() { grep -c "^workflow run $1 " "$LOG" 2>/dev/null || true; }

# ---------------------------------------------------------------- 1. off
git checkout -q -b switched-off
switch off; land I0001; crate 1.4.0
commit "landed, with publication off" >/dev/null
: > "$LOG"
expect_exit 0 autotag --apply || { echo "    publication off was refused"; exit 1; }
expect_grep 'publication is off in .ai/repo/ci/release-auto.yaml' || exit 1
expect_grep '^release-autotag: decision off at ' || exit 1
[ "$(remote_tags)" = "v1.3.0 " ] || { echo "    publication off tagged: $(remote_tags)"; exit 1; }
[ ! -s "$LOG" ] || { echo "    publication off asked the forge:"; cat "$LOG"; exit 1; }
git checkout -q master

# ---------------------------------------------------------------- 2. a landed issue: one dev tag
land I0001; crate 1.4.0
B="$(commit "I0001 lands, and the tree declares 1.4.0")"
: > "$LOG"
expect_exit 0 autotag || { echo "    the dry run failed"; exit 1; }
expect_grep '^release-autotag: issues DONE since v1.3.0: I0001$' || exit 1
expect_grep "^release-autotag: decision dev v1.4.0-dev.1 at ${B:0:12}$" || { echo "    the dry run does not name v1.4.0-dev.1"; exit 1; }
[ -z "$(git tag --list 'v1.4*')" ] || { echo "    the dry run tagged"; exit 1; }
[ "$(dispatches release.yml)" = 0 ] || { echo "    the dry run dispatched"; exit 1; }
out="$T/.git/fixture-output"; : > "$out"
AUTOTAG_OUTPUT="$out"
expect_exit 0 autotag --apply || { echo "    --apply failed"; exit 1; }
AUTOTAG_OUTPUT=""
[ "$(remote_tags)" = "v1.3.0 v1.4.0-dev.1 " ] || { echo "    origin holds $(remote_tags), not exactly v1.4.0-dev.1 beside v1.3.0"; exit 1; }
[ "$(git rev-parse 'v1.4.0-dev.1^{commit}')" = "$B" ] || { echo "    v1.4.0-dev.1 is not at the landed commit"; exit 1; }
[ "$(git cat-file -t v1.4.0-dev.1)" = tag ] || { echo "    v1.4.0-dev.1 is not an annotated tag"; exit 1; }
git cat-file -p v1.4.0-dev.1 | grep -q 'Issues landed since v1.3.0: I0001' || { echo "    the tag does not name what landed"; git cat-file -p v1.4.0-dev.1; exit 1; }
[ "$(dispatches release.yml)" = 1 ] || { echo "    release.yml was dispatched $(dispatches release.yml) times"; cat "$LOG"; exit 1; }
grep -qx 'workflow run release.yml --repo o/r --ref v1.4.0-dev.1 -f tag=v1.4.0-dev.1' "$LOG" || { echo "    the dispatch does not name the tag as its ref and input"; cat "$LOG"; exit 1; }
grep -qx 'decision=dev' "$out" && grep -qx 'tag=v1.4.0-dev.1' "$out" && grep -qx 'version=1.4.0-dev.1' "$out" \
  || { echo "    GITHUB_OUTPUT does not carry the decision:"; cat "$out"; exit 1; }

# ---------------------------------------------------------------- 3. a rerun tags and dispatches nothing new
: > "$LOG"
expect_exit 0 autotag --apply || { echo "    a rerun failed"; exit 1; }
expect_grep "^release-autotag: decision already v1.4.0-dev.1 at ${B:0:12}$" || exit 1
[ "$(remote_tags)" = "v1.3.0 v1.4.0-dev.1 " ] || { echo "    a rerun tagged twice: $(remote_tags)"; exit 1; }
[ "$(dispatches release.yml)" = 0 ] || { echo "    a rerun dispatched again"; exit 1; }
rm -f "$FX/runs-v1.4.0-dev.1"; : > "$LOG"
expect_exit 0 autotag --apply || exit 1
expect_grep 'no release run of v1.4.0-dev.1 is listed' || exit 1
[ "$(dispatches release.yml)" = 1 ] || { echo "    a tag with no release run was not dispatched once"; exit 1; }
: > "$LOG"
expect_exit 0 autotag --apply || exit 1
[ "$(dispatches release.yml)" = 0 ] || { echo    "the rerun after it dispatched again"; exit 1; }
# a published release whose runs are no longer listed is not released again
rm -f "$FX/runs-v1.4.0-dev.1"; touch "$FX/published-v1.4.0-dev.1"; : > "$LOG"
expect_exit 0 autotag --apply || exit 1
expect_grep 'v1.4.0-dev.1 is published and lists no release run; nothing to start' || exit 1
[ "$(dispatches release.yml)" = 0 ] || { echo "    a published release was dispatched again"; exit 1; }

# ---------------------------------------------------------------- 4. a red commit is held, then tagged once green
land I0002
C="$(commit "I0002 lands")"
touch "$FX/red-$C"; : > "$LOG"
expect_exit 0 autotag --apply || { echo "    a red commit was an error, not a held decision"; exit 1; }
expect_grep "concluded failure" || exit 1
expect_grep "^release-autotag: decision held at ${C:0:12}$" || exit 1
[ "$(remote_tags)" = "v1.3.0 v1.4.0-dev.1 " ] || { echo "    a red commit was tagged: $(remote_tags)"; exit 1; }
[ "$(dispatches release.yml)" = 0 ] || { echo "    a red commit was dispatched"; exit 1; }
rm -f "$FX/red-$C"
expect_exit 0 autotag --apply || exit 1
expect_grep '^release-autotag: issues DONE since v1.4.0-dev.1: I0002$' || exit 1
expect_grep "^release-autotag: decision dev v1.4.0-dev.2 at ${C:0:12}$" || exit 1
[ "$(remote_tags)" = "v1.3.0 v1.4.0-dev.1 v1.4.0-dev.2 " ] || { echo "    origin holds $(remote_tags)"; exit 1; }

# ---------------------------------------------------------------- 5. nothing landed
printf 'unrelated\n' > NOTES.md
D="$(commit "a change no record carries")"
expect_exit 0 autotag --apply || exit 1
expect_grep "^release-autotag: decision none at ${D:0:12}$" || { echo "    a commit that landed nothing was tagged"; exit 1; }

# ---------------------------------------------------------------- 6. a completed milestone is the minor
"$MJ" plan evidence M1 --covers proof --type test --command true --result "the outcome holds" >/dev/null
E="$(commit "M1 is proven")"
: > "$LOG"
expect_exit 0 autotag --apply || exit 1
expect_grep '^release-autotag: milestones DONE since v1.3.0: M1$' || exit 1
expect_grep "^release-autotag: decision minor v1.4.0 at ${E:0:12}$" || { echo "    a completed milestone is not the minor"; exit 1; }
[ "$(git rev-parse 'v1.4.0^{commit}')" = "$E" ] || { echo "    v1.4.0 is not at the commit that completed M1"; exit 1; }
[ "$(dispatches release.yml)" = 1 ] || exit 1
expect_exit 0 autotag --apply --commit "$D" || exit 1
expect_grep "^release-autotag: decision behind v1.4.0 at ${D:0:12}$" || { echo "    an older commit was not behind the newer release"; exit 1; }

# ---------------------------------------------------------------- 7. a released version cannot carry new work
land I0003
F="$(commit "I0003 lands on a tree that declares the released 1.4.0")"
echo 1 > "$FX/open-1.5.0"; : > "$LOG"
expect_exit 0 autotag --apply || exit 1
expect_grep '^release-autotag: release/bump-1.5.0 is already proposed$' || exit 1
expect_grep "^release-autotag: decision bump at ${F:0:12}$" || exit 1
[ "$(dispatches version.yml)" = 0 ] || { echo "    an open proposal was proposed again"; exit 1; }
rm -f "$FX/open-1.5.0"
expect_exit 0 autotag --apply || exit 1
grep -qx 'workflow run version.yml --repo o/r --ref master -f to=1.5.0' "$LOG" || { echo "    version.yml was not asked for 1.5.0"; cat "$LOG"; exit 1; }
[ -z "$(git tag --points-at "$F")" ] || { echo "    a tree declaring a released version was tagged"; exit 1; }
crate 1.5.0
G="$(commit "the bump to 1.5.0 lands")"
expect_exit 0 autotag --apply || exit 1
expect_grep '^release-autotag: issues DONE since v1.4.0: I0003$' || exit 1
expect_grep "^release-autotag: decision dev v1.5.0-dev.1 at ${G:0:12}$" || exit 1

# ---------------------------------------------------------------- 8. an unreadable verdict refuses
printf 'more\n' >> NOTES.md
commit "after the last release" >/dev/null
touch "$FX/down"
expect_exit 12 autotag --apply || { echo "    an unreadable verdict was not a refusal"; exit 1; }
expect_grep 'nothing is tagged unjudged' || exit 1
rm -f "$FX/down"
exit 0

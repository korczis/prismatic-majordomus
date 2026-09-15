# majordomus-covers: none
# A rebuild is asked for when it is the thing that helps, and never when it would preempt.
#
# An owed publication has two causes and two remedies. If gh-pages does not carry the trunk's
# site, deploying is the fix. If gh-pages is right and GitHub's own builder errored, deploying
# again changes nothing — the tree it would push is already there — and only a direct request
# repairs it. On 2026-09-14 three consecutive builds of a correct tree errored with "Page build
# failed." and no message, and a request built the identical commit successfully.
#
# Two hazards, both observed rather than imagined:
#
#   A request PREEMPTS a build in flight: asking while GitHub is building kills that build, so
#   one slow build becomes two failed ones. The predicate must be "this commit's build errored
#   AND nothing is building".
#
#   The state is not stable under one reading — `building` at 01:03 was `errored` minutes later,
#   and the repository-level status disagreed with the per-build list at the same moment.
#
# The fixture is a `gh` on PATH: every state is a row it prints, and the POST is a file it
# appends to, so "did it ask?" is a question with a yes-or-no answer rather than a guess.
. "$ROOT/test/lib.sh"

P="$ROOT/scripts/pages"
[ -x "$P" ] || { echo "    scripts/pages is missing"; exit 1; }
C=1111111111111111111111111111111111111111   # the commit under test
O=2222222222222222222222222222222222222222   # some other commit

mkdir -p "$T/bin"
cat > "$T/bin/gh" <<'STUB'
#!/bin/sh
# `gh api repos/<slug>/pages/builds --jq ...` prints the rows MJ_STUB_ROWS holds;
# `gh api -X POST repos/<slug>/pages/builds` records that it was asked.
case "$*" in
  *"-X POST"*) printf 'asked\n' >> "$MJ_STUB_POSTS"; printf '{"status":"queued"}\n'; exit 0 ;;
esac
[ -n "${MJ_STUB_ROWS:-}" ] && [ -f "$MJ_STUB_ROWS" ] || exit 1
cat "$MJ_STUB_ROWS"
STUB
chmod +x "$T/bin/gh"

rows() { printf '%s\n' "$@" > "$T/rows"; }
posts() { wc -l < "$T/posts" 2>/dev/null | tr -d ' '; }
rebuild() {
  : > "$T/posts"
  rc=0
  ( cd "$ROOT" && PATH="$T/bin:$PATH" GITHUB_REPOSITORY=owner/repo MJ_STUB_ROWS="$T/rows" MJ_STUB_POSTS="$T/posts" \
      "$P" rebuild --commit "$C" ) > "$T/out.txt" 2>&1 || rc=$?
}

# ---------------------------------------------------------------- 1. errored, nothing in flight
rows "errored	$C	Page build failed."
rebuild
[ "$rc" = 0 ] || { echo "    an errored build with nothing in flight answered $rc, not 0"; cat "$T/out.txt"; exit 1; }
[ "$(posts)" = 1 ] || { echo "    the rebuild was not requested ($(posts) request(s))"; cat "$T/out.txt"; exit 1; }
grep -q 'errored' "$T/out.txt" || { echo "    the output does not say why it asked"; cat "$T/out.txt"; exit 1; }
echo "    an errored build with nothing in flight is asked for again"

# ---------------------------------------------------------------- 2. this commit is building
rows "building	$C	no message"
rebuild
[ "$rc" = 12 ] || { echo "    a build in flight answered $rc, not 12"; cat "$T/out.txt"; exit 1; }
[ "$(posts)" = 0 ] || { echo "    a request was sent while GitHub was building: that preempts the build"; exit 1; }
grep -q 'preempt' "$T/out.txt" || { echo "    the refusal does not say what it protects"; cat "$T/out.txt"; exit 1; }
echo "    a build in flight is left alone, and the refusal names the preemption"

# ---------------------------------------------------------------- 3. another commit is building
#     The newest build is what "nothing is building" is asked about — a request is repository-wide
#     and would preempt a build of anything, not only of this commit.
rows "building	$O	no message" "errored	$C	Page build failed."
rebuild
[ "$rc" = 12 ] || { echo "    a build of another commit in flight answered $rc, not 12"; cat "$T/out.txt"; exit 1; }
[ "$(posts)" = 0 ] || { echo "    a request was sent while another commit was building"; exit 1; }
echo "    a build of another commit in flight is left alone too"

# ---------------------------------------------------------------- 4. already built: nothing to ask
rows "built	$C	no message"
rebuild
[ "$rc" = 0 ] || { echo "    a successful build answered $rc, not 0"; cat "$T/out.txt"; exit 1; }
[ "$(posts)" = 0 ] || { echo "    a rebuild was requested for a build that had succeeded"; exit 1; }
echo "    a build that succeeded is not asked for again"

# ---------------------------------------------------------------- 5. unreadable: refuse, never ask
: > "$T/rows"
rebuild
[ "$rc" = 12 ] || { echo "    an unreadable state answered $rc, not 12"; cat "$T/out.txt"; exit 1; }
[ "$(posts)" = 0 ] || { echo "    a request was sent without knowing the state"; exit 1; }
grep -q 'blindly' "$T/out.txt" || { echo "    the refusal does not say it will not ask blindly"; cat "$T/out.txt"; exit 1; }
echo "    a state that cannot be read is a refusal, not a blind request"

# ---------------------------------------------------------------- 6. the advice names the remedy
grep -q 'scripts/pages rebuild' "$P" \
  || { echo "    nothing points a reader at the remedy; 'dispatch the workflow' is a loop when the tree is already right"; exit 1; }
echo "    the errored verdict names the command that repairs it"

# ---------------------------------------------------------------- 7. the scheduled path chooses
# A remedy nothing calls is a remedy nobody has. The publisher's scheduled run finds a publication
# owed and then has to choose: deploy when gh-pages does not carry the tree, ask for a rebuild when
# it does and GitHub's build of it errored. Without the choice the loop redeploys an identical tree
# every thirty minutes, achieving nothing while looking like it is trying — the 2026-09-10 shape,
# where a built-then-errored commit left the site 27 minutes stale with every repository-owned
# signal green.
WF="$ROOT/.github/workflows/pages.yml"
[ -f "$WF" ] || { echo "    the pages workflow is missing"; exit 1; }
grep -q 'id: remedy' "$WF" \
  || { echo "    the scheduled path does not choose a remedy: one cause would be treated as both"; exit 1; }
awk '/id: remedy/{f=1} f{print} /^      - id: ask-rebuild/{exit}' "$WF" | grep -q 'pages built' \
  || { echo "    the remedy is chosen without reading the build state"; exit 1; }
awk '/id: ask-rebuild/{f=1} f{print} /^      - name:/{if(f) exit}' "$WF" | grep -q 'pages rebuild' \
  || { echo "    nothing asks for a rebuild: the errored-build cause has no remedy wired"; exit 1; }
grep -qE "steps\.remedy\.outputs\.remedy != 'rebuild'" "$WF" \
  || { echo "    the deploy steps are not held off when the remedy is a rebuild: it would deploy and ask"; exit 1; }
echo "    the scheduled path reads the state and chooses deploy or rebuild"

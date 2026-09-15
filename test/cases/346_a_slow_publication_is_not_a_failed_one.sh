# majordomus-covers: none
# `pages verify` distinguishes a publication in flight from one that failed.
#
# Publishing is two steps owned by two parties: this repository pushes gh-pages, and GitHub's
# own "pages build and deployment" turns that branch into the bytes a reader gets. `pages
# verify` watches only the second party's output — the commit the live site names — so when it
# times out it is seeing one symptom with two causes: GitHub has not finished, or the
# publication is broken.
#
# Until this, it reported the second in both cases: "after 180 s ... still serves X, not Y",
# exit 10. On 2026-09-15 at 01:05 a session hit exactly that while GitHub's build was in
# progress, and the only way to tell a slow success from a failure was to leave the tool and
# ask the API by hand. A check that cannot say "I do not know yet" reports every unfinished
# thing as a broken one — the same defect as a link checker denying a tag it was never given,
# from the other side.
#
# What is asserted: the three states are three, and the one that means "not yet" is not the
# one that means "no". The stub is `gh`, because that is the boundary where the unknown
# actually lives; the site probe is pointed at a URL that answers nothing, so every run here
# reaches the timeout branch and the verdict comes from the build status alone.
. "$ROOT/test/lib.sh"

command -v curl >/dev/null 2>&1 || { echo "    skip: no curl"; exit 0; }

TIP="$(git -C "$ROOT" rev-parse origin/gh-pages 2>/dev/null || echo 0000000000000000000000000000000000000000)"
WANT=1234567890abcdef1234567890abcdef12345678   # a commit the site will never serve
BIN="$T/bin"; mkdir -p "$BIN"

# `gh api ... --jq` emits tab-separated rows, which is what `pages built` parses. A stub that
# printed JSON would be ignored and every section below would pass through the fall-through
# branch — green, and proving nothing. That is how the first draft of this case was wrong.
stub_gh() {
  cat > "$BIN/gh" <<EOF
#!/bin/sh
printf '%s\t%s\tno message\n' "$1" "$TIP"
EOF
  chmod +x "$BIN/gh"
}

# an origin the probe cannot get an answer from, so the wait always ends in the timeout branch
DEAD=http://127.0.0.1:9

verify() {
  rc=0
  PATH="$BIN:$PATH" "$ROOT/scripts/pages" verify --url "$DEAD" --commit "$WANT" --timeout 0 \
    > "$T/v.out" 2> "$T/v.err" || rc=$?
}

# --- 1. unreachable is already its own state, and must stay one
verify
[ "$rc" = 12 ] || { echo "    a site that answers nothing exited $rc, not 12"; cat "$T/v.err"; exit 1; }
echo "    a site that cannot be reached is not reported as a failed publication"

# The remaining sections need the probe to reach something, so they run against the published
# site if it answers and are skipped if it does not — the subject here is the verdict, not the
# network, and a case that fails on connectivity teaches nobody anything.
LIVE="$(sed -n 's/^base_url = "\(.*\)"/\1/p' "$ROOT/site/config.toml")"
curl -fsS --max-time 10 "${LIVE%/}/build.json" >/dev/null 2>&1 || {
  echo "    (the published site did not answer; the served-commit sections need it)"; exit 0; }

verify_live() {
  rc=0
  PATH="$BIN:$PATH" "$ROOT/scripts/pages" verify --url "$LIVE" --commit "$WANT" --timeout 0 \
    > "$T/v.out" 2> "$T/v.err" || rc=$?
}

# --- 2. GitHub still building it: in flight, not failed
stub_gh building
verify_live
[ "$rc" = 12 ] || {
  echo "    a publication GitHub is still building exited $rc, not 12 (not a verdict)"
  cat "$T/v.err"; exit 1; }
grep -q 'in flight, not failed' "$T/v.err" || {
  echo "    it refused to decide, but does not say the publication is in flight:"; cat "$T/v.err"; exit 1; }
echo "    a publication GitHub is still building is reported as in flight, not as a failure"

# --- 3. the mutation: a build that errored is a failure, and says which half failed
# Without this, "never decide" would pass §2 and the command would have stopped reporting
# real breakage — trading a false failure for a missed one.
stub_gh errored
verify_live
[ "$rc" = 10 ] || {
  echo "    a build that errored exited $rc, not 10"; cat "$T/v.err"; exit 1; }
grep -q 'errored' "$T/v.err" || {
  echo "    the failure does not name GitHub's build as the half that failed:"; cat "$T/v.err"; exit 1; }
echo "    a build that errored is still a failure, and names which half of publishing failed"

# --- 4. built, and the site serves something else: the original verdict, unchanged
stub_gh built
verify_live
[ "$rc" = 10 ] || {
  echo "    a finished build over a site serving another commit exited $rc, not 10"; cat "$T/v.err"; exit 1; }
grep -q 'still serves' "$T/v.err" || {
  echo "    it no longer reports what the site actually serves:"; cat "$T/v.err"; exit 1; }
echo "    a finished build over a site serving another commit is the failure it always was"

echo "    a slow publication is not a failed one, and a failed one is still reported"

# majordomus-covers: none
# The pages job judges production only after the deployment of its own commit has arrived
# (I2711, evidence pages_waits_for_its_deployment_case).
#
# validate's `pages` job starts minutes after the push that also starts the deploy of the same
# commit. On 2026-10-14 run 38600347235 on ff38297096 read the site while that deploy was in
# flight — gh-pages' head was still the previous deploy, whose GitHub build had errored — and
# exited 10; a rerun of the job passed. `scripts/ci/pages-check --for COMMIT` now waits first,
# bounded (PAGES_CHECK_DEPLOY_WAIT), asking every PAGES_CHECK_POLL seconds and printing a line
# each time, until production serves COMMIT or a commit that carries it. Offline: the live
# site is a stub `curl` that answers build.json from a script of commits, the Actions API a
# stub `gh`, the repository a fixture with a trunk. Asked with --wait-only, section 0 alone:
#
#   1  production already serves the commit: ok at once, no wait line
#   2  production serves the previous commit while a pages run is in flight, then the commit:
#      one wait line per poll naming what is in flight, then ok
#   3  production serves a newer commit of the trunk: ok, the newer one carries it
#   4  an older commit and nothing changed since that the site is built from: ok, no deploy owed
#   5  an older commit and no deployment in flight: refused at once, naming both commits
#   6  an older commit with a deploy in flight past the bound: refused, naming both commits
#   7  the deployment's state unreadable: waited to the bound and refused, saying it could not read
#   8  a commit that is not on the trunk (a pull request's merge commit): no wait
#   9  the workflow passes its own commit and gives the job room for the wait
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/pages-check"
[ -x "$GATE" ] || { echo "    scripts/ci/pages-check is missing or not executable"; exit 1; }
V="$ROOT/.github/workflows/validate.yml"

R="$T/repo"; FX="$T/fx"; mkdir -p "$R" "$FX" "$T/bin"
g() { git -C "$R" "$@"; }
g init -q -b master
g config user.email t@example.com; g config user.name T
mkdir -p "$R/scripts/ci" "$R/.ai/repo/ci" "$R/site"
cp "$GATE" "$R/scripts/ci/pages-check"
printf 'deploy:\n  identity: build.json\n' > "$R/.ai/repo/ci/pages.yaml"
# the publication's inputs, as `scripts/pages paths` states them: site/** and nothing else
printf '#!/bin/sh\ncase "$1" in paths) echo "site/**" ;; *) exit 12 ;; esac\n' > "$R/scripts/pages"
chmod +x "$R/scripts/pages"
printf 'one\n' > "$R/site/index.md"; g add -A; g commit -qm "the published commit"
OLD="$(g rev-parse HEAD)"
printf 'two\n' > "$R/site/index.md"; g commit -qam "a change to the site"
SHA="$(g rev-parse HEAD)"
printf 'three\n' > "$R/site/index.md"; g commit -qam "a newer change to the site"
NEWER="$(g rev-parse HEAD)"
g checkout -q -b side "$OLD"
printf 'nothing the site reads\n' > "$R/README.md"; g add -A; g commit -qm "a branch commit"
BRANCHED="$(g rev-parse HEAD)"
g checkout -q master
printf 'readme\n' > "$R/README.md"; g add -A; g commit -qm "nothing the site is built from"
QUIET="$(g rev-parse HEAD)"
g branch -q gh-pages "$OLD"
g remote add origin "$R"; g fetch -q origin 'refs/heads/*:refs/remotes/origin/*'

# the live site: each call answers the next line of $FX/served, the last line for ever; an
# empty line is a site that does not answer
cat > "$T/bin/curl" <<'STUB'
#!/usr/bin/env bash
n="$(cat "$CURL_STUB_N" 2>/dev/null || echo 0)"; n=$((n + 1)); echo "$n" > "$CURL_STUB_N"
total="$(wc -l < "$CURL_STUB_SERVED" | tr -d ' ')"
[ "$n" -le "$total" ] || n="$total"
c="$(sed -n "${n}p" "$CURL_STUB_SERVED")"
[ -n "$c" ] || exit 22
printf '{"commit":"%s"}\n' "$c"
STUB
# the Actions API: the pages runs in flight, one file per status; $FX/down, unreadable
cat > "$T/bin/gh" <<'STUB'
#!/usr/bin/env bash
[ ! -f "$GH_STUB_FX/down" ] || exit 1
case "$*" in
  *"/actions/workflows/pages.yml/runs?status="*)
    a="$*"; s="${a#*status=}"; s="${s%%&*}"
    if [ -f "$GH_STUB_FX/pages-$s.json" ]; then cat "$GH_STUB_FX/pages-$s.json"; else echo '{"workflow_runs":[]}'; fi ;;
  *) exit 1 ;;
esac
STUB
chmod +x "$T/bin/curl" "$T/bin/gh"

served() { rm -f "$FX/n"; printf '%s\n' "$@" > "$FX/served"; }
in_flight() { printf '{"workflow_runs":[{"id":77,"status":"in_progress","head_sha":"%s"}]}' "$SHA" > "$FX/pages-in_progress.json"; }
nothing_in_flight() { rm -f "$FX"/pages-*.json "$FX/down"; }
check() { # <commit> [deploy-wait]
  ( cd "$R" && env -u GITHUB_REPOSITORY PATH="$T/bin:$PATH" CURL_STUB_N="$FX/n" CURL_STUB_SERVED="$FX/served" \
      GH_STUB_FX="$FX" PAGES_CHECK_REPO=o/r \
      ./scripts/ci/pages-check --for "$1" --wait-only --url https://site.invalid --poll 1 --deploy-wait "${2:-30}" )
}
s12() { printf '%s' "$1" | cut -c1-12; }

# ---------------------------------------------------------------- 1. already served
nothing_in_flight; served "$SHA"
expect_exit 0 check "$SHA" || { echo "    a site serving the commit was not accepted"; exit 1; }
expect_grep "^ok    production serves $(s12 "$SHA"), which carries $(s12 "$SHA")" || { echo "    the acceptance does not name the commit"; exit 1; }
expect_no_grep '^wait  ' || { echo "    it waited for a commit already served"; exit 1; }

# ---------------------------------------------------------------- 2. the deploy arrives while waited for
in_flight; served "$OLD" "$OLD" "$OLD" "$SHA"
expect_exit 0 check "$SHA" || { echo    "the deploy arrived inside the bound and the job was refused"; exit 1; }
waits="$(grep -c '^wait  ' <<<"$LAST_OUT" || true)"
[ "$waits" -ge 2 ] || { echo "    $waits wait line(s) for three polls of the previous commit:"; printf '%s\n' "$LAST_OUT"; exit 1; }
expect_grep "^wait  production serves $(s12 "$OLD"); waiting for $(s12 "$SHA") — pages run 77 \(in_progress" \
  || { echo "    a wait line does not say what is served, what is awaited and what is in flight"; exit 1; }
expect_grep 'of 30s, every 1s\)' || { echo    "a wait line does not state the bound and the poll"; exit 1; }
expect_grep "^ok    production serves $(s12 "$SHA")" || { echo "    the arrival was not said"; exit 1; }

# ---------------------------------------------------------------- 3. a newer commit carries it
nothing_in_flight; served "$NEWER"
expect_exit 0 check "$SHA" || { echo "    a site serving a newer trunk commit was refused"; exit 1; }
expect_grep "serves $(s12 "$NEWER"), which carries $(s12 "$SHA")" || exit 1

# ---------------------------------------------------------------- 4. nothing for the site since
nothing_in_flight; served "$NEWER"
expect_exit 0 check "$QUIET" || { echo "    a commit that changes nothing the site reads waited for a deploy that is not owed"; exit 1; }

# ---------------------------------------------------------------- 5. older, and nothing coming
nothing_in_flight; served "$OLD"
expect_exit 10 check "$SHA" 300 || { echo "    an older site with no deployment in flight was not refused"; exit 1; }
expect_grep "^FAIL  production serves $(s12 "$OLD"), older than $(s12 "$SHA"), and no deployment is in flight" \
  || { echo "    the refusal does not name both commits"; exit 1; }
expect_no_grep '^wait  ' || { echo "    a deploy that is not coming was waited for"; exit 1; }

# ---------------------------------------------------------------- 6. in flight past the bound
in_flight; served "$OLD"
expect_exit 10 check "$SHA" 2 || { echo "    a deploy slower than the bound was not refused"; exit 1; }
expect_grep "^FAIL  after [0-9]+s production serves $(s12 "$OLD")[0-9a-f]*, not $(s12 "$SHA") or a newer commit; the bound is 2s" \
  || { echo "    the refusal after the bound does not name both commits and the bound"; exit 1; }
expect_grep 'pages run 77 .* is in flight' || { echo "    the refusal does not say what was still in flight"; exit 1; }

# ---------------------------------------------------------------- 7. the deployment's state unreadable
nothing_in_flight; : > "$FX/down"; served "$OLD"
expect_exit 10 check "$SHA" 2 || { echo    "an unreadable deployment state with an older site ended other than at the bound"; exit 1; }
expect_grep "the deployment's state could not be read" || { echo "    the waits do not say the state was unreadable"; exit 1; }
expect_grep '^FAIL  after ' || { echo "    an unreadable state refused before the bound"; exit 1; }
rm -f "$FX/down"

# ---------------------------------------------------------------- 8. not on the trunk
nothing_in_flight; served "$OLD"
expect_exit 0 check "$BRANCHED" || { echo "    a commit that is not on master waited for a deployment"; exit 1; }
expect_grep "is not on master, so no deployment of it is owed" || exit 1

# ---------------------------------------------------------------- 9. the workflow
pages_job="$(awk '$0 == "  pages:" {f=1; next} /^  [a-z][a-z-]*:$/ {f=0} f' "$V")"
printf '%s\n' "$pages_job" | grep -qx '        run: scripts/ci/pages-check --for "$GITHUB_SHA"' \
  || { echo "    validate's pages job does not wait for the deployment of its own commit"; exit 1; }
job_min="$(printf '%s\n' "$pages_job" | sed -n 's/^    timeout-minutes: *\([0-9]*\).*/\1/p')"
deploy_wait="$(sed -n 's/^DEPLOY_WAIT="\${PAGES_CHECK_DEPLOY_WAIT:-\([0-9]*\)}"$/\1/p' "$GATE")"
wait2="$(sed -n 's/^WAIT="\${PAGES_CHECK_WAIT:-\([0-9]*\)}"$/\1/p' "$GATE")"
[ -n "$job_min" ] && [ -n "$deploy_wait" ] && [ -n "$wait2" ] || { echo "    the bounds could not be read (job '$job_min', deploy '$deploy_wait', serve '$wait2')"; exit 1; }
[ $(( job_min * 60 )) -gt $(( deploy_wait + wait2 )) ] \
  || { echo "    the pages job's ${job_min} min cannot hold the ${deploy_wait}s deploy wait and the ${wait2}s serve wait"; exit 1; }

echo "    the pages job waits, bounded and line by line, for production to serve its commit or one that carries it; it refuses early only when an older commit is served and nothing is in flight, otherwise at the bound, naming both commits"

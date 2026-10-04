# majordomus-covers: capability:served.observe capability:release.obligation
# majordomus-negative: capability:served.observe
# A deployment that fails is retried on the version that was integrated, never on a new one,
# and the work is not proven published until the site serves that commit at that version
# (ADR 0106: merge and deploy are evidence, not intent).
#
#   work integrated               1.4.0 -> 1.5.0 on the trunk
#   the deploy fails              the site still serves the old build      stale (10)
#   the retry advances nothing    release advance: satisfied, still 1.5.0
#   a half-done deploy            the new commit, but the old version       mismatched (10)
#   the deploy succeeds           the new commit at 1.5.0                   served (0)
#
# Integration, version and publication are three facts: none of the failures above lowers or
# raises the version, and none of them is read as published.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null || skip "jq absent"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN

"$MJ" init >/dev/null; "$MJ" update >/dev/null
printf '\nrelease:\n  cadence: minor\n' >> .ai/repo/policy.yaml
printf '.origin.git/\nsite/\n' >> .git/info/exclude
mkdir -p apps/majordomus-cli site
printf '[package]\nname = "majordomus-cli"\nversion = "1.4.0"\n' > apps/majordomus-cli/Cargo.toml
printf '[[package]]\nname = "majordomus-cli"\nversion = "1.4.0"\n' > apps/majordomus-cli/Cargo.lock
git add -A && git commit -qm base
OLD="$(git rev-parse HEAD)"
git init -q --bare "$PWD/.origin.git"; git remote add origin "$PWD/.origin.git"
git push -q origin HEAD:refs/heads/master && git fetch -q origin

declared() { sed -n 's/^version = "\(.*\)"$/\1/p' apps/majordomus-cli/Cargo.toml | head -1; }
publish() { printf '{"schema":1,"commit":"%s","dirty":false,"source_version":"%s"}\n' "$1" "$2" > site/build.json; }
observe() { # <expected-exit> <verdict>
  local rc=0
  "$RB" served observe --url "$HTTP_BASE" --commit "$NEW" --dry-run --format json > out.json 2>err.txt || rc=$?
  [ "$rc" = "$1" ] || { echo "    served observe exited $rc, expected $1"; cat out.json err.txt; exit 1; }
  [ "$(jq -r .observation.verdict out.json)" = "$2" ] \
    || { echo "    served observe judged $(jq -r .observation.verdict out.json), expected $2"; cat out.json; exit 1; }
}

# ---- work integrated, with its advance ------------------------------------------------
echo 'fn work() {}' > work.rs && git add work.rs && git commit -qm 'feat: work'
"$RB" release advance --base origin/master >/dev/null
git commit -qam 'chore(release): advance'
[ "$(declared)" = 1.5.0 ] || { echo "    the work did not advance to 1.5.0"; exit 1; }
git push -q origin HEAD:refs/heads/master && git fetch -q origin
NEW="$(git rev-parse HEAD)"

publish "$OLD" 1.4.0
start_http "$PWD/site" || skip "no python3 or node to serve the fixture site"
trap stop_http EXIT

# ---- the deploy failed: the site serves the previous build -----------------------------
observe 10 stale

# ---- the retry is a retry: the obligation holds, nothing is written --------------------
for retry in 1 2; do
  expect_exit 0 "$RB" release advance --base origin/master~2
  [ "$(declared)" = 1.5.0 ] || { echo "    retry $retry of the deploy advanced the version to $(declared)"; exit 1; }
done
[ -z "$(git status --porcelain --untracked-files=no)" ] || { echo "    a retry wrote to the tree:"; git status --short; exit 1; }

# ---- half a deploy: the new commit with the old version on its pages --------------------
publish "$NEW" 1.4.0
observe 10 mismatched

# ---- the deploy succeeds: the integrated commit at the integrated version ---------------
publish "$NEW" 1.5.0
observe 0 served
jq -e '.observation.reason | endswith("at version 1.5.0")' out.json >/dev/null \
  || { echo "    the served reason does not name the version"; cat out.json; exit 1; }

echo "    integrated 1.5.0; failed deploy stale, retry wrote nothing, half deploy mismatched, then served at 1.5.0"

# majordomus-covers: check
# claim: deployment-verified-live
# A deployment is verified by asking the deployed surface what it serves (ADR 0057, rule
# project.deployment-is-verified-live). A disposable repository declares its site's origin
# as a local HTTP server that serves a `build.json`; `deploy.verify` reports the same target
# verified, stale and unreachable in turn, a target the change does not reach is not asked,
# and the `verify` obligation reads the trunk before asking anything.
. "$ROOT/test/lib.sh"
BIN="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_BIN="$BIN" MAJORDOMUS_SHARE="$ROOT/share"
command -v jq >/dev/null 2>&1 || { echo "    skip: jq is not installed"; exit 0; }
command -v curl >/dev/null 2>&1 || { echo "    skip: curl is not installed"; exit 0; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj281.XXXXXX")"; trap 'stop_http; rm -rf "$S"' EXIT
git config user.email a@b.c; git config user.name a

mkdir -p "$S/www"
start_http "$S/www" || { echo "    skip: no http server runner (python3 or node)"; exit 0; }

"$MJ" init >/dev/null; "$MJ" update >/dev/null
# the CI model says what the site is built from; the site's configuration says where it is
mkdir -p .ai/repo/ci site docs lib
cat > .ai/repo/ci/gates.yaml <<'YAML'
version: 1
gates:
  - id: site-build
    job: site
    runs: "true"
    summary: the site builds
  - id: version-surface
    job: rust
    runs: "true"
    summary: the version is at least what the contract requires
classes:
  - id: docs
    paths: [docs/**, site/**]
    gates: [site-build]
  - id: rust
    paths: [lib/**]
    gates: [version-surface]
  - id: layer
    paths: [.ai/repo/**, AGENTS.md, CLAUDE.md, .bb/**, .gitignore]
    gates: []
YAML
printf 'title = "fixture"\nbase_url = "%s"\n' "$HTTP_BASE" > site/config.toml
echo '# guide' > docs/guide.md; echo 'a() { :; }' > lib/a.sh
git add -A >/dev/null && git commit -qm base
head="$(git rev-parse HEAD)"

verify() { "$BIN" run deploy.verify --input "$1" --quiet --format json --repo . 2>/dev/null | jq -c '.output'; }
status_of() { verify "$1" | jq -r --arg t "$2" '.verifications[] | select(.target == $t) | .status'; }

# ---------------------------------------------------------------- the plan is derived
plan="$("$BIN" run gates.completion --input '{"changed":["docs/guide.md"]}' --quiet --format json --repo . 2>/dev/null | jq -c '.output.deployment')"
printf '%s' "$plan" | jq -e '.targets[] | select(.id == "pages") | .applicable' >/dev/null \
  || { echo "    a documentation change does not reach the site: $plan"; exit 1; }
printf '%s' "$plan" | jq -e '[.targets[] | select(.id == "release")] | length == 0' >/dev/null \
  || { echo "    a repository with no release record has a release target"; exit 1; }
printf '%s' "$plan" | jq -r '.targets[] | select(.id == "pages") | .identity_url' | grep -q "$HTTP_BASE/build.json" \
  || { echo "    the site's identity address is not derived from its configuration"; exit 1; }

# ---------------------------------------------------------------- verified, stale, unreachable
printf '{"commit":"%s","source_version":"0.1.0"}\n' "$head" > "$S/www/build.json"
[ "$(status_of '{}' pages)" = verified ] || { echo "    a site serving HEAD is $(status_of '{}' pages)"; exit 1; }
verify '{}' | jq -e '.ok == true and .asked == 1' >/dev/null || { echo "    one verified target is not ok"; exit 1; }
# the deploy command exited 0 and the old revision stayed live
printf '{"commit":"%s","source_version":"0.0.9"}\n' "0000000000000000000000000000000000000000" > "$S/www/build.json"
[ "$(status_of '{}' pages)" = stale ] || { echo "    a site serving an older commit is $(status_of '{}' pages), not stale"; exit 1; }
verify '{}' | jq -r '.verifications[] | select(.target == "pages") | .detail' | grep -q 'is live' \
  || { echo "    a stale verification does not say what is live"; exit 1; }
verify '{}' | jq -e '.ok == false and (.refusing | index("pages")) != null' >/dev/null || { echo "    stale did not refuse"; exit 1; }
# a 200 that is not an identity
echo '<html>ok</html>' > "$S/www/build.json"
[ "$(status_of '{}' pages)" = unreadable ] || { echo "    a 200 with no identity is $(status_of '{}' pages)"; exit 1; }
# a target the change does not reach is not asked
printf '{"commit":"%s","source_version":"0.1.0"}\n' "$head" > "$S/www/build.json"
[ "$(status_of '{"changed":["lib/a.sh"]}' pages)" = not_applicable ] \
  || { echo "    a change outside the site's inputs asked the site anyway"; exit 1; }
verify '{"changed":["lib/a.sh"]}' | jq -e '.asked == 0 and .ok == false' >/dev/null \
  || { echo "    a verification that asked nothing reported ok"; exit 1; }
# the evidence carries the address asked and nothing it was not asked for
verify '{}' | jq -e '.verifications[0].asked != null' >/dev/null || { echo "    no address in the evidence"; exit 1; }
# the expected commit may be named
[ "$(status_of "{\"expected_commit\":\"0000000000000000000000000000000000000000\"}" pages)" = stale ] \
  || { echo "    an expected commit a caller names is not compared"; exit 1; }

# ---------------------------------------------------------------- the obligation reads the trunk first
expect_exit 0 "$MJ" start "publish it" --scope docs --requires verify
echo 'more' >> docs/guide.md
expect_exit 0 "$MJ" check
# no remote: nothing deployed can be compared with anything, and the recorded evidence stands
expect_grep 'obligation +verify .*(no remote|cannot be compared)'
git init -q --bare "$S/remote.git"; git remote add origin "$S/remote.git"
branch="$(git rev-parse --abbrev-ref HEAD)"
git push -q -u origin "$branch"; git remote set-head origin "$branch"
git add -A >/dev/null && git commit -qm 'docs: more'
expect_exit 0 "$MJ" check
expect_grep 'obligation +verify .*does not reach'
git push -q origin "$branch"
printf '{"commit":"%s","source_version":"0.1.0"}\n' "$(git rev-parse HEAD)" > "$S/www/build.json"
expect_exit 0 "$MJ" check
expect_grep 'OK +obligation +verify .*every applicable surface serves'
printf '{"commit":"%s","source_version":"0.1.0"}\n' "$head" > "$S/www/build.json"
expect_exit 0 "$MJ" check
expect_grep 'obligation +verify .*pages: stale'
# unreachable refuses as well
stop_http
expect_exit 0 "$MJ" check
expect_grep 'obligation +verify .*pages: unreachable'
echo "    ok"

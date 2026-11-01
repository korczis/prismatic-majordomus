# majordomus-covers: none
# A deploy of master keeps every pre-release's metadata the site already serves (I3448).
# A pre-release's publish job puts releases/<tag>.json on gh-pages before its record reaches
# master, and latest.json never names a pre-release (I2829), so the rule that keeps latest.json
# (ADR 0131) never kept it: the next ordinary deploy copied site/public over gh-pages and the
# tag file answered 404 while the GitHub release existed (v1.0.0-alpha.1 on 2026-10-20).
# scripts/site-deploy against a local bare remote, in a scratch repository that carries the
# script and the few files it reads, so the case needs no zola, no node and no network.
#
#   1. master's site names the stable v9.9.8; the pre-release v9.9.9-alpha.1 publishes its
#      metadata beside it with --release-metadata
#   2. a deploy of master whose build lacks that tag file keeps it, byte-identical, says so on
#      its output and in the gh-pages commit, and leaves latest.json the build's
#   3. negative: once the build holds a releases/<tag>.json, the build's copy is served, not the
#      one gh-pages carried, and nothing is said to be kept
#   4. latest.json as before: kept while it names a newer stable release than the build,
#      replaced once the build names that release or a newer one
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"

S="$(mktemp -d "${TMPDIR:-/tmp}/mj3721.XXXXXX")"; trap 'rm -rf "$S"' EXIT
gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
git init -q --bare "$S/remote.git"
git init -q -b master "$S/work"
cd "$S/work" || exit 1
git remote add origin "$S/remote.git"
mkdir -p scripts site/static/releases
cp "$ROOT/scripts/site-deploy" scripts/site-deploy
printf 'base_url = "https://example.invalid"\n' > site/config.toml
printf 'site/public/\n' > .gitignore

meta() {   # <version> [<channel>]: the metadata a release record projects, as release/v1 writes it
  printf '{\n  "schema": "release/v1",\n  "version": "%s",\n  "tag": "v%s",\n  "channel": "%s"\n}\n' \
    "$1" "$1" "${2:-stable}"
}
stable() {   # <version>: a stable release's two files under site/static/releases
  meta "$1" > "site/static/releases/v$1.json"; meta "$1" > site/static/releases/latest.json
}
build() {   # the site of HEAD as site/public, with every release file site/static holds
  rm -rf site/public; mkdir -p site/public/releases
  printf '<a href="/commit/%s">source</a>\n' "$(git rev-parse HEAD)" > site/public/index.html
  cp site/static/releases/*.json site/public/releases/
}
commit() { gitq add -A && gitq commit -qm "$1" && git push -q origin HEAD:master; }
blob() { git fetch -q origin gh-pages && git rev-parse -q --verify "origin/gh-pages:$1" 2>/dev/null; }
version_served() { git fetch -q origin gh-pages && git --no-pager show origin/gh-pages:releases/latest.json | jq -r .version; }
deploy() { scripts/site-deploy --skip-build "$@"; }
A=releases/v9.9.9-alpha.1.json

# ---------------------------------------------------------------- 1. stable, then the pre-release
stable 9.9.8
commit "release v9.9.8 recorded"
build
expect_exit 0 deploy
meta 9.9.9-alpha.1 prerelease > "site/static/$A"
git add site/static/releases
expect_exit 0 scripts/site-deploy --release-metadata v9.9.9-alpha.1
expect_grep "pushed .* to origin/gh-pages: release: v9.9.9-alpha.1 pre-release metadata"
published="$(blob "$A")" || { echo "    --release-metadata did not publish $A"; exit 1; }
[ "$(version_served)" = 9.9.8 ] || { echo "    the pre-release moved latest.json to $(version_served)"; exit 1; }
# back to master as committed: the pre-release's record has not landed
git reset -q -- site/static/releases; rm -f "site/static/$A"

# ---------------------------------------------------------------- 2. master moves before the record lands
echo change > change.txt && commit "an unrelated change"
build
[ ! -e "site/public/$A" ] || { echo "    the fixture's build holds $A; the case proves nothing"; exit 1; }
expect_exit 0 deploy
expect_grep "kept $A: gh-pages serves it and the build does not hold it"
[ "$(blob "$A")" = "$published" ] \
  || { echo "    a deploy of master removed or changed $A (served blob: $(blob "$A" || echo none), published: $published)"; exit 1; }
git --no-pager log -1 --format=%s origin/gh-pages | grep -q "release files kept: v9.9.9-alpha.1.json" \
  || { echo "    the gh-pages commit does not name the kept tag file: $(git log -1 --format=%s origin/gh-pages)"; exit 1; }
expect_no_grep 'kept releases/latest.json'
[ "$(version_served)" = 9.9.8 ] || { echo "    latest.json is $(version_served), not the build's 9.9.8"; exit 1; }
git --no-pager show origin/gh-pages:index.html | grep -q "/commit/$(git rev-parse HEAD)" \
  || { echo "    the deploy did not publish the new build"; exit 1; }

# ---------------------------------------------------------------- 3. the build's own copy wins
meta 9.9.9-alpha.1 prerelease | sed 's/"channel"/"recorded": true, "channel"/' > "site/static/$A"
commit "release v9.9.9-alpha.1 recorded"
build
expect_exit 0 deploy
expect_no_grep "kept $A"
[ "$(blob "$A")" = "$(git hash-object "site/public/$A")" ] \
  || { echo "    the build holds $A and gh-pages kept its own copy instead"; exit 1; }
[ "$(blob "$A")" != "$published" ] || { echo "    the fixture's two copies of $A are the same; the case proves nothing"; exit 1; }

# ---------------------------------------------------------------- 4. latest.json keeps its own rule
stable 9.10.0
git add site/static/releases
expect_exit 0 scripts/site-deploy --release-metadata v9.10.0
git reset -q -- site/static/releases; git checkout -q -- site/static/releases; rm -f site/static/releases/v9.10.0.json
echo change2 > change.txt && commit "another unrelated change"
build
expect_exit 0 deploy
expect_grep 'kept releases/latest.json at v9.10.0 \(9.10.0\): the build carries 9.9.8'
[ "$(version_served)" = 9.10.0 ] || { echo "    a deploy of master took latest.json back to $(version_served)"; exit 1; }
stable 9.10.0
commit "release v9.10.0 recorded"
build
expect_exit 0 deploy
expect_no_grep 'kept releases/latest.json'
expect_no_grep 'kept releases/v'
stable 9.10.1
commit "release v9.10.1 recorded"
build
expect_exit 0 deploy
[ "$(version_served)" = 9.10.1 ] || { echo "    a newer build did not replace latest.json: $(version_served)"; exit 1; }
exit 0

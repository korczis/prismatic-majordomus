# majordomus-covers: none
# A release publishes its own metadata to the site before its record lands, and the site never
# goes back a release (ADR 0131, issue I2303). scripts/site-deploy against a local bare remote,
# in a scratch repository that carries the script and the few files it reads, so the case needs
# no zola, no node and no network.
#
#   1. an ordinary deploy publishes the site of master, which names release v0.1.0
#   2. `--release-metadata v0.2.0` over a working tree that holds v0.2.0's metadata, staged and
#      uncommitted as the release's publish job holds it, changes exactly releases/latest.json
#      and releases/v0.2.0.json on gh-pages: index.html and build.json stay master's
#   3. it refuses a tag the metadata does not name, a latest.json that is not the tag's file, and
#      a release older than the one the site serves; none of them pushes
#   4. a deploy of master before the record lands keeps v0.2.0 as the latest release and says so,
#      while every other file is the new build's
#   5. once the build itself names v0.2.0, nothing is kept; a newer build replaces it
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"

S="$(mktemp -d "${TMPDIR:-/tmp}/mj1041.XXXXXX")"; trap 'rm -rf "$S"' EXIT
gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
git init -q --bare "$S/remote.git"
git init -q -b master "$S/work"
cd "$S/work" || exit 1
git remote add origin "$S/remote.git"
mkdir -p scripts site/static/releases
cp "$ROOT/scripts/site-deploy" scripts/site-deploy
printf 'base_url = "https://example.invalid"\n' > site/config.toml
printf 'site/public/\n' > .gitignore

meta() {   # <version>: the metadata a release record projects, as release/v1 writes it
  printf '{\n  "schema": "release/v1",\n  "version": "%s",\n  "tag": "v%s",\n  "channel": "stable"\n}\n' "$1" "$1"
}
static() {   # <version>: the release's two files under site/static/releases
  meta "$1" > "site/static/releases/v$1.json"; meta "$1" > site/static/releases/latest.json
}
build() {   # the site of HEAD as site/public, naming the release site/static says is the latest
  rm -rf site/public; mkdir -p site/public/releases
  printf '<a href="/commit/%s">source</a>\n' "$(git rev-parse HEAD)" > site/public/index.html
  printf '{"commit":"%s"}\n' "$(git rev-parse HEAD)" > site/public/build.json
  cp site/static/releases/*.json site/public/releases/
}
served() { git fetch -q origin gh-pages && git --no-pager show "origin/gh-pages:$1"; }
version_served() { served releases/latest.json | jq -r .version; }
deploy() { scripts/site-deploy --skip-build "$@"; }

# ---------------------------------------------------------------- 1. master's site, v0.1.0
static 0.1.0
gitq add -A && gitq commit -qm "release v0.1.0 recorded" && git push -q origin HEAD:master
build
expect_exit 0 deploy
[ "$(version_served)" = 0.1.0 ] || { echo "    the first deploy serves $(version_served), not 0.1.0"; exit 1; }
M1="$(git rev-parse HEAD)"
pages_before="$(git rev-parse origin/gh-pages)"

# ---------------------------------------------------------------- 2. the release's metadata alone
static 0.2.0
git add site/static/releases
expect_exit 0 scripts/site-deploy --release-metadata v0.2.0
expect_grep "pushed .* to origin/gh-pages: release: v0.2.0 metadata"
[ "$(version_served)" = 0.2.0 ] || { echo "    after --release-metadata the site serves $(version_served), not 0.2.0"; exit 1; }
served releases/v0.2.0.json | jq -e '.tag == "v0.2.0"' >/dev/null || { echo "    releases/v0.2.0.json is not served"; exit 1; }
changed="$(git --no-pager diff --name-only "$pages_before" origin/gh-pages | tr '\n' ' ')"
[ "$changed" = "releases/latest.json releases/v0.2.0.json " ] || { echo "    --release-metadata changed [$changed], not the two release files"; exit 1; }
served build.json | jq -e --arg c "$M1" '.commit == $c' >/dev/null || { echo "    build.json no longer names master's commit"; exit 1; }
[ "$(git rev-parse HEAD)" = "$M1" ] || { echo "    the working tree's branch moved"; exit 1; }

# ---------------------------------------------------------------- 3. what it refuses
pages_now="$(git rev-parse origin/gh-pages)"
expect_exit 10 scripts/site-deploy --release-metadata v0.3.0
expect_grep 'REFUSE .*v0.3.0.json is missing'
meta 0.2.0 > site/static/releases/v0.3.0.json
expect_exit 10 scripts/site-deploy --release-metadata v0.3.0
expect_grep 'REFUSE .*latest.json names v0.2.0, not v0.3.0'
rm site/static/releases/v0.3.0.json
printf '{"schema":"release/v1","version":"0.2.0","tag":"v0.2.0","channel":"beta"}\n' > site/static/releases/v0.2.0.json
expect_exit 10 scripts/site-deploy --release-metadata v0.2.0
expect_grep 'REFUSE .*differ'
# back to master as committed: v0.2.0 was never recorded there
git reset -q -- site/static/releases; git checkout -q -- site/static/releases; rm -f site/static/releases/v0.2.0.json
expect_exit 10 scripts/site-deploy --release-metadata v0.1.0
expect_grep 'REFUSE .*serves 0.2.0, newer than 0.1.0'
git fetch -q origin gh-pages
[ "$(git rev-parse origin/gh-pages)" = "$pages_now" ] || { echo "    a refused --release-metadata pushed gh-pages"; exit 1; }

# ---------------------------------------------------------------- 4. master moves before the record lands
[ -z "$(git status --porcelain --untracked-files=no)" ] || { echo "    the case left the tree dirty:"; git status --short; exit 1; }
echo change > change.txt && gitq add change.txt && gitq commit -qm "an unrelated change" && git push -q origin HEAD:master
build
expect_exit 0 deploy
expect_grep 'kept releases/latest.json at v0.2.0 \(0.2.0\): the build carries 0.1.0'
[ "$(version_served)" = 0.2.0 ] || { echo "    a deploy of master took the site back to $(version_served)"; exit 1; }
served releases/v0.2.0.json >/dev/null 2>&1 || { echo "    the kept release lost its tag file"; exit 1; }
served index.html | grep -q "/commit/$(git rev-parse HEAD)" || { echo "    the deploy did not publish the new build"; exit 1; }
git --no-pager log -1 --format=%s origin/gh-pages | grep -q 'release metadata kept at v0.2.0' || { echo "    the gh-pages commit does not say what it kept"; exit 1; }

# ---------------------------------------------------------------- 5. the record lands, then a newer release
static 0.2.0
gitq add -A && gitq commit -qm "release v0.2.0 recorded" && git push -q origin HEAD:master
build
expect_exit 0 deploy
expect_no_grep 'kept releases/latest.json'
[ "$(version_served)" = 0.2.0 ] || { echo "    after the record landed the site serves $(version_served)"; exit 1; }
static 0.3.0
gitq add -A && gitq commit -qm "release v0.3.0 recorded" && git push -q origin HEAD:master
build
expect_exit 0 deploy
[ "$(version_served)" = 0.3.0 ] || { echo "    a newer build did not replace the release: $(version_served)"; exit 1; }
exit 0

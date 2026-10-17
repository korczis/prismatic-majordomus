# majordomus-covers: none
# A pre-release publishes its own metadata beside the latest release and never as it (I2829).
# scripts/site-deploy against a local bare remote, in a scratch repository that carries the
# script and the few files it reads, as case 1041 does; no zola, no node, no network.
#
#   1. a stable release's --release-metadata publishes latest.json and its tag file, as before
#   2. `--release-metadata v1.0.0-alpha.1` over a tree whose latest.json still names the stable
#      release changes exactly releases/v1.0.0-alpha.1.json on gh-pages: latest.json stays
#   3. a latest.json naming the pre-release is refused, and nothing is pushed
#   4. a suffix that is not a pre-release (v0.2.0-ai-documents) takes the stable path and is
#      refused there, because latest.json does not name it
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"

S="$T/s"; mkdir -p "$S"
gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
git init -q --bare "$S/remote.git"
git init -q -b master "$S/work"
cd "$S/work" || exit 1
git remote add origin "$S/remote.git"
mkdir -p scripts site/static/releases
cp "$ROOT/scripts/site-deploy" scripts/site-deploy
printf 'base_url = "https://example.invalid"\n' > site/config.toml
printf 'site/public/\n' > .gitignore

meta() {   # <version> [channel]: the metadata a release record projects
  printf '{\n  "schema": "release/v1",\n  "version": "%s",\n  "tag": "v%s",\n  "channel": "%s"\n}\n' "$1" "$1" "${2:-stable}"
}
build() {
  rm -rf site/public; mkdir -p site/public/releases
  printf '<a href="/commit/%s">source</a>\n' "$(git rev-parse HEAD)" > site/public/index.html
  printf '{"commit":"%s"}\n' "$(git rev-parse HEAD)" > site/public/build.json
  cp site/static/releases/*.json site/public/releases/
}
served() { git fetch -q origin gh-pages && git --no-pager show "origin/gh-pages:$1"; }
tag_served() { served releases/latest.json | jq -r .tag; }

# ---------------------------------------------------------------- 1. the stable release, as before
meta 0.29.0 > site/static/releases/v0.29.0.json; meta 0.29.0 > site/static/releases/latest.json
gitq add -A && gitq commit -qm "release v0.29.0 recorded" && git push -q origin HEAD:master
build
expect_exit 0 scripts/site-deploy --skip-build
[ "$(tag_served)" = v0.29.0 ] || { echo "    the first deploy serves $(tag_served), not v0.29.0"; exit 1; }
before="$(git rev-parse origin/gh-pages)"

# ---------------------------------------------------------------- 2. the pre-release beside it
meta 1.0.0-alpha.1 prerelease > site/static/releases/v1.0.0-alpha.1.json
git add site/static/releases
expect_exit 0 scripts/site-deploy --release-metadata v1.0.0-alpha.1
expect_grep "pushed .* to origin/gh-pages: release: v1.0.0-alpha.1 pre-release metadata, published beside the latest release"
changed="$(git --no-pager diff --name-only "$before" origin/gh-pages | tr '\n' ' ')"
[ "$changed" = "releases/v1.0.0-alpha.1.json " ] \
  || { echo "    --release-metadata of a pre-release changed [$changed], not its tag file alone"; exit 1; }
[ "$(tag_served)" = v0.29.0 ] \
  || { echo "    a pre-release became the latest release: latest.json names $(tag_served)"; exit 1; }
served releases/v1.0.0-alpha.1.json | jq -e '.tag == "v1.0.0-alpha.1"' >/dev/null \
  || { echo "    the pre-release's tag file is not served"; exit 1; }

# ---------------------------------------------------------------- 3. a latest.json naming it is refused
now="$(git rev-parse origin/gh-pages)"
meta 1.0.0-alpha.1 prerelease > site/static/releases/latest.json
git add site/static/releases
expect_exit 10 scripts/site-deploy --release-metadata v1.0.0-alpha.1
expect_grep 'REFUSE .*latest.json names the pre-release v1.0.0-alpha.1; a pre-release is never the latest release'
git fetch -q origin gh-pages
[ "$(git rev-parse origin/gh-pages)" = "$now" ] || { echo "    a refused pre-release deploy pushed gh-pages"; exit 1; }
[ "$(tag_served)" = v0.29.0 ] || { echo "    the refusal left latest.json naming $(tag_served)"; exit 1; }

# ---------------------------------------------------------------- 4. another suffix is not a pre-release
meta 0.29.0 > site/static/releases/latest.json
meta 0.2.0-ai-documents > site/static/releases/v0.2.0-ai-documents.json
git add site/static/releases
expect_exit 10 scripts/site-deploy --release-metadata v0.2.0-ai-documents
expect_grep 'REFUSE .*latest.json names v0.29.0, not v0.2.0-ai-documents'
echo "    a pre-release publishes its tag file beside the latest release and never replaces it; a latest.json naming one is refused; any other suffix is held to the stable rules"

# majordomus-covers: none
# scripts/site-deploy publishes site/public to the gh-pages branch of a remote, and refuses
# what nobody could audit: a dirty tree, a commit master does not contain, a build that is
# not HEAD's. The remote here is a local bare repository, so the case needs no network and
# no GitHub; the deploy path is the same one the Pages workflow and a person run.
#
# Skips itself without zola or node_modules, as 12_site_build does; the build is real.
. "$ROOT/test/lib.sh"
command -v zola >/dev/null || { echo "    zola absent; skipping"; exit 0; }
[ -d "$ROOT/node_modules/tailwindcss" ] || { echo "    node_modules absent; skipping"; exit 0; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj96.XXXXXX")"; trap 'rm -rf "$S"' EXIT

# a clone of this checkout's HEAD with a bare remote of its own, so nothing here touches origin
git init -q --bare "$S/remote.git"
git clone -q "$ROOT" "$S/work" 2>/dev/null
cd "$S/work" || exit 1
git remote set-url origin "$S/remote.git"
git push -q origin HEAD:refs/heads/master
ln -s "$ROOT/node_modules" node_modules
# the derived-data check reads the tool version from bin/majordomus and the commit from git,
# and the Rust check needs a built executable; both belong to CI's gate, so the case builds
# site/public itself and publishes with --skip-build, which is how the workflow calls it too.
expect_exit 0 scripts/site-build
# the build regenerates site/data/generated/source.json with a generation time; that is
# the build's, not a change of ours, and the deploy's dirty-tree check must see a clean tree
git checkout -q -- .

# --- a dirty tree is refused, nothing pushed
echo "x" >> README.md
expect_exit 10 scripts/site-deploy --skip-build
expect_grep 'REFUSE .*uncommitted'
git checkout -q -- README.md
[ -z "$(git ls-remote --heads origin gh-pages)" ] || { echo "    a refused deploy pushed gh-pages"; exit 1; }

# --- a commit off master is refused without --any-ref
git checkout -q -b preview
echo "preview" > PREVIEW.md && git add PREVIEW.md && git -c user.name=t -c user.email=t@t commit -qm preview
expect_exit 10 scripts/site-deploy --skip-build
expect_grep 'REFUSE .*not on origin/master'
git checkout -q master && git branch -q -D preview

# --- a dry run commits nothing to the remote and names what it would push
expect_exit 0 scripts/site-deploy --skip-build --dry-run
expect_grep 'would push .*/gh-pages: deploy: site from'
[ -z "$(git ls-remote --heads origin gh-pages)" ] || { echo "    a dry run pushed gh-pages"; exit 1; }

# --- the deploy: gh-pages is created, carries the site, .nojekyll and the source commit
head="$(git rev-parse HEAD)"
expect_exit 0 scripts/site-deploy --skip-build
expect_grep "pushed .* to origin/gh-pages"
git fetch -q origin gh-pages
git show origin/gh-pages:index.html | grep -q "/commit/$head" || { echo "    the published index does not name the source commit"; exit 1; }
git show origin/gh-pages:.nojekyll >/dev/null 2>&1 || { echo "    .nojekyll missing on gh-pages"; exit 1; }
git show origin/gh-pages:guarantees/index.html >/dev/null 2>&1 || { echo "    /guarantees/ missing on gh-pages"; exit 1; }
git log -1 --format=%B origin/gh-pages | grep -q "source: $head" || { echo "    the gh-pages commit does not name its source"; exit 1; }

# --- the same site again: nothing to push, no second commit
n1="$(git rev-list --count origin/gh-pages)"
expect_exit 0 scripts/site-deploy --skip-build
expect_grep 'already serves .*; nothing to push'
git fetch -q origin gh-pages
[ "$(git rev-list --count origin/gh-pages)" = "$n1" ] || { echo "    an unchanged site made a new gh-pages commit"; exit 1; }

# --- a site not built from HEAD is refused: the footer names another commit
git -c user.name=t -c user.email=t@t commit -q --allow-empty -m "move head"
expect_exit 10 scripts/site-deploy --skip-build --any-ref
expect_grep 'REFUSE .*not built from HEAD'

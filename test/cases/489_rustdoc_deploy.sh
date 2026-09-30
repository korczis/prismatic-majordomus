# majordomus-covers: none
# claims: rustdoc-published
# The crate's rustdoc is published with the site at /rustdoc, built from the commit the site
# names, and never committed (ADR 0086). The deploy is scripts/site-deploy, the one path the
# Pages workflow and a person both run, to a local bare remote: no network, no GitHub.
#
# The regressions this case exists for: a reference that the build composed and the deploy
# left behind, one published beside a site of another commit with nothing refusing it, and one
# that is committed to the source branch, where it would be 80 MB of pages that go stale on
# the next commit. So the case asks the source branch for the tree (it must not be there), asks
# the site's own check to refuse a composed tree that declares another commit than the build,
# deploys, and reads the reference and both statements of its commit back from the published
# branch.
#
# The tree is a producer output, never committed, so the clone borrows this checkout's as case
# 96 does, and it must be this commit's: the claim is a reference built from the commit the site
# names. Under CI the suite job produced it from this commit first (.github/actions/rustdoc),
# and the job installs zola, the site's node_modules and jq, so any of them absent, or a tree
# of another commit, fails the case; anywhere else the case says what it did not measure and
# ends (skip_case).
. "$ROOT/test/lib.sh"
command -v zola >/dev/null || skip_case "zola is absent, so no site was built or deployed"
[ -d "$ROOT/node_modules/tailwindcss" ] || skip_case "node_modules is absent, so no site was built or deployed"
command -v jq >/dev/null 2>&1 || skip_case "no jq, so no site was built or deployed"
[ -f "$ROOT/target/web/rustdoc/surface.json" ] \
  || skip_case "target/web/rustdoc was not produced (run: scripts/rust-check --doc), so there was no reference to deploy"
built="$(jq -r '.built_from // empty' "$ROOT/target/web/rustdoc/surface.json")"
[ "$built" = "$(git -C "$ROOT" rev-parse HEAD)" ] \
  || skip_case "target/web/rustdoc was built from ${built:-no recorded commit}, not HEAD (run: scripts/rust-check --doc), so this commit's reference was not deployed"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj489.XXXXXX")"; trap 'rm -rf "$S"' EXIT
git init -q --bare "$S/remote.git"
git clone -q "$ROOT" "$S/work" 2>/dev/null
cd "$S/work" || exit 1
# The site links the repository's branches by name (blob/master/...), and the site's check
# resolves them against the refs git knows, which in a clone are the cloned checkout's branches
# and not its remote-tracking ones (a pull request's checkout has no local master). The clone is
# given this checkout's, so that the check below refuses for the reason asked and no other. Before
# the push: the push then moves origin/master to this commit, which the deploy requires.
git fetch -q "$ROOT" '+refs/remotes/origin/*:refs/remotes/origin/*'
git remote set-url origin "$S/remote.git"
git push -q origin HEAD:refs/heads/master
ln -s "$ROOT/node_modules" node_modules

# ---------------------------------------------------------------- never committed
# Not in the source branch, and ignored where the producer writes it: a commit cannot carry it
# by accident either.
[ -z "$(git ls-files target/web)" ] || { echo "    target/web is tracked:"; git ls-files target/web | head -5; exit 1; }
mkdir -p target/web
ln -s "$ROOT/target/web/rustdoc" target/web/rustdoc
git check-ignore -q target/web/rustdoc || { echo "    target/web/rustdoc is not ignored"; exit 1; }
[ -z "$(git status --porcelain -- target)" ] || { echo "    the produced tree shows up as a change:"; git status --porcelain -- target; exit 1; }

expect_exit 0 scripts/site-build --no-data
expect_grep '^== compose rustdoc at /rustdoc'
git checkout -q -- .

# ---------------------------------------------------------------- refused before publication
# A composed tree that declares another commit than the build: the site's own check, which the
# deploy runs before it pushes (site-deploy without --skip-build) and the Pages workflow runs
# as its `check` step, refuses it (section 13m). Mutated in the build, never in the producer's
# tree this clone borrows.
cp site/public/rustdoc/surface.json "$S/surface.json"
jq '.built_from = "0123456789abcdef0123456789abcdef01234567"' "$S/surface.json" > site/public/rustdoc/surface.json
expect_exit 10 scripts/site-check --no-sync
expect_grep "^FAIL surfaces +surface rustdoc was built from 0123456789abcdef0123456789abcdef01234567, this build is $(git rev-parse HEAD)$"
expect_grep '^FAIL surfaces +/build\.json records surface rustdoc as built from [0-9a-f]{40}, its surface\.json says 0123456789abcdef0123456789abcdef01234567$'
# and that is the whole refusal: nothing else in the build is wrong, so the verdict is the
# stale reference's
others="$(printf '%s\n' "$LAST_OUT" | grep '^FAIL' | grep -v '^FAIL surfaces ' || true)"
[ -z "$others" ] || { echo "    site-check refused this build for more than the stale reference:"; printf '%s\n' "$others" | head -5; exit 1; }
cp "$S/surface.json" site/public/rustdoc/surface.json

# ---------------------------------------------------------------- the deploy
head="$(git rev-parse HEAD)"
expect_exit 0 scripts/site-deploy --skip-build
expect_grep "pushed .* to origin/gh-pages"
git fetch -q origin gh-pages
pub() { git show "origin/gh-pages:$1"; }
pub rustdoc/index.html > "$S/landing.html" 2>/dev/null || { echo "    gh-pages carries no rustdoc/index.html"; exit 1; }
grep -qF "$built" "$S/landing.html" || { echo "    the published landing page does not name $built"; exit 1; }
pub rustdoc/majordomus_cli/index.html 2>/dev/null | grep -qF '<title>majordomus_cli - Rust</title>' \
  || { echo "    gh-pages carries no crate index for majordomus_cli"; exit 1; }
[ "$(pub rustdoc/surface.json | jq -r '.id + " " + .mount + " " + .built_from')" = "rustdoc /rustdoc $built" ] \
  || { echo "    gh-pages carries another declaration at rustdoc/surface.json:"; pub rustdoc/surface.json; exit 1; }
# the COMMIT constant page, the build's own statement of its commit, came with it
pub rustdoc/majordomus_cli/constant.COMMIT.html 2>/dev/null | grep -qF "&quot;$built" \
  || { echo "    the published COMMIT page does not name $built"; exit 1; }
# the site's identity names its own commit and the reference's, side by side
pub build.json | jq -e --arg h "$head" --arg b "$built" \
  '.commit == $h and .surfaces == [{id: "rustdoc", mount: "/rustdoc", built_from: $b}]' >/dev/null \
  || { echo "    the published build.json does not record the reference:"; pub build.json | jq -c '{commit, surfaces}'; exit 1; }
# the whole tree, not its entry points: every file the producer wrote is on the branch
want="$(cd target/web/rustdoc/ && find . -type f | wc -l | tr -d ' ')"
got="$(git ls-tree -r --name-only origin/gh-pages -- rustdoc | wc -l | tr -d ' ')"
[ "$want" = "$got" ] || { echo "    the producer wrote $want file(s) and gh-pages carries $got under rustdoc/"; exit 1; }
git log -1 --format=%B origin/gh-pages | grep -q "source: $head" || { echo "    the gh-pages commit does not name its source"; exit 1; }
exit 0

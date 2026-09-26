# majordomus-covers: none
# claims: rustdoc-composed-from-the-topology
# scripts/site-build composes every published static surface into the site at its mount, read
# from the committed topology with no surface named in the script, and refuses, naming the
# producer, when a published surface's artifact is absent (ADR 0086).
#
# The regression this case exists for is a copy step per surface: the day it is written it
# carries the reference, and the day a surface is added or renamed without it the site is
# published without that surface and nothing says so. So the case builds a clone of this
# checkout without the reference's tree (refused, naming the producer), with it (composed, and
# recorded in /build.json with the commit its producer declared), with a second surface that
# exists only in the clone's topology (composed with no script changed), without that one's
# artifact (refused, naming its producer), and at a mount the site itself fills (refused: one
# path has one owner).
#
# A composition that silently stopped is the other half of that regression, and the site's own
# check is what must catch it before a deploy does: so the case runs scripts/site-check over
# the composed build (clean: the surface is in it, built from the build's commit, and the
# menu's link to it resolves) and over a build made while the topology lacked the surface —
# which site-build carries out without a word — and requires the refusal from three sections:
# the surface is not in the build (13m), the menu's /rustdoc/ link resolves to nothing (13d),
# and link-check (6) finds the site's links into the reference dangling. With the declaration
# itself gone, so that 13m has nothing to ask about, link-check alone still refuses the menu's
# link.
#
# The reference's tree is a producer output, never committed, so the clone borrows this
# checkout's as case 96 does, and it must be this commit's for the site's check to accept it.
# Under CI the suite job produced it from this commit first (.github/actions/rustdoc) and
# installed zola, the site's node_modules and jq, so any of them absent, or a tree of another
# commit, fails the case; anywhere else it is what the person has not run yet
# (scripts/rust-check --doc), and the case says what it did not measure and ends (skip_case).
# The builds are real.
. "$ROOT/test/lib.sh"
command -v zola >/dev/null || skip_case "zola is absent, so no site was built and nothing was composed"
[ -d "$ROOT/node_modules/tailwindcss" ] || skip_case "node_modules is absent, so no site was built and nothing was composed"
command -v jq >/dev/null 2>&1 || skip_case "no jq, so the topology was not read"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj486.XXXXXX")"; trap 'rm -rf "$S"' EXIT
git clone -q "$ROOT" "$S/work" 2>/dev/null
cd "$S/work" || exit 1
# The site links the repository's branches by name (blob/master/...), and the site's link check
# resolves such a link against the refs git knows. A clone knows the branches of the checkout it
# cloned, not that checkout's remote-tracking ones — a pull request's checkout has no local
# master at all — so the clone is given this checkout's, and sees the repository as it does.
git fetch -q "$ROOT" '+refs/remotes/origin/*:refs/remotes/origin/*'
ln -s "$ROOT/node_modules" node_modules
TAB="$(printf '\t')"

# ---------------------------------------------------------------- what the topology composes
# The committed topology publishes the reference, and the one selection every consumer reads
# says so: id, mount, artifact, producer.
composed="$(. "$ROOT/lib/common.sh"; mj_web_composed "$PWD" rows)" || { echo "    the clone's topology cannot be read"; exit 1; }
printf '%s\n' "$composed" | grep -qx "rustdoc${TAB}/rustdoc${TAB}target/web/rustdoc${TAB}scripts/rust-check --doc" \
  || { echo "    the topology does not compose the reference as declared:"; printf '%s\n' "$composed"; exit 1; }

# ---------------------------------------------------------------- the producer has not run
expect_exit 12 scripts/site-build --no-data
expect_grep 'surface rustdoc is published at /rustdoc and its artifact target/web/rustdoc is absent; run its producer: scripts/rust-check --doc'
[ ! -e site/public/index.html ] || { echo "    a refused build still wrote the site"; exit 1; }

# ---------------------------------------------------------------- the producer has run
[ -f "$ROOT/target/web/rustdoc/surface.json" ] \
  || skip_case "target/web/rustdoc was not produced (run: scripts/rust-check --doc), so only the refusal without it was measured"
built="$(jq -r '.built_from // empty' "$ROOT/target/web/rustdoc/surface.json")"
[ "$built" = "$(git rev-parse HEAD)" ] \
  || skip_case "target/web/rustdoc was built from ${built:-no recorded commit}, not HEAD (run: scripts/rust-check --doc), so only the refusal without it was measured"
mkdir -p target/web
ln -s "$ROOT/target/web/rustdoc" target/web/rustdoc
expect_exit 0 scripts/site-build --no-data
expect_grep '^== compose rustdoc at /rustdoc \(from target/web/rustdoc\)$'
expect_grep 'composed: rustdoc$'
expect_file site/public/rustdoc/index.html
expect_file site/public/rustdoc/majordomus_cli/index.html
cmp -s site/public/rustdoc/surface.json target/web/rustdoc/surface.json \
  || { echo "    the composed declaration is not the producer's"; exit 1; }
# the build records what it composed, and the commit the producer declared it was built from
jq -e --arg b "$built" '.surfaces == [{id: "rustdoc", mount: "/rustdoc", built_from: $b}]' site/data/build.json >/dev/null \
  || { echo "    build.json does not record the composed reference:"; jq -c '.surfaces' site/data/build.json; exit 1; }
cmp -s site/data/build.json site/public/build.json || { echo "    /build.json is not the build's record"; exit 1; }

# ---------------------------------------------------------------- the site's check, composed
# The publication as composed is clean under the site's own check: 13m finds the surface in
# the build and built from the commit the build is, the menu's link to it resolves (13d), and
# link-check (6) resolves every site link into it while scanning none of its pages. The menu
# links the mount: that is the link the negative below loses.
base="$(sed -n 's/^base_url = "\(.*\)"/\1/p' site/config.toml)"
grep -qF "href=\"${base%/}/rustdoc/\"" site/public/index.html \
  || { echo "    the built home page's menu does not link ${base%/}/rustdoc/"; exit 1; }
expect_exit 0 scripts/site-check --no-sync
expect_grep "^OK   surfaces +1 composed surface\(s\) \(/rustdoc\) are in the build, each built from ${built:0:12}, the commit this build is$"
expect_grep '^OK   nav +every navigation link on the homepage resolves$'
expect_grep "^OK   link +.* the [0-9]+ page\(s\) under /rustdoc/ are composed surfaces' own and were not scanned"

# ---------------------------------------------------------------- a build that lost the compose
# The regression simulated where it happens: the topology the build reads lacks the surface —
# a declaration dropped, or a composition that stopped reading it — and site-build carries it
# out without a word, because it composes what the topology says. The site's own check, over
# the committed topology, refuses that build from three sections at once.
jq 'del(.surfaces[] | select(.id == "rustdoc"))' docs/generated/web.json > "$S/web-without.json"
cp "$S/web-without.json" docs/generated/web.json
expect_exit 0 scripts/site-build --no-data
expect_no_grep 'compose rustdoc'
[ ! -e site/public/rustdoc ] || { echo "    a build whose topology lacks the surface still composed it"; exit 1; }
git checkout -q -- docs/generated/web.json
expect_exit 10 scripts/site-check --no-sync
expect_grep '^FAIL surfaces +surface rustdoc is not in the build: no /rustdoc/index\.html \(run its producer, scripts/rust-check --doc, then scripts/site-build\)$'
expect_grep '^FAIL nav +navigation/hero link /rustdoc/ does not resolve$'
expect_grep "^FAIL link +internal +[^ ]+: ${base%/}/rustdoc/ resolves to no file under site/public"
# With the declaration itself gone as well, the check's own list of composed surfaces is empty
# and 13m has nothing to ask about; the site's links into the reference are still links, and
# link-check still refuses them.
cp "$S/web-without.json" docs/generated/web.json
expect_exit 10 scripts/ci/link-check
expect_grep "^FAIL internal +[^ ]+: ${base%/}/rustdoc/ resolves to no file under site/public"
git checkout -q -- docs/generated/web.json

# ---------------------------------------------------------------- a surface no script names
# A second published surface that exists only in this clone's topology: its artifact and its
# declaration, and one row of docs/generated/web.json. No script changes.
jq '.surfaces += [{id: "fixture-report", title: "A report only this clone publishes",
                   kind: "static-directory", mount: "/fixture-report",
                   producer: "the fixture producer", artifact: "target/web/fixture-report",
                   index: "index.html", availability: "published-only",
                   category: "report", visibility: "public"}]' \
  docs/generated/web.json > "$S/web.json" && cp "$S/web.json" docs/generated/web.json
mkdir -p target/web/fixture-report
printf '<!doctype html><title>fixture report</title><p>fixture</p>\n' > target/web/fixture-report/index.html
jq -n --arg b "$(git rev-parse HEAD)" \
  '{schema: "web-surface/v1", id: "fixture-report", mount: "/fixture-report", built_from: $b}' \
  > target/web/fixture-report/surface.json
[ "$(git status --porcelain --untracked-files=no | sed 's/^...//')" = docs/generated/web.json ] \
  || { echo "    the fixture changed more than the topology:"; git status --porcelain; exit 1; }
expect_exit 0 scripts/site-build --no-data
expect_grep '^== compose fixture-report at /fixture-report \(from target/web/fixture-report\)$'
expect_grep 'composed: rustdoc fixture-report$'
expect_file site/public/fixture-report/index.html
jq -e '[.surfaces[].id] == ["rustdoc", "fixture-report"]' site/data/build.json >/dev/null \
  || { echo "    build.json does not record both composed surfaces:"; jq -c '.surfaces' site/data/build.json; exit 1; }

# its producer has not run: refused, naming that producer, before anything is built
rm -rf target/web/fixture-report site/public
expect_exit 12 scripts/site-build --no-data
expect_grep 'surface fixture-report is published at /fixture-report and its artifact target/web/fixture-report is absent; run its producer: the fixture producer'
[ ! -e site/public/index.html ] || { echo "    a refused build still wrote the site"; exit 1; }

# ---------------------------------------------------------------- one path, one owner
# The same surface at a mount the site's own pages fill: refused, never overwritten.
mkdir -p target/web/fixture-report
printf '<!doctype html><title>fixture report</title>\n' > target/web/fixture-report/index.html
jq '(.surfaces[] | select(.id == "fixture-report") | .mount) = "/guarantees"' \
  docs/generated/web.json > "$S/web.json" && cp "$S/web.json" docs/generated/web.json
expect_exit 10 scripts/site-build --no-data
expect_grep 'the site itself built site/public/guarantees, the mount of surface fixture-report; one path has one owner'
grep -q 'fixture report' site/public/guarantees/index.html 2>/dev/null \
  && { echo "    the site's own page was overwritten by the surface"; exit 1; }
exit 0

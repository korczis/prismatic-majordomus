# The two halves of the published-site gate that ask the network, held to fixtures.
#
# `scripts/ci/pages-check` answers four questions. Case 104 holds the two that are decidable
# from a clone — is the published commit on master, and has master moved past it — and says of
# the other two: "they ask the network and are not simulated: a case that stood up an HTTP
# server would be testing the fixture."
#
# That reasoning covers the server and not the gate. What these two halves decide is a
# comparison and a message, and both are this repository's: which commit the served identity
# document names against which commit the branch published, and what a reader is told when the
# two differ. The server is a prop; the comparison is the subject. And the two modes they
# catch are precisely the two that leave a green tick, so leaving them untested left the
# loudest part of publication as the least proven part of it:
#
#   PAGES NEVER REBUILT — a push made with GITHUB_TOKEN starts no workflow, so gh-pages can
#   carry the new commit while the live site serves the old one, with the deploy run green.
#
#   GITHUB'S OWN BUILD ERRORED — 2026-09-10 17:35:10Z, gh-pages commit dfd9c989c, "Page build
#   failed.". The push succeeded, the branch was right, the workflow was green, and the site
#   served the previous commit for 27 minutes.
#
# Nothing here reaches the real network: the site is a directory behind test/lib.sh's own
# local server, and GitHub's build API is a stub on PATH. Neither is what is being asserted.
. "$ROOT/test/lib.sh"

GATE="$ROOT/scripts/ci/pages-check"
[ -x "$GATE" ] || { echo "    scripts/ci/pages-check is missing or not executable"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    skip: jq is required"; exit 0; }

# A repository whose gh-pages names a published commit, whose site/config.toml points at a
# directory this case serves, and which carries the publication model the gate reads the
# identity document's name out of.
fixture() {
  local dir="$1" base="$2"
  rm -rf "$dir"; mkdir -p "$dir/scripts/ci" "$dir/.ai/repo/ci" "$dir/site"
  cp "$GATE" "$dir/scripts/ci/pages-check"
  cp "$ROOT/scripts/pages" "$dir/scripts/pages"
  cp -R "$ROOT/lib" "$dir/lib"
  # `scripts/pages` resolves the layer through lib/common.sh, which needs the manifest and the
  # bin directory to exist before it will say anything at all. A fixture missing them makes the
  # gate report "not measured" for reasons that have nothing to do with what is being asserted.
  cp -R "$ROOT/bin" "$dir/bin"
  cp -R "$ROOT/share" "$dir/share"
  cp "$ROOT/.ai/manifest.yaml" "$ROOT/.ai/README.md" "$dir/.ai/"
  cp "$ROOT/.ai/repo/ci/pages.yaml" "$dir/.ai/repo/ci/pages.yaml"
  cp "$ROOT/.ai/repo/ci/gates.yaml" "$dir/.ai/repo/ci/gates.yaml"
  printf 'base_url = "%s"\n' "$base" > "$dir/site/config.toml"
  (
    cd "$dir" || exit 1
    git init -q .
    git config user.email t@e; git config user.name t
    git add -A >/dev/null 2>&1
    git commit -qm "first"
    git branch -M master
    PUB="$(git rev-parse HEAD)"
    # gh-pages is built with plumbing rather than by checking out an orphan branch: the
    # fixture's working tree carries the gate and the library it needs, and switching to an
    # empty branch and back would have git refuse to overwrite them. Only the subject of the
    # deploy commit is read by the gate, so an empty tree is the whole content it needs.
    empty="$(git hash-object -t tree /dev/null)"
    gp="$(git commit-tree "$empty" -m "deploy: site from $PUB (registry deadbeefcafe), 800 routes")"
    git update-ref refs/heads/gh-pages "$gp"
    git remote add origin "$dir"
    git fetch -q origin 'refs/heads/*:refs/remotes/origin/*'
    printf '%s\n' "$PUB" > .published
  )
}

# What the site serves: the identity document the model names, and a page whose footer names a
# commit the way every rendered page does. The two are produced by different steps of one
# deploy, which is why the gate reads both.
serve() {
  local dir="$1" identity_commit="$2" footer_commit="$3"
  rm -rf "$dir"; mkdir -p "$dir"
  jq -n --arg c "$identity_commit" '{schema: 1, commit: $c, dirty: false}' > "$dir/build.json"
  printf '<html><body><a href="https://example.invalid/commit/%s">%s</a></body></html>\n' \
    "$footer_commit" "$footer_commit" > "$dir/index.html"
}

# GitHub's Pages build API, as a stub: the one endpoint the gate reads, answering what this
# case wants it to have answered. The real one is asked by `scripts/pages built`, in one
# place; this replaces the transport and nothing else.
stub_gh() {
  local dir="$1" status="$2" commit="$3" message="$4"
  mkdir -p "$dir"
  cat > "$dir/gh" <<STUB
#!/usr/bin/env bash
# only the pages/builds endpoint is answered; anything else is not this stub's business
case "\$*" in
  *pages/builds*) printf '%s\t%s\t%s\n' "$status" "$commit" "$message"; exit 0 ;;
esac
exit 1
STUB
  chmod +x "$dir/gh"
}

WWW="$T/www"
serve "$WWW" 0000000000000000000000000000000000000000 0000000000000000000000000000000000000000
start_http "$WWW" || { echo "    skip: no python3 and no node to serve a fixture site"; exit 0; }
trap 'stop_http' EXIT

fixture "$T/repo" "$HTTP_BASE"
PUB="$(cat "$T/repo/.published")"
OLD=1111111111111111111111111111111111111111

# The gate must never be run against the real gh-pages of this checkout by accident, so every
# invocation is inside the fixture, with the stub ahead of anything real on PATH.
# MAJORDOMUS_ROOT and MAJORDOMUS_SHARE are exported by this repository's own .envrc and point
# at the checkout a person is sitting in. Inside a fixture they make the tool resolve another
# tree's layer, and what a reader then sees is a gate reporting "not measured" for reasons that
# have nothing to do with the thing under test. A fixture is hermetic or it is not a fixture.
gate() {
  local out="$1"; shift
  ( cd "$T/repo" \
    && PATH="$T/bin:$PATH" env -u MAJORDOMUS_ROOT -u MAJORDOMUS_SHARE \
       ./scripts/ci/pages-check --wait 0 "$@" > "$out" 2>&1 )
}

# ---------------------------------------------------------------- the live site serves it
stub_gh "$T/bin" built "$(cd "$T/repo" && git rev-parse origin/gh-pages)" "no message"
serve "$WWW" "$PUB" "$PUB"
if gate "$T/live-ok.out"; then rc=0; else rc=$?; fi
[ "$rc" = 0 ] || { echo "    the gate refused a site that is serving the published commit (exit $rc):"; cat "$T/live-ok.out"; exit 1; }
grep -q "the live site's build.json names" "$T/live-ok.out" || {
  echo "    the gate did not say what the live site is serving:"; cat "$T/live-ok.out"; exit 1; }

# ---------------------------------------------------------------- PAGES NEVER REBUILT
#
# gh-pages carries the new commit and the site still answers with the old one. This is the
# GITHUB_TOKEN deadlock: a push made with that token starts no workflow, so the branch moves
# and the build never runs. The refusal must name both commits and the remedy, because the
# symptom is indistinguishable from a slow deploy and the difference is what a reader does next.
serve "$WWW" "$OLD" "$OLD"
if gate "$T/stale.out"; then rc=0; else rc=$?; fi
[ "$rc" = 10 ] || { echo "    the gate accepted a site serving a commit the branch did not publish (exit $rc):"; cat "$T/stale.out"; exit 1; }
grep -q "still serves" "$T/stale.out" || grep -q "names ${OLD:0:12}" "$T/stale.out" || {
  echo "    the refusal does not say which commit is being served:"; cat "$T/stale.out"; exit 1; }
grep -q "Pages did not rebuild" "$T/stale.out" || {
  echo "    the refusal does not name the cause:"; cat "$T/stale.out"; exit 1; }
grep -q "GITHUB_TOKEN" "$T/stale.out" || {
  echo "    the refusal does not name the remedy a reader can act on:"; cat "$T/stale.out"; exit 1; }

# ---------------------------------------------------------------- half a deploy
#
# The identity document and the pages are written by different steps. When one step published
# and the other did not, both halves are individually plausible and the site is wrong; only
# comparing them says so.
serve "$WWW" "$PUB" "$OLD"
if gate "$T/half.out"; then rc=0; else rc=$?; fi
[ "$rc" = 10 ] || { echo "    the gate accepted a site whose pages and identity disagree (exit $rc):"; cat "$T/half.out"; exit 1; }
grep -q "footer names" "$T/half.out" || {
  echo "    the refusal does not say the two halves disagree:"; cat "$T/half.out"; exit 1; }

# ---------------------------------------------------------------- published and unreachable
#
# A site that answers nothing is not a site that is fine; the branch says it was published, so
# silence is a finding rather than an absence of one.
stop_http
if gate "$T/dark.out"; then rc=0; else rc=$?; fi
[ "$rc" = 10 ] || { echo "    the gate accepted a site that did not answer at all (exit $rc):"; cat "$T/dark.out"; exit 1; }
grep -q "did not answer" "$T/dark.out" || {
  echo "    the refusal does not say the site is unreachable:"; cat "$T/dark.out"; exit 1; }

start_http "$WWW" || { echo "    the fixture server would not restart"; exit 1; }
fixture "$T/repo" "$HTTP_BASE"
PUB="$(cat "$T/repo/.published")"
serve "$WWW" "$PUB" "$PUB"

# ---------------------------------------------------------------- GITHUB'S OWN BUILD ERRORED
#
# 2026-09-10 17:35:10Z in a fixture. Everything this repository owns is correct — the branch
# carries the right tree, the deploy ran, the site answers — and the publication still did not
# happen, because the second step is GitHub's. The status is read rather than inferred, so the
# gate must refuse on it even while every other line it prints is `ok`.
HEADSHA="$(cd "$T/repo" && git rev-parse origin/gh-pages)"
stub_gh "$T/bin" errored "$HEADSHA" "Page build failed."
if gate "$T/errored.out"; then rc=0; else rc=$?; fi
[ "$rc" = 10 ] || { echo "    the gate accepted a branch GitHub could not build (exit $rc):"; cat "$T/errored.out"; exit 1; }
grep -q "errored: Page build failed." "$T/errored.out" || {
  echo "    the refusal does not carry GitHub's own message:"; cat "$T/errored.out"; exit 1; }

# ---------------------------------------------------------------- and the build that worked
stub_gh "$T/bin" built "$HEADSHA" "no message"
if gate "$T/built.out"; then rc=0; else rc=$?; fi
[ "$rc" = 0 ] || { echo "    the gate refused a branch GitHub built (exit $rc):"; cat "$T/built.out"; exit 1; }
grep -q "is built" "$T/built.out" || {
  echo "    the gate does not say GitHub built the branch:"; cat "$T/built.out"; exit 1; }

# ---------------------------------------------------------------- a build that is not a verdict
#
# GitHub has not built this commit yet, or the token cannot read the endpoint. That is weather,
# not a failure — and it must not read as one, nor as an `ok`. `note` is the third thing.
stub_gh "$T/bin" building "$HEADSHA" "no message"
if gate "$T/building.out"; then rc=0; else rc=$?; fi
[ "$rc" = 0 ] || { echo "    a build still in flight was treated as a failure (exit $rc):"; cat "$T/building.out"; exit 1; }
grep -q "^note  .*Pages build" "$T/building.out" || {
  echo "    a build that is not a verdict was not reported as unmeasured:"; cat "$T/building.out"; exit 1; }

# ---------------------------------------------------------------- one reader of the API
#
# The pages workflow and this gate both need GitHub's build status, and a second `gh api` call
# spelled out somewhere else is a second definition of what "errored" means. `scripts/pages
# built` is the one reader; the gate must go through it.
grep -q 'pages built' "$GATE" || {
  echo "    the gate asks GitHub's Pages build API itself instead of through scripts/pages built"; exit 1; }
if grep -n 'pages/builds' "$GATE" | grep -v '^[0-9]*: *#' | grep -q .; then
  echo "    the gate still carries its own reading of the Pages build API:"
  grep -n 'pages/builds' "$GATE" | grep -v '^[0-9]*: *#'; exit 1
fi

# ---------------------------------------------------------------- and the workflow goes red for it
#
# The gate is what notices afterwards. The run that did the publishing is what can notice at
# the moment it happens, and until 2026-09-11 it could not: the publication probe is
# `continue-on-error` by design, so an errored build left a green tick. It must now fail.
WF="$ROOT/.github/workflows/pages.yml"
grep -q 'scripts/pages built' "$WF" || {
  echo "    the pages workflow never reads GitHub's own build of what it pushed;"
  echo "    an errored build would still end in a green tick"; exit 1; }
grep -q 'pages: read' "$WF" || {
  echo "    the pages workflow reads the Pages build API without asking for pages: read"; exit 1; }

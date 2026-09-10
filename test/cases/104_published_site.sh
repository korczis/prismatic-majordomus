# The published-site gate, against fixture repositories rather than against the network.
#
# `scripts/ci/pages-check` answers four questions, and two of them are decidable offline.
#
# Is the commit gh-pages says it was built from an ancestor of the trunk? That is the half
# that failed in production on 2026-09-09, when an unmerged branch was served for half an
# hour with every tree-level gate green.
#
# And has the trunk moved past it? That is the converse, and it was owned by nobody until
# 2026-09-10, when twenty pull requests landed in an afternoon, the site sat hours behind
# master, and this gate said `ok` every time it ran — because a publication that is on master
# is on master however old it is. A one-way check is this repository's most repeated defect
# class, so the direction that was missing is held to a case here beside the one that was not.
#
# The other two halves ask the network — the live site, and GitHub's own build of gh-pages —
# and are not simulated: a case that stood up an HTTP server would be testing the fixture, and
# one that reached the real site would fail whenever the network did. `--offline` is the seam
# the gate declares for exactly this, and the case asserts that the seam says so rather than
# silently skipping.
#
# Every fixture is a repository this case builds, so the assertions do not depend on the
# checkout's own history.
. "$ROOT/test/lib.sh"

GATE="$ROOT/scripts/ci/pages-check"
[ -x "$GATE" ] || { echo "    scripts/ci/pages-check is missing or not executable"; exit 1; }

# A repository with a trunk, a published commit on it, and a gh-pages branch that says so.
fixture() {
  local dir="$1" published="$2"
  rm -rf "$dir"; mkdir -p "$dir"
  (
    cd "$dir" || exit 1
    git init -q .
    git config user.email t@e; git config user.name t
    git commit -q --allow-empty -m "first"
    git branch -M master
    git commit -q --allow-empty -m "second"
    # the branch that is not on master, for the unmerged case
    git checkout -q -b elsewhere
    git commit -q --allow-empty -m "work nobody merged"
    UNMERGED="$(git rev-parse --short HEAD)"
    git checkout -q master
    MERGED="$(git rev-parse --short HEAD)"
    case "$published" in
      merged)   SHA="$MERGED" ;;
      unmerged) SHA="$UNMERGED" ;;
    esac
    # gh-pages carries the deploy subject the gate reads, and nothing else needs to be real
    git checkout -q --orphan gh-pages
    git rm -rq --cached . 2>/dev/null || true
    git commit -q --allow-empty -m "deploy: site from $SHA (registry deadbeefcafe), 800 routes"
    git checkout -q master
    # the gate reads `$REMOTE/$BRANCH`, so the fixture is its own remote
    git remote add origin "$dir"
    git fetch -q origin 'refs/heads/*:refs/remotes/origin/*'
    mkdir -p scripts/ci
    cp "$GATE" scripts/ci/pages-check
  )
}

# Master moves past the publication, the way a merged pull request moves it.
advance() {
  (
    cd "$1" || exit 1
    git checkout -q master
    : > moved.txt; git add moved.txt
    git commit -q -m "something the site is built from"
    git fetch -q origin 'refs/heads/*:refs/remotes/origin/*'
  )
}

# --- a publication from the trunk is accepted
fixture "$T/ok" merged
if ( cd "$T/ok" && ./scripts/ci/pages-check --offline > "$T/ok.out" 2>&1 ); then rc=0; else rc=$?; fi
[ "$rc" = 0 ] || { echo "    the gate refused a publication that is on master (exit $rc):"; cat "$T/ok.out"; exit 1; }
grep -q "is on master" "$T/ok.out" || { echo "    the gate did not say the commit is on master:"; cat "$T/ok.out"; exit 1; }

# the seam is declared rather than silent: --offline says the live site was not asked
grep -q "not asked" "$T/ok.out" || {
  echo "    --offline skipped the live check without saying so; a check that is silently"
  echo "    absent is indistinguishable from one that passed"; cat "$T/ok.out"; exit 1; }

# --- a publication from a branch that is not on the trunk is refused
#
# This is 2026-09-09 in a fixture. The exit code is the repository's contract for a refusal
# (10), and the message must name the commit, because a refusal nobody can act on is noise.
fixture "$T/bad" unmerged
if ( cd "$T/bad" && ./scripts/ci/pages-check --offline > "$T/bad.out" 2>&1 ); then rc=0; else rc=$?; fi
[ "$rc" = 10 ] || { echo "    the gate accepted an unmerged publication (exit $rc):"; cat "$T/bad.out"; exit 1; }
grep -q "NOT on master" "$T/bad.out" || { echo "    the refusal does not say what is wrong:"; cat "$T/bad.out"; exit 1; }
grep -q "any-ref" "$T/bad.out" || { echo "    the refusal does not name the cause or the remedy:"; cat "$T/bad.out"; exit 1; }

# --- a deploy commit that does not say where it came from is unauditable, and refused
#
# The subject is the only record of which tree was published. A deploy that stopped writing
# it would make every later answer a guess, so the gate refuses to guess (12, missing
# artifact) rather than passing.
fixture "$T/mute" merged
if ( cd "$T/mute" && git checkout -q gh-pages && git commit -q --allow-empty -m "chore: something else entirely" \
      && git checkout -q master && git fetch -q origin 'refs/heads/*:refs/remotes/origin/*' \
      && ./scripts/ci/pages-check --offline > "$T/mute.out" 2>&1 ); then rc=0; else rc=$?; fi
[ "$rc" = 12 ] || { echo "    a deploy commit naming no source was accepted (exit $rc):"; cat "$T/mute.out"; exit 1; }
grep -q "unauditable" "$T/mute.out" || { echo "    the refusal does not say why it cannot proceed:"; cat "$T/mute.out"; exit 1; }

# --- a trunk that has moved past the publication, and the deploy never happened
#
# This is 2026-09-10 in a fixture: gh-pages still names the commit it named an hour ago while
# master carries commits it does not. The window is the whole judgement — the same tree is a
# deploy in flight when it is a minute old and a stale site when it is an hour old — so it is
# asked both ways over one fixture rather than by waiting.
fixture "$T/owed" merged
advance "$T/owed"

# inside the window: owed, young, and not a refusal
if ( cd "$T/owed" && ./scripts/ci/pages-check --offline --owed-after 3600 > "$T/young.out" 2>&1 ); then rc=0; else rc=$?; fi
[ "$rc" = 0 ] || { echo "    the gate refused a publication that is merely in flight (exit $rc):"; cat "$T/young.out"; exit 1; }
grep -q "inside the .* deploy window" "$T/young.out" || {
  echo "    the gate did not say a publication is owed and young:"; cat "$T/young.out"; exit 1; }

# past the window: the site owes a publication, and that is the refusal that was missing
if ( cd "$T/owed" && ./scripts/ci/pages-check --offline --owed-after 0 > "$T/owed.out" 2>&1 ); then rc=0; else rc=$?; fi
[ "$rc" = 10 ] || { echo "    the gate accepted a site the trunk had moved past (exit $rc):"; cat "$T/owed.out"; exit 1; }
grep -q "owes a publication" "$T/owed.out" || {
  echo "    the refusal does not say the publication is owed:"; cat "$T/owed.out"; exit 1; }
grep -q "cancelled by the next push" "$T/owed.out" || {
  echo "    the refusal names no cause a reader could act on:"; cat "$T/owed.out"; exit 1; }

# --- a check that could not be made is never reported as a check that passed
#
# The fixture has no publication model and no scripts/pages, so the trigger paths cannot be
# read. The gate must say so: an absent measurement that prints `ok` is the disease this whole
# file exists to treat.
grep -q "^note  " "$T/owed.out" || {
  echo "    the gate could not read the trigger paths and did not say so:"; cat "$T/owed.out"; exit 1; }

# --- the gate carries no URL of its own
#
# Where the site lives is declared in site/config.toml; a URL written into the gate would be
# a second declaration of it, and the two would drift the first time the site moved.
if grep -nE 'https?://[a-z]' "$GATE" | grep -v '^\s*#' | grep -q .; then
  echo "    the gate contains a literal URL; it must read site/config.toml:"
  grep -nE 'https?://[a-z]' "$GATE" | grep -v '^\s*#' | head -3
  exit 1
fi

# --- and no identity path of its own either
#
# `.ai/repo/ci/pages.yaml` declares where the site states which commit it is; scripts/pages
# reads it there, and so must this.
grep -q 'pages.yaml' "$GATE" || {
  echo "    the gate does not read the identity path from the publication model"; exit 1; }

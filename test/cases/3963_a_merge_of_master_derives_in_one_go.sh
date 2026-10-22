# claim: derived-data-current
# A merge of master is derived and committed in one go (I3963).
#
# On 2026-10-22 a merge of master could be neither derived nor committed: scripts/derive
# refused while MERGE_HEAD existed (I2863), and the pre-commit pages gate refuses a merge
# whose derived data is stale. Every `prs repair --apply` failed at its derive, and the
# landing seat worked around it by hand. The derive now writes the staged merge as the commit
# it will be and derives that in a scratch worktree, so a reader of history sees both parents.
#
# The fixture is a disposable repository with a copy of scripts/derive and stub stages that
# keep its shape: the "executable" projects `git log` (a reader of history, as released_in and
# the changelog are), and the "site generator" records the commit it observes (delivery.json)
# and the hash of its inputs (source.json), which a pages-style pre-commit hook compares.
#
#   1  merging master leaves the derived files in conflict; resolved as ours (the derived
#      merge driver's answer) the merge cannot be committed: the hook calls them stale
#   2  the derive runs on the merge, the derived history lists the commit master added, the
#      observed commit is the branch's HEAD and never the scratch commit, nothing is left
#      behind, and the merge then commits through the hook with both parents
#   3  a merge with an unresolved path is refused before the lock is taken
. "$ROOT/test/lib.sh"

fail() { printf '    %s\n' "$*"; exit 1; }
STUB="$(mktemp -d "${TMPDIR:-/tmp}/mj-3963-stub.XXXXXX")"
trap 'rm -rf "$STUB"' EXIT

mkdir -p scripts lib docs .githooks
cp "$ROOT/scripts/derive" scripts/derive
cp "$ROOT/lib/machine_lock.sh" "$ROOT/lib/rust_bin.sh" lib/
# the executable: `generate --repo R --strict` projects R's history and nothing else
cat > "$STUB/majordomus" <<'EOF'
#!/usr/bin/env bash
set -eu
[ "${1:-}" = generate ] || exit 0
repo="$3"; mkdir -p "$repo/docs/generated"
git -C "$repo" log --format=%s | LC_ALL=C sort > "$repo/docs/generated/history.txt"
EOF
# the site generator: the commit it observes (MJ_DERIVE_OBSERVE when an ancestor, else HEAD,
# as scripts/generate-site-data decides) and the hash of the canonical inputs
cat > scripts/generate-site-data <<'EOF'
#!/usr/bin/env bash
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; mkdir -p "$ROOT/site"
if [ -n "${MJ_DERIVE_OBSERVE:-}" ] && git -C "$ROOT" merge-base --is-ancestor "$MJ_DERIVE_OBSERVE" HEAD; then
  git -C "$ROOT" rev-parse "$MJ_DERIVE_OBSERVE" > "$ROOT/site/delivery.txt"
else git -C "$ROOT" rev-parse HEAD > "$ROOT/site/delivery.txt"; fi
cat "$ROOT"/docs/*.md | git hash-object --stdin > "$ROOT/site/source.txt"
EOF
printf '#!/bin/sh\nexit 0\n' > scripts/rules-package
printf '#!/bin/sh\nexit 0\n' > scripts/gitattributes
# the pages gate: the recorded input hash must be this tree's
cat > .githooks/pre-commit <<'EOF'
#!/bin/sh
[ "$(cat docs/*.md | git hash-object --stdin)" = "$(cat site/source.txt 2>/dev/null)" ] && exit 0
echo "pages: site data is stale" >&2; exit 1
EOF
chmod +x "$STUB/majordomus" scripts/* .githooks/pre-commit
git config core.hooksPath .githooks
export MAJORDOMUS_BIN="$STUB/majordomus"
export MAJORDOMUS_DERIVE_LOCK="$T/machine/.derive.lock"
export MAJORDOMUS_DERIVE_PRIORITY="$T/machine/.derive.priority"
export MAJORDOMUS_DERIVE_TIMINGS=""
printf 'machine/\n' > .gitignore
D="$T/scripts/derive"
derive_commit() { expect_exit 0 "$D" || exit 1; git add -A; git commit -qm "$1"; }

echo seed > docs/seed.md
git add -A && "$D" >/dev/null 2>&1 && git add -A && git commit -qm "base"
master="$(git rev-parse --abbrev-ref HEAD)"
git checkout -q -b feature
echo feature > docs/feature.md && git add docs/feature.md && derive_commit "feature: its own page"
git checkout -q "$master"
echo master > docs/master.md && git add docs/master.md && derive_commit "master: a page the branch has not seen"
git checkout -q feature
left="$(git rev-parse HEAD)"

# ---------------------------------------------------------------- 1. the wall
git merge -q --no-ff --no-commit "$master" >/dev/null 2>&1 || true
[ -e "$(git rev-parse --git-dir)/MERGE_HEAD" ] || fail "the fixture did not leave a merge in progress"
[ -n "$(git diff --name-only --diff-filter=U)" ] || fail "the derived files did not conflict"
git checkout -q --ours -- docs/generated site && git add docs/generated site
expect_exit 1 git commit -q --no-edit
expect_grep 'pages: site data is stale'

# ---------------------------------------------------------------- 2. derived and committed in one go
expect_exit 0 "$D"
expect_grep 'a merge in progress: deriving the commit it will be'
expect_grep 'the merge is derived and staged'
grep -qx 'master: a page the branch has not seen' docs/generated/history.txt \
  || fail "the derived history does not list the commit master added: $(cat docs/generated/history.txt)"
grep -qx 'feature: its own page' docs/generated/history.txt || fail "the branch's own history was lost"
[ "$(cat site/delivery.txt)" = "$left" ] \
  || fail "delivery observed $(cat site/delivery.txt), not the branch's HEAD $left"
[ -z "$(git diff --name-only)" ] || fail "the derive left unstaged changes: $(git diff --name-only)"
[ "$(git worktree list | wc -l | tr -d ' ')" = 1 ] || fail "a scratch worktree was left behind: $(git worktree list)"
! git for-each-ref --format='%(refname)' | grep -qv -e "refs/heads/$master" -e refs/heads/feature \
  || fail "the derive left a ref: $(git for-each-ref --format='%(refname)')"
[ ! -e "$MAJORDOMUS_DERIVE_LOCK" ] || fail 'the derive kept the machine lock'
expect_exit 0 git commit -q --no-edit
[ "$(git rev-list --parents -n 1 HEAD | wc -w | tr -d ' ')" = 3 ] || fail "the commit is not a merge of two parents"
git merge-base --is-ancestor "$master" HEAD || fail "master is not in the branch after the merge"

# ---------------------------------------------------------------- 3. an unresolved merge is refused
git checkout -q "$master"
echo again > docs/master.md && git add docs/master.md && derive_commit "master: changed again"
git checkout -q feature
git merge -q --no-ff --no-commit "$master" >/dev/null 2>&1 || true
[ -n "$(git diff --name-only --diff-filter=U)" ] || fail "the second merge did not conflict"
expect_exit 10 "$D"
expect_grep 'refused before anything ran: the merge still has unresolved paths'
[ ! -e "$MAJORDOMUS_DERIVE_LOCK" ] || fail 'a refused derive took the machine lock'
git merge --abort
echo "    a merge of master is derived as the commit it will be and commits through the gate in one go"

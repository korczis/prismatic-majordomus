# claim: derived-data-current
# A staged merge derived once commits through the real pages gate in one go (I4513).
#
# I3963 made scripts/derive derive a merge in progress as the commit it will be, with both
# parents as history, and case 3963 proved it against a stub hook. The real pre-commit gate,
# `scripts/pages current`, judged the registry half (`majordomus generate --check`) at HEAD, the
# merge's first parent, so that correct output read as stale and the commit was refused: three
# times on 2026-10-30 (#1056, #1083, #2688) and on 2026-11-07 for #2599, where master's ADRs
# were missing from the changelog's history. The gate now judges a staged merge on the same
# two-parent commit.
#
# The fixture is a disposable repository with copies of scripts/derive, scripts/pages and lib/,
# and stub stages that keep their shape: the executable projects `git log` (a reader of history,
# as the changelog is) and checks that projection with `generate --check`; the site generator
# records the hash of its inputs, which `pages current` compares. The hook is the repository's
# own line, `scripts/pages current`, and runs under `git commit`, which sets GIT_INDEX_FILE.
#
#   1  outside a merge the gate passes a derived commit and refuses an underived one
#   2  a merge of master, derived once, commits with both parents; the committed derived files
#      are what a derive of the committed merge writes; no scratch worktree or ref is left
#   3  a staged merge whose registry half is stale (HEAD's history) is still refused
. "$ROOT/test/lib.sh"

fail() { printf '    %s\n' "$*"; exit 1; }
STUB="$(mktemp -d "${TMPDIR:-/tmp}/mj-4513-stub.XXXXXX")"
trap 'rm -rf "$STUB"' EXIT

mkdir -p scripts lib bin docs .githooks .ai/repo/ci site/data/generated
cp "$ROOT/scripts/derive" "$ROOT/scripts/pages" scripts/
cp "$ROOT"/lib/*.sh lib/
cp "$ROOT/.ai/repo/ci/pages.yaml" "$ROOT/.ai/repo/ci/gates.yaml" .ai/repo/ci/
# the history projection both stubs agree on: every non-merge subject of the repository's HEAD
cat > "$STUB/history" <<'EOF'
#!/usr/bin/env bash
git -C "$1" log --no-merges --format=%s | LC_ALL=C sort
EOF
# the executable scripts/derive runs: `generate --repo R --strict` writes R's history
cat > "$STUB/majordomus" <<EOF
#!/usr/bin/env bash
set -eu
[ "\${1:-}" = generate ] || exit 0
repo="\$3"; mkdir -p "\$repo/docs/generated"
"$STUB/history" "\$repo" > "\$repo/docs/generated/history.txt"
EOF
# the executable scripts/pages runs: `generate [--repo R] --check` refuses a stale projection.
# It parses as the real one does: --repo is an option of `generate`, and before the subcommand
# it is refused ("unexpected argument '--repo'"), which is how the first version of this gate
# failed on a real merge on 2026-11-08 while this stub accepted it.
cat > bin/majordomus-cli <<EOF
#!/usr/bin/env bash
set -eu
repo="\$(git rev-parse --show-toplevel)"
[ "\${1:-}" = generate ] || { echo "error: unexpected argument '\${1:-}' found" >&2; exit 2; }
shift; check=""
while [ \$# -gt 0 ]; do
  case "\$1" in --repo) repo="\$2"; shift 2 ;; --check) check=1; shift ;; *) shift ;; esac
done
[ -n "\$check" ] || exit 0
"$STUB/history" "\$repo" | cmp -s - "\$repo/docs/generated/history.txt" && exit 0
echo "generate --check: docs/generated/history.txt is stale for \$repo" >&2; exit 10
EOF
# the site generator: the hash of the canonical inputs, written or (--fingerprint) printed
cat > scripts/generate-site-data <<'EOF'
#!/usr/bin/env bash
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
h="$(cat "$ROOT"/docs/*.md | git hash-object --stdin)"
[ "${1:-}" = --fingerprint ] && { printf '%s\n' "$h"; exit 0; }
mkdir -p "$ROOT/site/data/generated"
printf '{"source_hash":"%s"}\n' "$h" > "$ROOT/site/data/generated/source.json"
EOF
printf '#!/bin/sh\nexit 0\n' > scripts/rules-package
printf '#!/bin/sh\nexit 0\n' > scripts/gitattributes
printf '#!/bin/sh\nscripts/pages current || exit $?\n' > .githooks/pre-commit
chmod +x "$STUB"/* bin/majordomus-cli scripts/* .githooks/pre-commit
git config core.hooksPath .githooks
export MAJORDOMUS_BIN="$STUB/majordomus"
export MAJORDOMUS_DERIVE_LOCK="$T/machine/.derive.lock"
export MAJORDOMUS_DERIVE_PRIORITY="$T/machine/.derive.priority"
export MAJORDOMUS_DERIVE_TIMINGS=""
printf 'machine/\n.ai/local/\n' > .gitignore
D="$T/scripts/derive"
derive_commit() { expect_exit 0 "$D" || exit 1; git add -A; expect_exit 0 git commit -qm "$1" || exit 1; }

# ---------------------------------------------------------------- 1. outside a merge
echo seed > docs/seed.md
git add -A && "$D" >/dev/null 2>&1 && git add -A
expect_exit 0 git commit -qm "base"
master="$(git rev-parse --abbrev-ref HEAD)"
git checkout -q -b feature
echo feature > docs/feature.md && git add docs/feature.md && derive_commit "feature: its own page"
echo more > docs/more.md && git add docs/more.md
expect_exit 1 git commit -qm "feature: an underived page"
expect_grep 'site/data/generated is stale'
git rm -q --cached docs/more.md && rm docs/more.md
git checkout -q "$master"
echo master > docs/master.md && git add docs/master.md && derive_commit "master: a page the branch has not seen"
git checkout -q feature

# ---------------------------------------------------------------- 2. derived once, committed once
git merge -q --no-ff --no-commit "$master" >/dev/null 2>&1 || true
[ -e "$(git rev-parse --git-dir)/MERGE_HEAD" ] || fail "the fixture did not leave a merge in progress"
git checkout -q --ours -- docs/generated site 2>/dev/null; git add -A
expect_exit 0 "$D"
expect_grep 'the merge is derived and staged'
grep -qx 'master: a page the branch has not seen' docs/generated/history.txt \
  || fail "the derived history does not list master's commit: $(cat docs/generated/history.txt)"
expect_exit 0 git commit -q --no-edit
expect_grep 'judged as the merge commit, both parents as history'
[ "$(git rev-list --parents -n 1 HEAD | wc -w | tr -d ' ')" = 3 ] || fail "the commit is not a merge of two parents"
[ -z "$(git status --porcelain)" ] || fail "the commit left the tree dirty: $(git status --porcelain)"
"$D" >/dev/null 2>&1 || fail "a derive of the committed merge failed"
[ -z "$(git status --porcelain)" ] \
  || fail "the committed derived files differ from a derive of the committed merge: $(git status --porcelain)"
[ "$(git worktree list | wc -l | tr -d ' ')" = 1 ] || fail "a scratch worktree was left behind: $(git worktree list)"
! git for-each-ref --format='%(refname)' | grep -qv -e "refs/heads/$master" -e refs/heads/feature \
  || fail "the gate left a ref: $(git for-each-ref --format='%(refname)')"

# ---------------------------------------------------------------- 3. a stale staged merge is refused
git checkout -q "$master"
echo again > docs/again.md && git add docs/again.md && derive_commit "master: another page"
git checkout -q feature
git merge -q --no-ff --no-commit "$master" >/dev/null 2>&1 || true
git checkout -q --ours -- docs/generated site 2>/dev/null; git add -A
"$D" >/dev/null 2>&1 || fail "the derive of the second merge failed"
"$STUB/history" . > docs/generated/history.txt && git add docs/generated/history.txt
expect_exit 1 git commit -q --no-edit
expect_grep 'history.txt is stale'
[ "$(git worktree list | wc -l | tr -d ' ')" = 1 ] || fail "a refused commit left a scratch worktree: $(git worktree list)"
git merge --abort
echo "    a staged merge derived once commits through the pages gate; a stale one is still refused"

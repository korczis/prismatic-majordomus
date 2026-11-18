# majordomus-covers: start finish
# A completed finish over a verified tree that no commit holds says so in its first line
# (I4628). On 2026-11-17 a pre-commit hook refused a commit, the session ran finish, and
# finish printed `finish: <task> completed`: the line an agent reads said finished work that
# was in no commit. Under the shipped policy finish still exits 0 — it reports, it never
# commits — but the verdict line says NOT COMMITTED, names both trees and the paths, and
# task.finished records `committed: false`; a committed task records `committed: true`.
. "$ROOT/test/lib.sh"

R="$T/repo"
mkdir -p "$R/lib"
cd "$R" || exit 1
git init -q .
git config user.email t@example.com
git config user.name t
"$MJ" init >/dev/null
"$MJ" update >/dev/null
echo a > lib/a
git add -A && git commit -qm base

printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' > "$T/note.md"
ledger=.ai/local/state/ledger.jsonl
last_finished() { grep '"event":"task.finished"' "$ledger" | tail -n 1; }

# --- a staged change whose commit a hook refused
expect_exit 0 "$MJ" start "staged, refused by a hook" --scope lib
echo staged >> lib/a
git add lib/a
mkdir -p "$T/hooks"
printf '#!/bin/sh\necho "pages: refused" >&2\nexit 1\n' > "$T/hooks/pre-commit"
chmod +x "$T/hooks/pre-commit"
if git -c core.hooksPath="$T/hooks" commit -qm "refused" 2>/dev/null; then
  echo "    the refusing hook let the commit through"; exit 1
fi
head_tree="$(git rev-parse 'HEAD^{tree}' | cut -c1-12)"
expect_exit 0 "$MJ" finish --outcome completed --verify-command "test -f lib/a" --note "$T/note.md"
first="$(printf '%s\n' "$LAST_OUT" | grep '^finish: ')"
printf '%s\n' "$first" | grep -q "^finish: t-.* completed, NOT COMMITTED: the verified tree [0-9a-f]\{12\} is not HEAD's $head_tree (no commit since the task started)$" \
  || { echo "    finish's verdict line does not say NOT COMMITTED with both trees: $first"; exit 1; }
expect_grep 'uncommitted  lib/a'
expect_grep 'next: git status --porcelain'
last_finished | grep -q '"committed":false' \
  || { echo "    task.finished does not record committed: false: $(last_finished)"; exit 1; }
grep -q '^outcome: completed' .ai/local/state/current.yaml \
  || { echo "    the outcome is not recorded: finish reports, it does not refuse"; exit 1; }
git reset -q --hard

# --- an unstaged change is reported the same way: the verified tree is the working tree
expect_exit 0 "$MJ" start "unstaged" --scope lib
echo unstaged >> lib/a
expect_exit 0 "$MJ" finish --outcome completed --verify-command "test -f lib/a" --note "$T/note.md"
expect_grep '^finish: t-.* completed, NOT COMMITTED'
expect_grep 'uncommitted  lib/a'
last_finished | grep -q '"committed":false' \
  || { echo "    an unstaged change was not recorded committed: false"; exit 1; }

# --- committed before finish: the bare verdict, and committed: true
git commit -qam "the unstaged change"
expect_exit 0 "$MJ" start "committed" --scope lib
echo committed >> lib/a
git commit -qam "committed work"
expect_exit 0 "$MJ" finish --outcome completed --verify-command "test -f lib/a" --note "$T/note.md"
expect_grep '^finish: t-[0-9a-f-]* completed$'
expect_no_grep 'NOT COMMITTED'
last_finished | grep -q '"committed":true' \
  || { echo "    a committed task was not recorded committed: true: $(last_finished)"; exit 1; }

# --- an outcome other than completed claims nothing about a commit
expect_exit 0 "$MJ" start "partial" --scope lib
echo partial >> lib/a
expect_exit 0 "$MJ" finish --outcome partial --note "$T/note.md"
expect_no_grep 'NOT COMMITTED'
last_finished | grep -q '"committed"' \
  && { echo "    a partial finish recorded a commit verdict: $(last_finished)"; exit 1; }
exit 0

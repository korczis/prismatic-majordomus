. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib/auth lib/other && echo x > lib/auth/a.txt && echo y > lib/other/b.txt && git add . && git commit -qm base
expect_exit 12 "$MJ" check
expect_grep 'no active task'
expect_exit 2 "$MJ" start "no scope"
expect_exit 2 "$MJ" start "escape" --scope ../x
expect_exit 12 "$MJ" start "bad profile" --scope lib/auth --profile nosuch
expect_exit 0 "$MJ" start "fix auth" --scope lib/auth/,./lib/auth/sub --profile debugging --owner alice
expect_grep 'started t-[0-9]+-[0-9a-f]{4}  profile=debugging  scope=lib/auth,lib/auth/sub'
expect_grep '^head: '"$(git rev-parse HEAD)"'$' .majordomus/state/current.yaml
expect_grep '^branch: '"$(git branch --show-current)"'$' .majordomus/state/current.yaml
expect_grep '^  - lib/auth$' .majordomus/state/current.yaml
expect_grep '"event":"task.started"' .majordomus/state/ledger.jsonl
# repeated --scope accumulates; spaces after commas are tolerated; a hand-edited trailing slash still contains
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover --close >/dev/null
expect_exit 0 "$MJ" start "multi" --scope lib/auth --scope="lib/other, ./lib/auth/sub/"
expect_grep 'scope=lib/auth,lib/other,lib/auth/sub$'
sed -i.bak 's#^  - lib/auth$#  - lib/auth/#' .majordomus/state/current.yaml; rm -f .majordomus/state/current.yaml.bak
echo z >> lib/auth/a.txt
expect_exit 0 "$MJ" check
expect_grep 'OK +scope'
git checkout -q -- lib/auth/a.txt
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover --close >/dev/null
expect_exit 0 "$MJ" start "fix auth" --scope lib/auth/,./lib/auth/sub --profile debugging --owner alice
# one active task per checkout
expect_exit 15 "$MJ" start "second" --scope lib/other
expect_grep 'is active'
# healthy check
expect_exit 0 "$MJ" check
expect_grep 'OK +state .* exact'
expect_grep 'OK +scope'
expect_grep 'OK +blockers'
# --explain merges task, profile, policy
expect_exit 0 "$MJ" check --explain
expect_grep '^# profile debugging'
expect_grep '^  effort=high'
# in-scope edits pass; committed too (advanced)
echo z >> lib/auth/a.txt && git commit -qam inscope
expect_exit 0 "$MJ" check
expect_grep 'OK +state .* advanced'
# out-of-scope edit fails, names the file
echo z >> lib/other/b.txt
expect_exit 10 "$MJ" check
expect_grep 'FAIL scope +lib/other/b.txt — outside claimed scope'
git checkout -q -- lib/other/b.txt
# state files and projections are always allowed
echo "## note" >> .majordomus/state/decisions.md; echo "" >> CLAUDE.md
expect_exit 0 "$MJ" check
git checkout -q -- CLAUDE.md
# unresolved question blocks
id=$(sed -n 's/^id: //p' .majordomus/state/current.yaml)
printf -- '- [unresolved] %s — refresh window? (2026-09-03)\n' "$id" >> .majordomus/state/open-questions.md
expect_exit 10 "$MJ" check
expect_grep 'FAIL blockers .* refresh window'
sed -i.bak "s/\[unresolved\] $id/[resolved 2026-09-03] $id/" .majordomus/state/open-questions.md; rm -f .majordomus/state/open-questions.md.bak
expect_exit 0 "$MJ" check
# checkpoint updates the record and the ledger
old=$(sed -n 's/^checkpoint_at: //p' .majordomus/state/current.yaml); sleep 1
expect_exit 0 "$MJ" check --checkpoint
[ "$(sed -n 's/^checkpoint_at: //p' .majordomus/state/current.yaml)" != "$old" ]
expect_grep '"event":"task.checkpoint"' .majordomus/state/ledger.jsonl
# stale checkpoint warns but does not fail
sed -i.bak 's/^checkpoint_at: .*/checkpoint_at: 2020-01-01T00:00:00Z/' .majordomus/state/current.yaml; rm -f .majordomus/state/current.yaml.bak
expect_exit 0 "$MJ" check
expect_grep 'WARN checkpoint'
# different branch -> different_context
git checkout -qb elsewhere
expect_exit 10 "$MJ" check
expect_grep 'FAIL state .* recorded on branch'
git checkout -q -
# overlap with another worktree's active task is reported at start
wt="$T-wt-bob"; rm -rf "$wt"
git worktree add -q "$wt" -b bob
( cd "$wt" && "$MJ" start "bob task" --scope lib/auth/sub --owner bob >/dev/null )   # .majordomus/ is tracked, so it is already there
expect_exit 0 "$MJ" check --overlap
expect_grep 'INFO overlap +.*wt-bob — claims lib/auth/sub — contained by your lib/auth'
git worktree remove --force "$wt"

# A task record names the checkout it belongs to. Local state is never tracked, but a record
# can still arrive in another checkout — a copied working directory, a synced folder, a tool
# that moved it — and that checkout must not be held to a scope it never claimed. "One
# active task per checkout" is checked at start, and this proves the same fact is
# represented in the record and honoured by every reader.
. "$ROOT/test/lib.sh"
# the tool records git's own toplevel, which on macOS resolves /var to /private/var
HERE="$(git rev-parse --show-toplevel)"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib other && echo a > lib/a && echo o > other/o && git add . && git commit -qm base

# start records which checkout the task belongs to, computed, not authored
"$MJ" start "local task" --scope lib >/dev/null
expect_grep "^worktree: $HERE\$" .ai/local/state/current.yaml
id=$(sed -n 's/^id: //p' .ai/local/state/current.yaml)

# in this checkout the task is enforced: an out-of-scope file fails
echo x >> other/o
expect_exit 10 "$MJ" check
expect_grep 'FAIL scope +other/o'
git checkout -q -- other/o

# --- a second worktree, into which this checkout's record is copied ---------------------
wt="$T-wt2"; rm -rf "$wt"
git worktree add -q --detach "$wt"
( cd "$wt" && git checkout -q -B second )
WT="$(git -C "$wt" rev-parse --show-toplevel)"
[ ! -e "$wt/.ai/local" ] || { echo "    a fresh worktree carried local state"; exit 1; }
mkdir -p "$wt/.ai/local/state" && cp .ai/local/state/current.yaml "$wt/.ai/local/state/"

# the record is there, it names another checkout, and nothing in it is enforced here
expect_grep "^worktree: $HERE\$" "$wt/.ai/local/state/current.yaml"
echo y >> "$wt/other/o"
expect_exit 0 "$MJ" --repo "$wt" check
expect_grep "INFO task +$id — belongs to $HERE, not this checkout"
expect_no_grep 'FAIL scope'
# the pre-push gate passes there, which is the whole point
expect_exit 0 "$MJ" --repo "$wt" finish --check
expect_grep 'belongs to'
# watch reports it rather than treating it as this checkout's drift
expect_exit 0 "$MJ" --repo "$wt" watch
expect_grep "INFO state +$id — task belongs to $HERE"
# and finish refuses to write to another checkout's record
expect_exit 15 "$MJ" --repo "$wt" finish --outcome completed --verify-command true
expect_grep 'finish it there'
expect_grep '^outcome: active$' .ai/local/state/current.yaml

# the second worktree can start its own task despite the foreign active record; the
# replacement is local to it and touches the other checkout's record not at all
expect_exit 0 "$MJ" --repo "$wt" start "second task" --scope other --owner bob
expect_grep 'WARN task .* belongs to .* replacing it in this working copy only'
expect_grep "^worktree: $HERE\$" .ai/local/state/current.yaml
expect_grep "^worktree: $WT\$" "$wt/.ai/local/state/current.yaml"
expect_exit 0 "$MJ" --repo "$wt" check
expect_grep 'OK +scope'
echo z >> "$wt/lib/a"
expect_exit 10 "$MJ" --repo "$wt" check
expect_grep 'FAIL scope +lib/a'

# the first checkout still enforces its own record, unchanged by any of this
expect_exit 0 "$MJ" check
expect_grep 'OK +scope'
git worktree remove --force "$wt"

# a record written before the field existed is treated as local, so an upgrade is not a wall
sed -i.bak "/^worktree: /d" .ai/local/state/current.yaml; rm -f .ai/local/state/current.yaml.bak
expect_no_grep '^worktree: ' .ai/local/state/current.yaml
echo x >> other/o
expect_exit 10 "$MJ" check
expect_grep 'FAIL scope +other/o'

# A session opens once per worktree, reports itself, and refuses to be opened twice.
#
# The session is the seventh durable record and the only one that is not task-shaped. This
# case covers the open half: identity computed from git, absence answered as absence, a
# second open refused rather than silently replacing the first, and a record from another
# checkout reported rather than obeyed.
. "$ROOT/test/lib.sh"

"$MJ" init >/dev/null
# init adds the local-state ignore line; commit it so the task that follows starts from a clean tree
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true

# --- absence is an answer, not a failure
expect_exit 0 "$MJ" session status
expect_grep 'No open session'

# --- opening records identity computed from git, not authored
expect_exit 0 "$MJ" session start --owner tester --worker "some-provider/some-model"
expect_grep 'session s-[0-9]{14}-[0-9a-f]{4} opened'
S=".ai/local/state/session-current.yaml"
[ -f "$S" ] || { echo "    no open session record written"; exit 1; }

head_now="$(git rev-parse HEAD)"
expect_grep "^start_head: $head_now$" "$S"
expect_grep "^branch: $(git symbolic-ref --short HEAD)$" "$S"
expect_grep "^worktree: $(pwd -P)$" "$S"
expect_grep '^owner: "tester"$' "$S"
expect_grep '^worker: "some-provider/some-model"$' "$S"

# --- the open record is private to the worker who opened it
[ "$(file_mode "$S")" = 600 ] || { echo "    open session record is $(file_mode "$S"), expected 600"; exit 1; }

# --- an absent worker stays absent; nothing plausible is invented for it
rm -f "$S"
expect_exit 0 "$MJ" session start --owner tester
expect_no_grep '^worker:' "$S"

# --- status reports what was recorded, with the divergence label of the opening commit
expect_exit 0 "$MJ" session status
expect_grep 'Start head: [0-9a-f]{7} \(exact\)'
expect_grep 'Owner: +tester'
expect_no_grep 'Worker:'

# --- a second open is refused, and names the command that ends the first
expect_exit 15 "$MJ" session start
expect_grep 'is open here since'
expect_grep 'session close'

# --- the ledger recorded the open, and only the open
grep -q '"event":"session.started"' .ai/local/state/ledger.jsonl || { echo "    no session.started event"; exit 1; }

# --- an open record belonging to another checkout is reported, never obeyed
sed 's#^worktree: .*#worktree: /somewhere/else#' "$S" > "$S.foreign" && mv "$S.foreign" "$S"
expect_exit 0 "$MJ" session status
expect_grep 'belongs to /somewhere/else'
# and it does not block this checkout from opening its own
expect_exit 0 "$MJ" session start --owner tester
expect_grep 'replacing it in this working copy only'

# --- a record that does not parse fails loudly rather than being treated as absent:
#     silently reading a corrupt record as "no session" would let a second open overwrite it
printf 'session_id: [x\n\tbad: value\n' > "$S"
expect_exit 10 "$MJ" session status
expect_grep 'does not parse'

# --- read-only means read-only: status must not write, even on the corrupt record
before="$(cksum < "$S")"
"$MJ" session status >/dev/null 2>&1 || true
[ "$(cksum < "$S")" = "$before" ] || { echo "    session status modified the record it read"; exit 1; }

# --- JSON output is machine-readable in both the absent and the open case
rm -f "$S"
expect_exit 0 "$MJ" --json session status
expect_grep '"open":null'
"$MJ" session start --owner tester >/dev/null
expect_exit 0 "$MJ" --json session status
expect_grep '"session_id":"s-'
expect_grep '"foreign":false'

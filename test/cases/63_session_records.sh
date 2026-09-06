# majordomus-covers: session
# A closed episode is a shared object of the layer: written into the tracked sessions
# section against a contract that admits what the repository can prove and nothing else,
# discovered by one source class, and refused by name when it carries what it may not.
#
# The two refusals this case exists for:
#
#   no conversation. The contract has no field for one, so a record that grew a key is an
#   error rather than a value carried through; project.never-store-transcripts enforced by
#   a schema instead of by a habit.
#
#   no fact about the machine. An absolute path is true of one disk and is disclosure
#   without purpose in a public repository, so a record that names one is refused.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
"$MJ" update >/dev/null
mkdir -p lib docs && echo a > lib/a && echo d > docs/d
git add -A >/dev/null; git commit -qm base

# ---------------------------------------------------------------- a closed episode
expect_exit 0 "$MJ" session start --worker "test/worker"
printf 'Wired the thing and proved it.\n' | "$MJ" session close > closed.txt
rec="$(cat closed.txt)"
[ -n "$rec" ] || { echo "    close printed no path"; exit 1; }
case "$rec" in
  .ai/repo/sessions/*) ;;
  *) echo "    the record went to $rec, not the layer's sessions section"; exit 1 ;;
esac
[ -f "$rec" ] || { echo "    $rec does not exist"; exit 1; }

# every field the contract requires, and the body the worker wrote
grep -q '^schema: session/v1$' "$rec"
grep -q '^kind: session$' "$rec"
grep -q '^session_id: ' "$rec"
grep -q '^started_at: ' "$rec"
grep -q '^closed_at: ' "$rec"
grep -q '^outcome: closed$' "$rec"
grep -q '^worker: "test/worker"$' "$rec"
grep -q 'Wired the thing and proved it' "$rec"

# derived, not authored: the branch and the heads come from git
grep -q "^branch: $(git rev-parse --abbrev-ref HEAD)$" "$rec"
grep -q "^head: $(git rev-parse HEAD)$" "$rec"

# no fact about this machine: the worktree is named by a hash, the repository by its remote
grep -q '^worktree: ' "$rec" && { echo "    the record names an absolute worktree path"; exit 1; }
grep -q '^worktree_id: [0-9a-f]' "$rec"
grep -qE '^repository_id: (local:[0-9a-f]+|[^/].*)$' "$rec" || { echo "    repository_id is a path: $(grep '^repository_id' "$rec")"; exit 1; }

# ---------------------------------------------------------------- discovered, not registered
git add -A >/dev/null; git commit -qm "the session record" >/dev/null
"$MJ" session list | grep -q "$(sed -n 's/^session_id: //p' "$rec")"
"$MJ" knowledge nodes --kind session | grep -q "session:$(sed -n 's/^session_id: //p' "$rec")"
"$MJ" doctor 2>&1 | grep -q 'OK   session' || { echo "    doctor did not report the record"; exit 1; }

# ---------------------------------------------------------------- every refusal, by name
cp "$rec" keep.md
probe() {   # reason mutation...
  local want="$1"; shift
  "$@"
  git add -A >/dev/null
  "$MJ" doctor 2>&1 | grep -qE "$want" || { echo "    doctor did not refuse: $want"; exit 1; }
  cp keep.md "$rec"; git add -A >/dev/null
}
probe 'key\(s\) the contract does not have: transcript' \
  sed -i.bak 's/^outcome: closed$/outcome: closed\ntranscript: "the user said hello"/' "$rec"
probe 'carries an absolute path' \
  sed -i.bak 's|^worktree_id: .*|worktree: /Users/somebody/dev/thing|' "$rec"
probe 'is neither closed nor interrupted' \
  sed -i.bak 's/^outcome: closed$/outcome: abandoned/' "$rec"
probe 'this executable reads session/v1' \
  sed -i.bak 's|^schema: session/v1$|schema: session/v2|' "$rec"
probe 'lacks session_id' \
  sed -i.bak 's/^session_id: /session_id_was: /' "$rec"
rm -f "$rec".bak

# two records, one identity
cp "$rec" .ai/repo/sessions/a-second-claim.md
git add -A >/dev/null
"$MJ" doctor 2>&1 | grep -q 'two records claim this identity' || { echo "    a duplicate identity was not refused"; exit 1; }
rm -f .ai/repo/sessions/a-second-claim.md; git add -A >/dev/null

# ---------------------------------------------------------------- the section's own contract
# The README declares itself a context document, so it is neither a record nor a finding.
[ -f .ai/repo/sessions/README.md ] || { echo "    the section carries no contract"; exit 1; }
"$MJ" knowledge nodes --kind session | grep -q 'README' \
  && { echo "    the section's contract was read as a record"; exit 1; }
"$MJ" doctor 2>&1 | grep -q '1 record(s)' || { echo "    doctor did not count exactly the record"; exit 1; }

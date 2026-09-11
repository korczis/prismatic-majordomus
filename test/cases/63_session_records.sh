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

# ---------------------------------------------------------------- the body is never empty
# project.episode-record-carries-its-episode. The lifecycle closes an episode with
# `< /dev/null`, so a record whose body is optional is a record whose body is always empty:
# every automatically closed record in this repository had a full front matter and nothing
# under it, including one that listed ninety-four commits.
echo "one" > lib/one; git add -A >/dev/null; git commit -qm "a commit inside the episode" >/dev/null

expect_exit 0 "$MJ" session start --worker "test/unauthored"
echo "two" > lib/two; git add -A >/dev/null; git commit -qm "a second commit inside the episode" >/dev/null
"$MJ" session close < /dev/null > closed2.txt
rec2="$(cat closed2.txt)"
[ -f "$rec2" ] || { echo "    the unauthored close wrote no record"; exit 1; }

body="$(awk 'c>=2{print} /^---$/{c++}' "$rec2" | sed '/^$/d')"
[ -n "$body" ] || { echo "    a close with no authored summary left the body empty"; exit 1; }

# every section the policy declares is present, by name
for want in "What Happened" "Progress Notes" "Decisions" "Open Questions" "Outcome"; do
  printf '%s\n' "$body" | grep -qF "# $want" \
    || { echo "    the composed body has no '$want' section"; exit 1; }
done

# the commits are named with their subjects, not only hashed in the front matter
printf '%s\n' "$body" | grep -qF "a second commit inside the episode" \
  || { echo "    the body does not name the commit the episode made"; exit 1; }

# scoped to the episode, not to the task: the commit made before `session start` is in the
# task's range and not in the episode's, and must not be counted here.
printf '%s\n' "$body" | grep -qF "a commit inside the episode" \
  && { echo "    the body counted a commit made before the episode opened"; exit 1; }
printf '%s\n' "$body" | grep -qE '^1 commit\(s\) on ' \
  || { echo "    the body did not report exactly the episode's one commit: $(printf '%s\n' "$body" | grep -E 'commit\(s\) on ')"; exit 1; }

# where the worker recorded nothing, the record says so rather than inventing a summary
printf '%s\n' "$body" | grep -qF "recorded no checkpoint" \
  || { echo "    an episode with no checkpoint did not say so"; exit 1; }
printf '%s\n' "$body" | grep -qF "recorded no decision" \
  || { echo "    an episode with no decision did not say so"; exit 1; }

# the composed body is still a legal record
git add -A >/dev/null
"$MJ" doctor 2>&1 | grep -q 'OK   session' \
  || { echo "    the composed body made the section invalid"; exit 1; }

# ---------------------------------------------------------------- the worker's own words
# The composed body is a hybrid: what the worker wrote down while the episode ran, and what
# git and the ledger prove. The half above covers an episode that wrote nothing. This covers
# the half no derivation can supply — without it the rule promises a source the case never
# reads back.
"$MJ" start "prove the checkpoint reaches the record" --scope lib >/dev/null
expect_exit 0 "$MJ" session start --worker "test/authored"
printf 'Rewrote the dispatcher so a section with no writer is named, not skipped.\nThe cap comes from the policy, not from a constant here.\n' \
  | "$MJ" checkpoint >/dev/null
"$MJ" session close < /dev/null > closed3.txt
rec3="$(cat closed3.txt)"
body3="$(awk 'c>=2{print} /^---$/{c++}' "$rec3")"

printf '%s\n' "$body3" | grep -qF "Rewrote the dispatcher so a section with no writer is named" \
  || { echo "    the checkpoint the worker wrote did not reach the record"; exit 1; }
printf '%s\n' "$body3" | grep -qF "The cap comes from the policy" \
  || { echo "    only the checkpoint's first line reached the record"; exit 1; }
printf '%s\n' "$body3" | grep -qF "recorded no checkpoint" \
  && { echo "    the record claims no checkpoint while carrying one"; exit 1; }

# the local path of a checkpoint is a fact about a disk and stays out of a shared record
printf '%s\n' "$body3" | grep -qF ".ai/local/" \
  && { echo "    the record names a local state path"; exit 1; }

git add -A >/dev/null
"$MJ" doctor 2>&1 | grep -q 'OK   session' \
  || { echo "    the record carrying a checkpoint made the section invalid"; exit 1; }

# ---------------------------------------------------------------- the lifecycle commits it
# A record nobody committed reaches no surface that reads session records. The lifecycle
# wrote one at every episode end and nothing committed it, so the gap was the default.
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID
git add -A >/dev/null; git commit -qm "before the lifecycle episode" >/dev/null

printf '{"session_id":"p-lifecycle","source":"startup"}' | ./.claude/hooks/majordomus-session-start >/dev/null 2>&1

# work the worker had staged when the window closed: it must survive uncommitted
echo "staged by the worker" > STAGED_WORK.txt; git add STAGED_WORK.txt >/dev/null
before="$(git rev-parse HEAD)"

printf '{"session_id":"p-lifecycle","reason":"clear"}' | ./.claude/hooks/majordomus-session-end >/dev/null 2>&1
after="$(git rev-parse HEAD)"

[ "$before" != "$after" ] \
  || { echo "    the lifecycle closed an episode and committed nothing"; exit 1; }

# exactly one path in that commit, and it is a session record
files="$(git show --name-only --format= "$after" | sed '/^$/d')"
[ "$(printf '%s\n' "$files" | wc -l | tr -d ' ')" = 1 ] \
  || { echo "    the record commit carried more than the record: $files"; exit 1; }
case "$files" in .ai/repo/sessions/*) ;; *) echo "    the record commit carried $files"; exit 1 ;; esac

# the worker's staged work stayed staged and uncommitted
git show --name-only --format= "$after" | grep -q '^STAGED_WORK.txt$' \
  && { echo "    the record commit swept the worker's staged work"; exit 1; }
git status --short STAGED_WORK.txt | grep -q '^A ' \
  || { echo "    the worker's staged work is no longer staged: $(git status --short STAGED_WORK.txt)"; exit 1; }

# and the record is clean in the tree, not reported as a staged deletion
#
# An `if` rather than `X && { ...; exit 1; }`. That form is fine in the middle of a case and
# is a trap at the end of one: when the guarded command finds nothing — the passing case —
# the list's status is 1, and a script's status is its last command's. The case then fails
# having printed nothing at all, which is the one failure mode a reader cannot act on.
dirty="$(git status --porcelain -- .ai/repo/sessions/)"
if [ -n "$dirty" ]; then
  echo "    the committed record is not clean: $dirty"; exit 1
fi
exit 0

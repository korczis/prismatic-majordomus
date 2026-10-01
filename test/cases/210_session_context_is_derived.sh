# majordomus-covers: session
# An episode's working context is derived on every read, not written once and left.
#
# The defect this case exists for had two halves, and each half made the other invisible.
#
#   The document under .ai/local/session-contexts/ was written by `session start` and never
#   updated. It held the context builder's output from the instant before the worker had
#   done anything, which is the least informative moment in an episode to freeze, and it was
#   called the working context anyway.
#
#   `majordomus session context` printed that file's *path*. So the command that answers
#   "what is my working context" answered with a filename, and nobody noticed the document
#   behind the filename was stale, because nobody was reading the document.
#
# And the document carried a `## Notes` heading for the worker to hand-edit. Measured on
# 2026-09-11 across the 23 episodes this repository's own checkout had opened: 23 carried the
# template, 0 carried a note. This case refuses its return.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
"$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a
git add -A >/dev/null; git commit -qm base

expect_exit 0 "$MJ" session start --worker "test/worker"

# ---------------------------------------------------------------- it is not a path
# The first refusal: a context is a document, not a filename. A single-line answer that
# names a file under the store is the old behaviour and is what this case exists to catch.
"$MJ" session context > ctx1.txt 2>&1 || { echo "    session context failed"; cat ctx1.txt; exit 1; }
[ -s ctx1.txt ] || { echo "    session context printed nothing"; exit 1; }
if [ "$(grep -c . ctx1.txt)" -le 2 ]; then
  echo "    session context returned $(grep -c . ctx1.txt) line(s); it answered with a path, not a context"
  cat ctx1.txt; exit 1
fi
grep -q '^# Working context of session ' ctx1.txt || {
  echo "    the composed context has no heading naming the episode"; cat ctx1.txt; exit 1; }

# --path is how a caller asks for the stored snapshot, and it still answers with one.
"$MJ" session context --path > snap.txt 2>&1 || { echo "    session context --path failed"; exit 1; }
case "$(cat snap.txt)" in
  .ai/local/session-contexts/*.md) ;;
  *) echo "    --path did not name the opening snapshot: $(cat snap.txt)"; exit 1 ;;
esac
[ -f "$(cat snap.txt)" ] || { echo "    --path named a file that does not exist"; exit 1; }

# ---------------------------------------------------------------- no magic dead section
# The heading nobody ever filled must not come back, in either the composed context or the
# snapshot. A section whose only producer is a worker remembering to edit Markdown is not a
# producer, and the store measured 23 for 23 on exactly that.
grep -q '^## Notes' ctx1.txt && { echo "    the composed context carries a ## Notes template"; exit 1; }
grep -q '^## Notes' "$(cat snap.txt)" && { echo "    the opening snapshot carries a ## Notes template"; exit 1; }

# An empty section names the command that fills it, so a worker reading its own context is
# told what is missing rather than left to infer the heading is decorative.
grep -q 'majordomus checkpoint' ctx1.txt || { echo "    the empty progress notes do not name \`majordomus checkpoint\`"; exit 1; }
grep -q 'majordomus decision' ctx1.txt  || { echo "    the empty decisions section does not name \`majordomus decision\`"; exit 1; }

# ---------------------------------------------------------------- it is alive
# The whole claim in one assertion: do something inside the episode, and read the context
# again. A frozen document is byte-identical here; a derived one is not.
echo b > lib/b
git add -A >/dev/null; git commit -qm "feat: b"
"$MJ" decision add "The context is derived" --why "a document written once at open is a snapshot" >/dev/null

"$MJ" session context > ctx2.txt 2>&1 || { echo "    session context failed after work"; exit 1; }
if cmp -s ctx1.txt ctx2.txt; then
  echo "    the working context did not change after a commit and a decision; it is frozen"
  exit 1
fi
grep -q 'The context is derived' ctx2.txt || {
  echo "    the decision this episode recorded is not in its working context"; cat ctx2.txt; exit 1; }
grep -q '1 commit(s)' ctx2.txt || {
  echo "    the commit this episode made is not counted in its working context"; cat ctx2.txt; exit 1; }

# The snapshot is a snapshot: it is evidence of the open and must NOT have moved.
grep -q 'The context is derived' "$(cat snap.txt)" && {
  echo "    the opening snapshot was rewritten; it is evidence and must stay frozen"; exit 1; }

# ---------------------------------------------------------------- it survives the close
# A record can be asked for by id after its episode has closed, and the answer then is still
# the episode's. The close path is the automatic one — no authored summary on stdin.
sid="$("$MJ" session status --json 2>/dev/null | sed 's/.*"session_id":"//; s/".*//')"
"$MJ" session close < /dev/null > closed.txt 2>&1 || { echo "    close failed"; cat closed.txt; exit 1; }
if [ -n "$sid" ]; then
  "$MJ" session context "$sid" > ctx3.txt 2>&1 || { echo "    session context <id> failed after close"; exit 1; }
  grep -q "$sid" ctx3.txt || { echo "    the closed episode's context does not name it"; exit 1; }
fi

# ---------------------------------------------------------------- an unknown episode
# The renderer composes from git and the clock, which answer for any string at all, so
# without a guard a typo comes back as a confident heading over facts belonging to no
# episode. Known means: it is the open one, or it left a snapshot, or it wrote a ledger line.
if "$MJ" session context s-no-such-episode > unknown.txt 2>&1; then
  echo "    session context accepted an episode that does not exist"; cat unknown.txt; exit 1
fi
grep -q 'No episode s-no-such-episode' unknown.txt || {
  echo "    an unknown episode was refused without saying so"; cat unknown.txt; exit 1; }

echo "    ok: the working context is composed on every read and the snapshot stays frozen"

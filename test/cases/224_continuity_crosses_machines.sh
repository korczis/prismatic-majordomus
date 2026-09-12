# majordomus-covers: session capture context
# A second machine, holding only what git carries: what a worker there can resume from, and
# what must not have travelled to it.
#
# The layer is two halves and the split is the whole design (ADR 0014). `.ai/repo/` is tracked
# and identical in every clone; `.ai/local/` is this checkout's own state, gitignored, never
# shared and never normative. A closed episode is the one continuity artefact that crosses
# that line: it is written into the tracked sessions section precisely so that the next
# worker — on another machine, in another clone, a week later — can find out what the last one
# did without being handed anything about the disk it happened on.
#
# Three things have to be true of that, and each is a different way of getting it wrong:
#
#   it works. A worker in a fresh clone can read the closed episodes of this branch. If the
#   record needed something under `.ai/local/` to be understood, it would be a local record
#   with a tracked path, and the section would be a promise nothing keeps.
#
#   nothing private travelled. The raw prompt archive — what the worker actually typed — is
#   the one store in this repository that is a transcript, and `project.never-store-
#   transcripts` is why it lives under `.ai/local/prompts/` and nowhere else. A clone that
#   has it is a repository that published its own conversations.
#
#   no private state is required. Every command a worker runs on arrival has to work in a
#   checkout that has never had an episode: `.ai/local/` does not exist yet, and absence is
#   an answer rather than a failure. The alternative is a tool that only works on the machine
#   that has already used it.
#
# The fixture is two clones of one bare remote rather than one repository read twice, because
# the thing being tested is the identity rule: a shared record names its repository by its
# remote and its worktree by an opaque id, and a second checkout has a different absolute path
# and the same remote. A fixture with one checkout would pass for the wrong reason.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }

# ---------------------------------------------------------------- machine one
# $T is the case's own scratch directory; the repository the runner made is not used here,
# because this case needs a remote and two working copies of it.
git init -q --bare "$T/remote.git"
git clone -q "$T/remote.git" "$T/one"
cd "$T/one" || exit 1
git config user.email t@example.com; git config user.name t
git commit -q --allow-empty -m init
"$MJ" init >/dev/null; "$MJ" update >/dev/null
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH

# A worker types something. This is the transcript half, and it is what must not travel.
printf '{"prompt":"the password is hunter2 and the client is unhappy","session_id":"one","role":"user"}' \
  | ./.claude/hooks/majordomus-capture >/dev/null 2>&1
must "the prompt archive was not written, so the case cannot prove it stays behind" \
  [ -d .ai/local/prompts ]
grep -rqF 'hunter2' .ai/local/prompts \
  || { echo "    the prompt archive does not hold what the worker typed"; exit 1; }

# ...and an episode opens, does something, and closes.
printf '{"session_id":"one","source":"startup"}' | ./.claude/hooks/majordomus-session-start >/dev/null 2>&1
MAJORDOMUS_PROVIDER_SESSION=one "$MJ" decision add "Chose the tracked store" \
  --why "a closed episode is the one continuity artefact that crosses machines" >/dev/null
printf '{"session_id":"one","reason":"clear"}' | ./.claude/hooks/majordomus-session-end >/dev/null 2>&1
rec="$(find .ai/repo/sessions -maxdepth 1 -name '2*.md' | head -n 1)"
must "no closed session record was written on machine one" [ -n "$rec" ]
sid="$(sed -n 's/^session_id: //p' "$rec" | head -n 1)"

# The record is committed, because a record nobody committed reaches nobody: the section's
# own contract calls these records shared with every surface, and every one of those surfaces
# reads the tracked tree.
git add -A >/dev/null; git commit -qm "a closed episode" >/dev/null
git push -q origin HEAD >/dev/null 2>&1
git ls-files --error-unmatch "$rec" >/dev/null 2>&1 \
  || { echo "    the closed session record is not tracked, so nothing can carry it"; exit 1; }
# and the local half is not tracked, whatever else happens
[ -z "$(git ls-files .ai/local)" ] \
  || { echo "    the local half is tracked:"; git ls-files .ai/local | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- machine two
# Only what git carries. No copying of directories, no fixture that hands the second checkout
# anything the first one had on disk: `git clone` is the whole transfer.
git clone -q "$T/remote.git" "$T/two"
cd "$T/two" || exit 1
git config user.email t@example.com; git config user.name t

# THE ASSERTION THIS CASE IS NAMED FOR, INVERTED: nothing private arrived.
must "the local half of the layer travelled to a clone" [ ! -e .ai/local ]
grep -rqF 'hunter2' . 2>/dev/null \
  && { echo "    what the worker typed on machine one is readable in a fresh clone"; exit 1; }
: # the grep above must not end the case when it correctly finds nothing

# No private state is required. Every one of these runs in a checkout that has never had an
# episode, and each answers absence as absence rather than failing.
expect_exit 0 "$MJ" context
expect_exit 0 "$MJ" session status
expect_grep 'No open session'
must "reading the layer created local state that reading must not create" [ ! -d .ai/local/state/sessions-open ]

# And the durable half is discoverable. Not with --all, which lifts the scoping rule: the
# point is that the ordinary, scoped question — what happened on this branch of this
# repository — finds a record written by another checkout of it, because a shared record names
# its repository by its remote rather than by a path on somebody's disk.
expect_exit 0 "$MJ" session list
expect_grep "$sid"
expect_exit 0 "$MJ" session show "$sid"
expect_grep 'Chose the tracked store'
# `same_branch` and not `same_worktree_same_branch`: the record is honest about having been
# written somewhere else, which is what lets a reader weigh it.
expect_exit 0 "$MJ" session latest
expect_grep 'Match: +same_branch'

# The record carries what the repository can prove and nothing about the machine that ran it,
# which is the reason it is portable at all: it names its worktree by an opaque id rather than
# by a path on somebody's disk, and it does not name the person who ran it. 63_session_records
# owns the refusal as a doctrine; this is the other side of it, read in the checkout that
# receives the record.
#
# `repository_id` is not covered by that: a shared record names its repository by its remote,
# and in this fixture the remote is a path on this disk because the fixture cannot have a
# GitHub. That is the identity the second checkout matches on, and it is the field that makes
# the record findable here at all.
expect_no_grep '^worktree:' ".ai/repo/sessions/$(basename "$rec")"
expect_grep '^worktree_id: [0-9a-f]+$' ".ai/repo/sessions/$(basename "$rec")"
expect_no_grep '^owner:' ".ai/repo/sessions/$(basename "$rec")"

# ---------------------------------------------------------------- and the typed reader
# `continuity.state` is the surface that answers "what is this checkout holding", and in a
# clone that has never run the lifecycle its honest answer is `present: false` with no
# findings — not an error, and not a silent claim that the lifecycle has run. It is not
# driven from here: the capability declares no CLI projection (it is MCP and HTTP only), so
# reaching it from a case means standing a server up, and this case would then be about a
# port. The reader's own behaviour on an empty and on a partial store is held by the unit
# tests in apps/majordomus-cli/src/capability/builtin/continuity.rs, which are what the
# per-domain coverage threshold in scripts/session-coverage-threshold measures.
#
# What this case does assert about it is the precondition those tests cannot: that a fresh
# clone really has nothing for it to read.
must "the clone carries a state directory it should not have" [ ! -d .ai/local/state ]

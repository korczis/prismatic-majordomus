# majordomus-covers: adr
# Allocating a decision identity, and refusing one two branches both took.
#
# On the night of 2026-09-11 three branches of this repository claimed 0044 and two claimed
# 0043. Every duplicate makes `generate --strict` exclude every claimant of that identity and
# refuse the whole tree, and the error names the identity rather than the two sessions — so
# it surfaces at merge time, hours from either cause. Three separate sessions each hand-rolled
# the same survey to find their way out, independently and correctly, because nothing offered
# it. This case holds both halves of the answer:
#
#   the survey names its sources. A number is not "free" because the sequence has a hole in
#   it; a hole means an identity was taken and withdrawn, or taken and not yet written.
#   Allocation is monotonic and never recycles, and `adr next` says which source set the
#   high-water mark and which sources it could not reach — because the reason those three
#   sessions each wrote their own was that no answer here carried its denominator.
#
#   the refusal happens on the branch. `adr check` asks the other refs whether a different
#   decision already stands at an identity this tree adds, and `doctor` runs the same
#   examination in the pre-commit hook. A rule whose precondition can only be observed after
#   the branch is pushed cannot be followed as written.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add . && git commit -qm base
trunk="$(git rev-parse --abbrev-ref HEAD)"

# ---------------------------------------------------------------- the surface
expect_exit 2 "$MJ" adr nonsense
expect_grep 'list\|show\|next\|propose\|check\|affected'
expect_exit 2 "$MJ" adr next --nonsense
expect_grep 'unknown option'

# ---------------------------------------------------------------- the survey
# an empty layer has no claims, and that is an answer: the first identity is 0001
expect_exit 0 "$MJ" adr next
expect_grep 'next free identity: adr-0001'
# every source is named with what it claimed, and the one that is not reachable says so
# rather than being silently left out of the denominator
expect_grep 'this working tree'
expect_grep 'every sibling worktree'
expect_grep 'every ref'
expect_grep 'the peer board'
expect_grep 'the peer board of the shared server +not reachable'

expect_exit 0 "$MJ" adr propose "The first decision"
expect_grep 'proposed: adr-0001'
expect_exit 0 "$MJ" adr propose "The second decision"
expect_grep 'proposed: adr-0002'
git add -A && git commit -qm "two decisions"

expect_exit 0 "$MJ" adr next
expect_grep 'next free identity: adr-0003'
expect_grep 'highest claimed: adr-0002'

# a decision on another branch is claimed even though this tree cannot see the file
git checkout -q -b other
expect_exit 0 "$MJ" adr propose "A decision on another branch"
expect_grep 'proposed: adr-0003'
other_file="$(ls .ai/repo/adrs/0003-*.md)"
git add -A && git commit -qm "a third, elsewhere"
git checkout -q "$trunk"
[ ! -e "$other_file" ] || { echo "    the branch's decision is still in this tree"; exit 1; }
expect_exit 0 "$MJ" adr next
expect_grep 'next free identity: adr-0004'
expect_grep 'every ref: branches, tags, remotes, worktree HEADs +3 identities'

# a decision written into a sibling worktree and never committed is claimed too: authored
# and not yet committed is a real claim, and it is exactly the one a branch scan cannot see
wt="$T/sibling"
git worktree add -q --detach "$wt" "$trunk"
cp "$(ls .ai/repo/adrs/0001-*.md)" "$wt/.ai/repo/adrs/0007-held-in-a-worktree.md"
expect_exit 0 "$MJ" adr next
expect_grep 'next free identity: adr-0008'
expect_grep 'every sibling worktree, as it stands on disk +[0-9]+ identities'

# the holes below the high-water mark are reported, and they are reported as spent. A hole
# means an identity was taken and withdrawn, or taken and not yet written, and no survey can
# tell those two from a number nobody ever used — so none of them is handed out.
expect_grep 'spent and not free'
expect_grep '^  0004'
expect_grep '^  0005'
expect_grep '^  0006'

# and when a hole is cited by something in the tree, the citation is named. This is the 0034
# case of the real repository: an identity no branch, tag or working tree carries, which
# master's own ADR 0035 and docs/ENTRY_AUDIT.md already refer to. The citation is evidence
# for the reader, never the mechanism — the hole is spent whether or not anything cites it.
printf 'See ADR 0005 for the reasoning.\n' > NOTES.md
git add NOTES.md
expect_exit 0 "$MJ" adr next
expect_grep '^  0005 +cited by: NOTES.md'
expect_grep '^  0004 +nothing cites it here'

rm "$wt/.ai/repo/adrs/0007-held-in-a-worktree.md"
git worktree remove --force "$wt"

# a decision that is deleted does not free its identity: the refs remember that it was added
git rm -q "$(ls .ai/repo/adrs/0002-*.md)"
git commit -qm "the second decision withdrawn"
expect_exit 0 "$MJ" adr next
expect_grep 'next free identity: adr-0004'
# and it is not a hole either: a hole is an identity nothing claims, and `other` still
# carries this one. (The holes section is what this asserts; the outstanding-allocations
# section below reports the same identity as held by that branch, which is the other half
# of the same fact.)
expect_no_grep '^  0002  (cited by|nothing cites it here)'

# ---------------------------------------------------------------- the refusal
# a clean branch passes, and says what it measured rather than nothing
git checkout -q -b mine
expect_exit 0 "$MJ" adr propose "A decision of my own"
expect_grep 'proposed: adr-0004'
mine="$(ls .ai/repo/adrs/0004-*.md)"
expect_exit 0 "$MJ" adr check
expect_grep '1 identity added here, measured against'
# the pre-commit hook runs doctor, and doctor runs this same examination — one inventory of
# the identities, not two. (The fixture's doctor reports unrelated warnings of its own, so
# what is asserted here is the decision finding, not the run's status.)
LAST_OUT="$("$MJ" doctor 2>&1 || true)"
expect_no_grep 'identity 0003 is also claimed'

# now take an identity `other` already carries. `propose` would never hand it out — this is
# the renumbering a person does by hand, which is how every one of tonight's duplicates
# arrived — so the file is moved into place the way a person moves it.
mv "$mine" .ai/repo/adrs/0003-a-decision-of-my-own.md
mine=.ai/repo/adrs/0003-a-decision-of-my-own.md
sed -i.bak 's/^id: adr-0004$/id: adr-0003/' "$mine" && rm -f "$mine.bak"

# the identity is refused on the branch, before the merge, naming both documents and the
# refs that carry the other one
expect_exit 10 "$MJ" adr check
expect_grep 'identity 0003 is also claimed by'
expect_grep "$other_file"
expect_grep 'on: .*other'
expect_grep 'majordomus adr next'
# and the pre-commit hook refuses it, because the hook runs doctor and doctor runs the same
# examination
LAST_OUT="$("$MJ" doctor 2>&1 || true)"
expect_grep 'FAIL +adr .*identity 0003 is also claimed by'

# renumbering to what the allocator says is free clears it, in both surfaces
free="$("$MJ" adr next --json | sed -n 's/.*"next":"\([0-9]*\)".*/\1/p')"
[ "$free" = 0004 ] || { echo "    expected 0004 from the allocator, got '$free'"; exit 1; }
mv "$mine" ".ai/repo/adrs/$free-a-decision-of-my-own.md"
sed -i.bak "s/^id: adr-0003$/id: adr-$free/" ".ai/repo/adrs/$free-a-decision-of-my-own.md"
rm -f ".ai/repo/adrs/$free-a-decision-of-my-own.md.bak"
expect_exit 0 "$MJ" adr check
expect_grep '1 identity added here, measured against'
LAST_OUT="$("$MJ" doctor 2>&1 || true)"
expect_no_grep 'identity 0003 is also claimed'

# ---------------------------------------------------------------- could not look
# A survey that could not reach the other refs has not cleared this branch of anything, and
# says so rather than exiting clean with nothing said (project.empty-is-not-failure).
expect_exit 0 env MJ_ADR_BASE=refs/nothing/here "$MJ" adr check
expect_grep 'the other refs were NOT surveyed'
expect_grep 'no base ref resolves here'
expect_no_grep 'identity added here'
LAST_OUT="$(MJ_ADR_BASE=refs/nothing/here "$MJ" adr check --json 2>&1)"
expect_grep '"surveyed":false'
LAST_OUT="$("$MJ" adr check --json 2>&1)"
expect_grep '"surveyed":true'

# ------------------------------------------------------- allocated, and not landed
# The half of the survey a high-water mark cannot express. On 2026-09-12 this repository's
# decisions ran to 0055 on master and to 0059 on the refs, and that gap was measured twice
# within one hour — once as four unlanded identities, once as seven — because each reading
# scanned only the branches its author already knew about. `next` answered 0060 both times,
# correctly, and said nothing about the identities in between: a high-water mark is one
# number and hides every one of them.
#
# So the identities the base ref does not carry are named, with who holds each. Nothing is
# decided: an identity on an open branch and one on an abandoned branch are the same bytes
# to any survey, and which of the two a number is can only be settled by a person.
git checkout -q "$trunk"
expect_exit 0 "$MJ" adr next
# the base is named, because a count of what is missing from an unnamed ref is not a fact
expect_grep 'allocated and not landed on (master|main)'
expect_grep '3 allocated and not landed on'
# each outstanding identity names a holder a person can go and open: the branch that carries
# it, or this tree when the claim has not been committed anywhere yet
expect_grep '^  0003 +held by: .*other'
expect_grep '^  0004 +held by: this working tree'
# a ref that once ADDED a decision and has since dropped it is not one of its holders. 0002
# was withdrawn on the trunk and survives on `other`, so `other` holds it and the trunk does
# not — answering "who holds this now" from the add-log would name the branch that deleted it.
expect_grep '^  0002 +held by: other'
expect_no_grep '^  0002 +held by:.*(master|main)'
# an identity the base already carries is nobody's outstanding allocation
expect_no_grep '^  0001 +held by'
# and the survey states what it cannot reach at all, rather than presenting its number as a
# guarantee: a ref scan sees what was pushed and the board forgets on reconnect, so two
# sessions allocating in the same minute are invisible to both
# (project.a-verdict-states-its-subject)
expect_grep 'not surveyed, and not surveyable'
expect_grep 'same minute'
LAST_OUT="$("$MJ" adr next --json 2>&1)"
expect_grep '"outstanding_base":"(master|main)"'
expect_grep '"id":"0003","held_by":"[^"]*other'
expect_grep '"id":"0004","held_by":"this working tree'

# A survey that cannot resolve a base has not established that anything is outstanding, and
# says so rather than reporting an empty list as an answer (project.empty-is-not-failure).
expect_exit 0 env MJ_ADR_BASE=refs/nothing/here "$MJ" adr next
expect_grep 'outstanding allocations were NOT surveyed'
expect_no_grep 'allocated and not landed on'
# the number itself still comes out: the high-water mark needs no base
expect_grep 'next free identity: adr-'
LAST_OUT="$(MJ_ADR_BASE=refs/nothing/here "$MJ" adr next --json 2>&1)"
expect_grep '"outstanding_base":null'

# the identical identity at the identical path is an edit, not a competing claim: two
# branches revising one decision is resolved by merging and is nobody's collision
git checkout -q "$trunk"
git checkout -q -b editing
sed -i.bak 's/^status: proposed$/status: accepted/' "$(ls .ai/repo/adrs/0001-*.md)"
rm -f .ai/repo/adrs/0001-*.md.bak
expect_exit 0 "$MJ" adr check

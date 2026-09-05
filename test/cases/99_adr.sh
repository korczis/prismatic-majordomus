# majordomus-covers: adr
# majordomus-negative: adr doctor
# Architecture decisions as objects: the surface, what propose writes and refuses to write,
# how an identity is allocated when several workers ask at once, and every finding the
# adr-integrity doctrine has to produce.
#
# The two guarantees this case exists for:
#
#   propose never writes `accepted`. Every assertion about status below follows a real
#   invocation, because a tool that can write `accepted` can turn its own inference into
#   repository truth by writing it down, and a reader months later cannot tell which.
#
#   two workers proposing at the same moment get two identities. This repository shipped
#   two 0005s and two 0007s before anything checked, so the concurrent case is exercised
#   with real concurrent processes rather than asserted.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add . && git commit -qm base

# ---------------------------------------------------------------- surface
expect_exit 2 "$MJ" adr
expect_grep 'usage: majordomus adr list'
expect_exit 2 "$MJ" adr nonsense
expect_grep 'unknown subcommand'

# a fresh layer has no decisions, and that is an answer rather than a failure
expect_exit 0 "$MJ" adr list
expect_grep '(none)'
expect_exit 0 "$MJ" adr check

# ---------------------------------------------------------------- propose
expect_exit 0 "$MJ" adr propose "The state directory is never tracked"
expect_grep 'proposed: adr-0001'
expect_grep 'accepting it is a person editing that field'
adr1="$(ls .ai/repo/adrs/0001-*.md)"
grep -q '^status: proposed$' "$adr1"
grep -q '^id: adr-0001$' "$adr1"
grep -q '^kind: adr$' "$adr1"
grep -q '^schema: adr/v1$' "$adr1"
grep -q '^  origin: authored$' "$adr1"
# the body carries the sections a reader relies on, already non-empty
grep -q '^## Context$' "$adr1"; grep -q '^## Decision$' "$adr1"; grep -q '^## Consequences$' "$adr1"

# the status is not the caller's to choose, however the caller asks
expect_exit 15 "$MJ" adr propose "Accepted by fiat" --status accepted
expect_grep 'not yours to choose'
expect_exit 15 "$MJ" adr propose "Accepted by fiat" --status=accepted
# and nothing was written by the refusal
[ "$(ls .ai/repo/adrs/[0-9]*.md 2>/dev/null | wc -l | tr -d ' ')" = 1 ]

# ---------------------------------------------------------------- provenance
# a reference is typed, and a file: reference must resolve
expect_exit 2 "$MJ" adr propose "x" --from "nonsense:1"
expect_grep 'unknown type'
expect_exit 2 "$MJ" adr propose "x" --from "file:does/not/exist.md"
expect_grep 'does not exist'
expect_exit 2 "$MJ" adr propose "x" --from "bare-value"
expect_grep 'not <type>:<value>'

# a record derived from something says so, and says what
expect_exit 0 "$MJ" adr propose "Discovery reads the index" --from "decision:t-1" --from "file:.ai/manifest.yaml" --tag records
adr2="$(ls .ai/repo/adrs/0002-*.md)"
grep -q '^  origin: extracted$' "$adr2"
grep -q '^    - decision:t-1$' "$adr2"
grep -q '^    - file:.ai/manifest.yaml$' "$adr2"
grep -q '^status: proposed$' "$adr2"

# ---------------------------------------------------------------- identity under concurrency
# eight workers proposing at once. The identity is allocated under a lock over the section
# directory, so the answer is eight identities, not one repeated.
for i in 1 2 3 4 5 6 7 8; do "$MJ" adr propose "Concurrent decision $i" >/dev/null 2>&1 & done
wait
[ "$(ls .ai/repo/adrs/[0-9]*.md | wc -l | tr -d ' ')" = 10 ]
[ "$(ls .ai/repo/adrs/[0-9]*.md | sed 's|.*/||' | cut -c1-4 | sort -u | wc -l | tr -d ' ')" = 10 ]
# and no lock was left behind
[ ! -e .ai/repo/adrs/.id.lock ]

git add . && git commit -qm decisions

# ---------------------------------------------------------------- list and show
expect_exit 0 "$MJ" adr list
expect_grep 'adr-0001'
expect_exit 0 "$MJ" adr list --status proposed
expect_grep 'adr-0002'
expect_exit 2 "$MJ" adr list --status invented
expect_grep 'must be one of'
# a person refers to a decision by its number
expect_exit 0 "$MJ" adr show 0001
expect_grep 'The state directory is never tracked'
expect_exit 0 "$MJ" adr show adr-0001
expect_exit 12 "$MJ" adr show adr-0777
expect_grep 'no decision with id'
"$MJ" adr list --json | grep -q '"id":"adr-0001"'
"$MJ" adr list --json | grep -q '"status":"proposed"'
"$MJ" adr list --json | grep -q '"valid":true'

# ---------------------------------------------------------------- the doctrine, finding by finding
# Each check below mutates the one thing it is about and puts the tree back afterwards, so
# a validator that passed for the wrong reason cannot hide behind an earlier mutation.
probe() { # description, the text the finding must carry, then the mutation as a command
  local what="$1" want="$2"; shift 2
  cp "$adr1" "$T/keep.md"
  "$@"
  git add . >/dev/null
  "$MJ" adr check >"$T/out" 2>&1 && { echo "    adr check passed despite: $what"; exit 1; }
  grep -qF "$want" "$T/out" || { echo "    the finding for '$what' does not name the reason; got:"; cat "$T/out"; exit 1; }
  "$MJ" doctor >"$T/dr" 2>&1 || true
  grep -q '^FAIL adr' "$T/dr" || { echo "    doctor did not fail on: $what"; sed -n '/adr/p' "$T/dr"; exit 1; }
  cp "$T/keep.md" "$adr1"; git add . >/dev/null
}
mutate() { sed -i.bak "$1" "$adr1" && rm -f "$adr1.bak"; }

probe "an unknown status" 'status must be one of' mutate 's/^status: proposed$/status: maybe/'
probe "an unknown front-matter key" 'unknown front-matter key' mutate 's/^kind: adr$/kind: adr\
confidence: 0.93/'
probe "a schema version nothing reads" 'schema must be adr/v1' mutate 's|^schema: adr/v1$|schema: adr/v9|'
probe "superseded with no replacement" 'superseded_by is missing' mutate 's/^status: proposed$/status: superseded/'
probe "a supersedes target that does not exist" 'which is not a decision here' mutate 's/^status: proposed$/status: proposed\
supersedes:\
  - adr-0777/'
probe "a body section removed" 'missing or empty in section' mutate 's/^## Consequences$/## Aftermath/'
probe "no front matter at all" 'no front matter' sed -i.bak '1,10d' "$adr1"

# an extracted record may not claim to have been accepted: this is the check that stops a
# machine's inference becoming repository truth by being written down
cp "$adr2" "$T/keep2.md"
sed -i.bak 's/^status: proposed$/status: accepted/' "$adr2" && rm -f "$adr2.bak"
git add . >/dev/null
expect_exit 10 "$MJ" adr check
expect_grep 'proposed until a person accepts it'
cp "$T/keep2.md" "$adr2"; git add . >/dev/null

# an extracted record with no evidence is an assertion, and is refused as one
sed -i.bak 's/^  derived_from:$/  derived_from_removed:/' "$adr2" && rm -f "$adr2.bak"
sed -i.bak '/^    - decision:t-1$/d; /^    - file:.ai\/manifest.yaml$/d; /^  derived_from_removed:$/d' "$adr2" && rm -f "$adr2.bak"
git add . >/dev/null
expect_exit 10 "$MJ" adr check
expect_grep 'derived_from names nothing'
cp "$T/keep2.md" "$adr2"; git add . >/dev/null

# ---------------------------------------------------------------- duplicate identity
# The failure this whole doctrine exists for. Two records claiming one identity is caught
# by identity and, separately, by file-name number, so neither check alone carries it.
cp "$adr1" .ai/repo/adrs/0020-a-second-claim.md
git add . >/dev/null
expect_exit 10 "$MJ" adr check
expect_grep 'identity adr-0001 is claimed by more than one record'
rm -f .ai/repo/adrs/0020-a-second-claim.md; git add . >/dev/null

# the same number under two file names, before either record is even parsed
cp "$adr1" .ai/repo/adrs/0001-another-file.md
sed -i.bak 's/^id: adr-0001$/id: adr-0031/' .ai/repo/adrs/0001-another-file.md && rm -f .ai/repo/adrs/0001-another-file.md.bak
git add . >/dev/null
expect_exit 10 "$MJ" adr check
expect_grep 'file-name number 0001 is used by more than one file'
rm -f .ai/repo/adrs/0001-another-file.md; git add . >/dev/null

# ---------------------------------------------------------------- one-sided supersession
# A chain walkable from one end only is not a chain; the record standing in for another
# must be named by it.
sed -i.bak 's/^status: proposed$/status: proposed\
supersedes:\
  - adr-0002/' "$adr1" && rm -f "$adr1.bak"
git add . >/dev/null
expect_exit 10 "$MJ" adr check
expect_grep 'does not name adr-0001 in superseded_by'
# name it back, and both ends resolve
sed -i.bak 's/^status: proposed$/status: superseded\
superseded_by: adr-0001/' "$adr2" && rm -f "$adr2.bak"
git add . >/dev/null
expect_exit 0 "$MJ" adr check

# ---------------------------------------------------------------- the tree is sound again
expect_exit 0 "$MJ" adr check
expect_grep 'every identity unique'
"$MJ" adr check --json | grep -q '"ok":true'
"$MJ" doctor 2>&1 | grep -q '^OK   adr'

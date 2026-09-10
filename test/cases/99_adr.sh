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
[ "$(ls .ai/repo/adrs/[0-9]*.md | sed 's|.*/||' | cut -c1-4 | LC_ALL=C sort -u | wc -l | tr -d ' ')" = 10 ]
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

# ---------------------------------------------------------------- what a decision put in force
# `related` is the forward edge, and every type is checked where that type says the target
# lives. The reverse direction is never written: it is this graph read backwards.
cp "$adr1" "$T/keep1.md"
mkdir -p docs test/cases
printf '# a case this repository has, so that a test: reference has something to resolve to\n' > test/cases/99_probe.sh
# a repository that declares where its cases live gets test nodes; without the class the
# reference is to a tracked file the graph has no node for, which is silence, not a finding
printf '  - id: test\n    kind: test\n    discovery: vcs\n    pathspec: \x27:(glob)test/cases/*.sh\x27\n    required: false\n' >> .ai/repo/knowledge/sources.yaml
printf 'claims:\n  - id: state-untracked\n    claim: The state directory is never tracked\n' > docs/CLAIMS.yaml
sed -i.bak 's|^status: proposed$|status: proposed\
related:\
  - rule:majordomus.adr-integrity\
  - claim:state-untracked\
  - file:.ai/manifest.yaml\
  - test:test/cases/99_probe.sh|' "$adr1" && rm -f "$adr1.bak"
git add . >/dev/null
expect_exit 0 "$MJ" adr check
# each reference became an edge of the graph, with the key that stated it as provenance
"$MJ" knowledge edges > edges.txt
expect_grep '^declares +adr:adr-0001 +rule:majordomus\.adr-integrity +.*:related\.0$' edges.txt
expect_grep '^supports +adr:adr-0001 +claim:state-untracked +.*:related\.1$' edges.txt
expect_grep '^tested_by +adr:adr-0001 +test:test/cases/99_probe\.sh +.*:related\.3$' edges.txt
rm -f edges.txt
# a rule the effective set does not have
sed -i.bak 's|^  - rule:majordomus.adr-integrity$|  - rule:majordomus.no-such-rule|' "$adr1" && rm -f "$adr1.bak"
git add . >/dev/null
expect_exit 10 "$MJ" adr check
expect_grep 'names a rule the effective set does not have'
sed -i.bak 's|^  - rule:majordomus.no-such-rule$|  - rule:majordomus.adr-integrity|' "$adr1" && rm -f "$adr1.bak"
# a claim the matrix does not have
sed -i.bak 's|^  - claim:state-untracked$|  - claim:no-such-claim|' "$adr1" && rm -f "$adr1.bak"
git add . >/dev/null
expect_exit 10 "$MJ" adr check
expect_grep 'names a claim docs/CLAIMS\.yaml does not have'
sed -i.bak 's|^  - claim:no-such-claim$|  - claim:state-untracked|' "$adr1" && rm -f "$adr1.bak"
# a path the repository does not contain
sed -i.bak 's|^  - file:.ai/manifest.yaml$|  - file:lib/gone.sh|' "$adr1" && rm -f "$adr1.bak"
git add . >/dev/null
expect_exit 10 "$MJ" adr check
expect_grep 'related "file:lib/gone.sh" names a path that does not exist'
sed -i.bak 's|^  - file:lib/gone.sh$|  - file:.ai/manifest.yaml|' "$adr1" && rm -f "$adr1.bak"
# a type that is not one a decision may state forward: provenance has its own vocabulary
sed -i.bak 's|^  - test:test/cases/99_probe.sh$|  - session:s-1|' "$adr1" && rm -f "$adr1.bak"
git add . >/dev/null
expect_exit 10 "$MJ" adr check
expect_grep 'unknown front-matter key|has an unknown type "session"'
cp "$T/keep1.md" "$adr1"; rm -f docs/CLAIMS.yaml; git add -A >/dev/null
expect_exit 0 "$MJ" adr check

# ---------------------------------------------------------------- what a change set reaches
# The forward edge read in the direction a reviewer needs: this file has a decision behind
# it. Review notes only — the exit code never says a decision stopped holding.
cp "$adr1" "$T/keep1.md"
mkdir -p docs test/cases
printf '# a case this repository has\n' > test/cases/99_probe.sh
printf 'x\n' > docs/governed.md
git add . >/dev/null; git commit -qm "the file a decision will name"
sed -i.bak 's|^status: proposed$|status: proposed\
related:\
  - file:docs/governed.md\
  - test:test/cases/99_probe.sh|' "$adr1" && rm -f "$adr1.bak"
git add . >/dev/null; git commit -qm "the decision names it"
# a clean tree reaches nothing
expect_exit 0 "$MJ" adr affected
expect_grep 'no decision names anything this change set touches'
# touching a named file names the decision, and says why
printf 'more\n' >> docs/governed.md
expect_exit 0 "$MJ" adr affected
expect_grep 'WARN adr adr-0001 .*names docs/governed\.md'
expect_grep 'decision\(s\) to read'
# a path no decision names reaches nothing
git checkout -- docs/governed.md
printf 'y\n' > docs/unrelated.md
expect_exit 0 "$MJ" adr affected
expect_grep 'no decision names anything'
rm -f docs/unrelated.md
# the record's own file changing is its own reason
printf '\nmore prose\n' >> "$adr1"
expect_exit 0 "$MJ" adr affected
expect_grep 'the record itself changed'
"$MJ" adr affected --json | grep -q '"reason":"record"'
cp "$T/keep1.md" "$adr1"; rm -rf docs/governed.md test/cases/99_probe.sh; git add -A >/dev/null
expect_exit 0 "$MJ" adr check

# ---------------------------------------------------------------- the tree is sound again
expect_exit 0 "$MJ" adr check
expect_grep 'every identity unique'
"$MJ" adr check --json | grep -q '"ok":true'
"$MJ" doctor 2>&1 | grep -q '^OK   adr'

# ---------------------------------------------------------------- identity across the repository
# One directory is not the repository. A number is taken when any branch has ever used it,
# and when any other worktree holds it right now — including work that is authored and not
# yet committed, which no ref can answer for.
#
# This is a regression. A session here proposed 0028, found it held by a peer's uncommitted
# record, took 0029, and found that one claimed by master while the branch was being
# written; both had to be renumbered by hand. The allocator read one directory and the lock
# it holds is exclusive over one worktree, which says nothing about the thirty others.
git add -A >/dev/null; git commit -qm "before the branch" >/dev/null 2>&1 || true

# a number used on another branch and not present here at all
git checkout -q -b other-branch
"$MJ" adr propose "Decided on a branch" >/dev/null
git add -A >/dev/null && git commit -qm "a decision on a branch" >/dev/null
high="$(ls .ai/repo/adrs/[0-9][0-9][0-9][0-9]-*.md | sed 's|.*/||; s|-.*||' | LC_ALL=C sort -n | tail -1)"
git checkout -q -
# the record is gone from this tree; the number is not free
test ! -e ".ai/repo/adrs/$high-decided-on-a-branch.md"
expect_exit 0 "$MJ" adr propose "After the branch"
next="$(printf '%04d' "$((10#$high + 1))")"
test -e ".ai/repo/adrs/$next-after-the-branch.md" \
  || { echo "    a number used on another branch was handed out again: wanted $next"; ls .ai/repo/adrs/; exit 1; }
git add -A >/dev/null && git commit -qm "after the branch" >/dev/null

# a number held by a sibling worktree, uncommitted: the case a ref cannot see
git worktree add -q "$T/sibling" -b sibling >/dev/null 2>&1
sib="$(printf '%04d' "$((10#$next + 7))")"
printf -- '---\nschema: adr/v1\nid: adr-%s\nkind: adr\ntitle: Held in a sibling\nstatus: proposed\n---\n' "$sib" \
  > "$T/sibling/.ai/repo/adrs/$sib-held-in-a-sibling.md"
expect_exit 0 "$MJ" adr propose "After the sibling"
after="$(printf '%04d' "$((10#$sib + 1))")"
test -e ".ai/repo/adrs/$after-after-the-sibling.md" \
  || { echo "    a number held uncommitted in a sibling worktree was handed out again: wanted $after"; ls .ai/repo/adrs/; exit 1; }
rm -f ".ai/repo/adrs/$after-after-the-sibling.md"
git worktree remove --force "$T/sibling" >/dev/null 2>&1 || true

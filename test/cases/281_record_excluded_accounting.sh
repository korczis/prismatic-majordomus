# majordomus-covers: session checkpoint handover
# `changed_files` carries its own denominator.
#
# test/cases/136 proves the classifier excludes the right paths. This case proves the record
# says so. The two are different failures: a classifier that silently shortens a list leaves
# a record that reads as an episode which touched two files, when the truth is that it
# touched two and the tool touched ninety-seven. Measured on this repository's own corpus:
# .ai/repo/sessions/20260911T072724Z--s-20260910214152-71cb--… names twelve paths, of which
# ten are sibling session records of the same episode and one is a temp file. Classified, the
# list is two. Two with no denominator is a different claim from "two, and ten excluded".
#
# The count is also the grandfather boundary the doctrine validator uses: a record carrying
# `changed_files_excluded` was written by the classifier and is held to it, one without it
# predates the classifier and is reported but never blocks. That makes the key's presence
# load-bearing, which is why it is asserted here on all three record kinds and asserted to
# be present even when its value is 0.
. "$ROOT/test/lib.sh"

must() { local why="$1"; shift; "$@" || { printf '    %s (failed: %s)\n' "$why" "$*"; return 1; }; }
# the value of a scalar front-matter key
val() { sed -n "s/^$2: //p" "$1" | head -n 1; }
# the entries of the changed_files_excluded_by list, one per line
by() { awk '/^changed_files_excluded_by:$/ { f = 1; next } f && /^  - / { sub(/^  - /, ""); print; next } f { exit }' "$1"; }
listed() { awk '/^changed_files:$/ { f = 1; next } f && /^  - / { sub(/^  - /, ""); print; next } f { exit }' "$1"; }

"$MJ" init >/dev/null; "$MJ" update >/dev/null

# Two declarations, so that the breakdown has two names in it and a single-tag bug cannot
# pass. docs/generated/ comes from the scope file, site/data/generated/ from the merge
# driver's attributes — the same two test/cases/136 sets, for the same reason.
sed -i.bak "s|^    paths: \[\]$|    paths: ['docs/generated/']|" .ai/repo/scope.yaml
rm -f .ai/repo/scope.yaml.bak
grep -q "docs/generated/" .ai/repo/scope.yaml || { echo "    the scope fixture did not apply"; exit 1; }
printf 'site/data/generated/**            merge=derived\n' > .gitattributes

mkdir -p docs/generated site/data/generated lib
printf 'seed\n' | tee docs/generated/a.json docs/generated/b.json docs/generated/c.json \
  site/data/generated/x.json site/data/generated/y.json lib/work.sh > /dev/null
git add -A && git commit -qm base

# One real file, three from one declaration, two from another: 1 kept, 5 excluded, and the
# breakdown must be scope-generated=3 plus gitattributes-derived=2. Constants that happen to
# be right for one shape cannot be right for this one.
printf 'x\n' >> lib/work.sh
printf 'x\n' >> docs/generated/a.json
printf 'x\n' >> docs/generated/b.json
printf 'x\n' >> docs/generated/c.json
printf 'x\n' >> site/data/generated/x.json
printf 'x\n' >> site/data/generated/y.json

# ---------------------------------------------------------------- a checkpoint
printf 'progress\n' | "$MJ" start "account" --scope lib >/dev/null
printf 'a note\n' | "$MJ" checkpoint >/dev/null
CP="$(find .ai/local/state/checkpoints -name '*.md' | head -n 1)"
expect_file "$CP"
must "the checkpoint states what it excluded" [ -n "$(val "$CP" changed_files_excluded)" ]
must "and the count is the number excluded"  [ "$(val "$CP" changed_files_excluded)" = 5 ]
must "the kept list is the one real file"    [ "$(listed "$CP" | grep -c .)" = 1 ]
must "named by the declaration that did it"  [ "$(by "$CP" | grep -c '^scope-generated=3$')" = 1 ]
must "and by the other declaration too"      [ "$(by "$CP" | grep -c '^gitattributes-derived=2$')" = 1 ]
# the breakdown is an accounting, not a decoration: it must add up to the total
SUM="$(by "$CP" | sed 's/.*=//' | awk '{ s += $1 } END { print s + 0 }')"
must "and the breakdown sums to the total"   [ "$SUM" = "$(val "$CP" changed_files_excluded)" ]

# ---------------------------------------------------------------- a handover
printf '# Objective\nfix it\n# Current State\nhalf\n# Next Action\nfinish\n' | "$MJ" handover >/dev/null
HO="$(find .ai/local/state/handovers -name '*.md' | head -n 1)"
expect_file "$HO"
must "the handover states it too" [ -n "$(val "$HO" changed_files_excluded)" ]
# Both writers are the same accounting, so the same tree gives the same answer. The
# checkpoint just written does not appear in either count: it lives under .ai/local/, which
# git ignores, so it never reaches the dirty tree the classifier is handed at all.
must "with the same count over the same tree" [ "$(val "$HO" changed_files_excluded)" = 5 ]
must "and the same breakdown"                 [ "$(by "$HO" | grep -c '^scope-generated=3$')" = 1 ]

# ---------------------------------------------------------------- a closed session
"$MJ" session start >/dev/null
printf 'summary\n' | "$MJ" session close >/dev/null
REC="$(grep -l '^session_id: ' .ai/repo/sessions/*.md | head -n 1)"
expect_file "$REC"
must "the session record states it"  [ -n "$(val "$REC" changed_files_excluded)" ]
must "it is not zero here"           [ "$(val "$REC" changed_files_excluded)" -ge 5 ]
must "and the work is still named"   grep -q '^  - lib/work.sh$' "$REC"
# The contract has to have the keys. majordomus.session-records reads the generated allow
# list, so a key added to the schema without regenerating share/allow/session-record.txt
# turns every new record red — which is the failure this assertion exists to catch, and it
# catches it here rather than on the next person's doctor run.
"$MJ" doctor > doctor.out 2>&1 || true
must "the contract has changed_files_excluded" \
  [ "$(grep -c 'changed_files_excluded' doctor.out)" = 0 ]
expect_no_grep 'key\(s\)? the contract does not have' doctor.out

# ---------------------------------------------------------------- the store, by name
# The finding that named this work: a record listing its own sibling records. Those are
# tracked files in a tracked tree, so they do reach the dirty tree — unlike the checkpoints
# above — and the store declaration is what keeps them out. The accounting has to say so,
# because "1 excluded" over a tree whose only extra file is another session record is the
# one case a reader most needs attributed.
git add -A && git commit -qm 'the first record is committed' >/dev/null 2>&1 || true
printf '\n' >> "$REC"
"$MJ" session start --provider-session second >/dev/null
printf 'summary\n' | "$MJ" session close --provider-session second >/dev/null
REC2="$(grep -l '^session_id: ' .ai/repo/sessions/*.md | while IFS= read -r f; do
  [ "$f" = "$REC" ] || printf '%s\n' "$f"; done | head -n 1)"
expect_file "$REC2"
must "the sibling record is not the work"   [ "$(listed "$REC2" | grep -c '\.ai/repo/sessions/')" = 0 ]
must "and the store is named as excluding it" [ "$(by "$REC2" | grep -c '^record-store=')" = 1 ]

# ---------------------------------------------------------------- zero is stated, not omitted
# A record with nothing to exclude must still carry the key. Omitting it would make "nothing
# was excluded" and "this record predates the classifier" the same observation, and the
# validator's whole grandfathering rests on being able to tell them apart.
git add -A && git commit -qm 'settle the tree' >/dev/null 2>&1 || true
printf 'x\n' >> lib/work.sh
printf 'a clean note\n' | "$MJ" checkpoint >/dev/null
CP0="$(find .ai/local/state/checkpoints -name '*.md' -newer "$CP" | head -n 1)"
expect_file "$CP0"
must "a record with nothing excluded says 0" [ "$(val "$CP0" changed_files_excluded)" = 0 ]
must "and omits the empty breakdown"         [ "$(by "$CP0" | grep -c .)" = 0 ]

# ---------------------------------------------------------------- the count follows the tree
# The number is measured, not a constant. Remove one declaration and both the kept list and
# the count must move together in opposite directions.
: > .gitattributes
printf 'x\n' >> site/data/generated/x.json
printf 'x\n' >> site/data/generated/y.json
printf 'another note\n' | "$MJ" checkpoint >/dev/null
CP2="$(find .ai/local/state/checkpoints -name '*.md' -newer "$CP0" | head -n 1)"
expect_file "$CP2"
must "an undeclared generated tree is named work" \
  [ "$(listed "$CP2" | grep -c '^site/data/generated/')" = 2 ]
must "and no longer counted as excluded" \
  [ "$(by "$CP2" | grep -c '^gitattributes-derived=')" = 0 ]

echo "  281 ok"

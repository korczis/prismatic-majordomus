# majordomus-covers: none
# The rule names every ground the code closes a pull request on, and the cases that hold each
# (decision D5 of 2026-10-07). Version 2's closure bullet named ancestry and a no-change merge
# while cleanup also closed on equal patches and on any body's declaration; a rule that names
# fewer grounds than the code acts on describes another program. Read from this repository:
#
#   1. project.integration-follows-the-current-master is version 3, in a file named .v3.md,
#      and no .v2.md is left beside it
#   2. its closure bullet names the four grounds: ancestry, a merge that changes no file,
#      every commit on master as an equal patch, and a declaration by an owner, member or
#      collaborator whose successor git finds in master; it says the forge's word proves
#      nothing, that anyone else's declaration is possible_supersession, and that a partial
#      cross-reference read holds
#   3. its tests name every case from 928 to 939 that this tree holds, by its file name
#   4. ADR 0101, which it implements, records the decisions under their date and is still
#      proposed: acceptance is the owner's act
. "$ROOT/test/lib.sh"
RULES="$ROOT/.ai/repo/rules/project"
RULE="$RULES/integration-follows-the-current-master.v3.md"

# ---------------------------------------------------------------- 1. version 3, alone
[ -f "$RULE" ] || { echo "    there is no ${RULE#"$ROOT"/}"; exit 1; }
grep -q '^version: 3$' "$RULE" || { echo "    the rule's front matter does not say version: 3"; exit 1; }
grep -q '^id: project.integration-follows-the-current-master$' "$RULE" || { echo "    the file does not carry the rule's id"; exit 1; }
for old in "$RULES"/integration-follows-the-current-master.v*.md; do
  [ "$old" = "$RULE" ] || { echo "    another version of the rule is still there: ${old#"$ROOT"/}"; exit 1; }
done

# ---------------------------------------------------------------- 2. the four grounds
# the rule's prose is wrapped: read it as one line
text="$(tr '\n' ' ' < "$RULE" | tr -s ' ')"
names() {   # <fragment> <what it is>
  case "$text" in *"$1"*) ;; *) echo "    the rule does not name $2 ('$1')"; exit 1 ;; esac
}
names 'one of four grounds' "how many grounds a closure has"
names 'the head is an ancestor of master' "ancestry"
names 'the merge changes no file' "a no-change merge"
names 'is on master as an equal patch' "every patch upstream"
names 'by an owner, member or collaborator in a pull request of this repository' "who may declare a successor"
names "git finds that successor's head or merge commit in master" "what makes a declared successor landed"
names "The forge's word that a pull request merged proves nothing here" "that the forge's flag decides nothing"
names '`possible_supersession` and neither holds nor closes' "what an unauthorised declaration is"
names 'whose cross-references were not read whole is held' "that a partial read holds"
names 'releases it when the successor is closed without landing' "when a hold is released"
names 'counts only when its head lives in this repository' "that a fork's head closed unmerged never lands"
names 'the refresh fails and the observation recorded before it is not replaced' "that a failed read is not a hold"
names 'a marker on a quoted line, in a fenced code block or in an HTML comment declares nothing' "what a body states"

# ---------------------------------------------------------------- 3. the cases that hold it
tests="$(grep -m1 '^  tests: ' "$RULE")" || { echo "    the rule lists no tests"; exit 1; }
n=928; held=0
while [ "$n" -le 939 ]; do
  for case_file in "$ROOT/test/cases/${n}"_*.sh; do
    [ -f "$case_file" ] || continue
    held=$((held + 1))
    case "$tests" in
      *"test/cases/${case_file##*/}"*) ;;
      *) echo "    the rule's tests do not name test/cases/${case_file##*/}"; exit 1 ;;
    esac
  done
  n=$((n + 1))
done
[ "$held" -ge 12 ] || { echo "    this tree holds $held of the cases 928-939, not all twelve"; exit 1; }
for n in 937 938 939; do
  case "$tests" in *"test/cases/${n}_"*) ;; *) echo "    the rule's tests name no case $n"; exit 1 ;; esac
done
# a case the rule names is a file here
for named in $(printf '%s' "$tests" | tr ',[]' '\n\n\n' | tr -d ' ' | grep '^test/cases/9[23][0-9]_'); do
  [ -f "$ROOT/$named" ] || { echo "    the rule names $named, which is not in this tree"; exit 1; }
done

# ---------------------------------------------------------------- 4. the decision record
ADR="$(printf '%s\n' "$ROOT"/.ai/repo/adrs/0101-*.md)"
[ -f "$ADR" ] || { echo "    ADR 0101 is not one file: $ADR"; exit 1; }
grep -q '^status: proposed$' "$ADR" || { echo "    ADR 0101 is no longer proposed: $(grep -m1 '^status:' "$ADR")"; exit 1; }
adr="$(tr '\n' ' ' < "$ADR" | tr -s ' ')"
case "$adr" in *"Amended 2026-10-07"*) ;; *) echo "    ADR 0101 does not record the amendment of 2026-10-07"; exit 1 ;; esac
for d in D1 D2 D3 D4 D5; do
  case "$adr" in *"**$d**"*) ;; *) echo "    ADR 0101 does not record decision $d"; exit 1 ;; esac
done
# the narrowing of D3 and the two residuals are in the record itself, not only in the manual
for said in 'Amendment to D3, 2026-10-07' '**R2** Truncation holds' '**R6** `MEMBER`' '**R3**' '**R4**' '**R5**'; do
  case "$adr" in *"$said"*) ;; *) echo "    ADR 0101 does not record '$said'"; exit 1 ;; esac
done
case "$adr" in *possible_supersession*declarations_unread*|*declarations_unread*possible_supersession*) ;;
  *) echo "    ADR 0101 does not name possible_supersession and declarations_unread"; exit 1 ;; esac
echo "    the rule is version 3 and names all four closure grounds and the cases that hold them; ADR 0101 is proposed"

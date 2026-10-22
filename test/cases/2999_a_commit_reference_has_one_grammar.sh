# majordomus-covers: adr knowledge
# majordomus-negative: adr knowledge
# A commit reference has one grammar for every writer and every checker (issue I2600).
#
# `commit:<name>` is 4 to 64 lower-case hexadecimal digits, a full object name or an
# abbreviation of one, and it resolves when git peels it to exactly one commit. Three places
# used to disagree: `adr propose --from commit:<anything>` wrote whatever it was given,
# `knowledge check` compared the reference with the full name git printed and so failed every
# abbreviation as "resolves to nothing", and the executable accepted an abbreviation. An
# adopter wrote 76 records with 8-digit references and the check failed 75 of them.
#
# So: a writer stores the full name and refuses a name that is malformed, ambiguous or
# absent, saying which; and a record that already carries an unambiguous abbreviation passes
# `knowledge check` and resolves in the executable's `knowledge record`, while an ambiguous
# or absent one is refused by both in the same words. The second half drives the Rust
# executable and skips itself when there is neither cargo nor MAJORDOMUS_BIN; the first half
# never skips.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true

ADRS=.ai/repo/adrs
CAND=.ai/repo/knowledge/candidates
LEDGER=.ai/local/state/ledger.jsonl
mkdir -p "$CAND"

FULL="$(git rev-parse HEAD)"
ABBR="$(printf '%s' "$FULL" | cut -c1-8)"
ABSENT=deadbeef00

# Two commits whose names begin with the same four digits, from one process: fifteen hundred
# commits on a branch of their own, written by fast-import from a fixed stream, and the first
# four-digit prefix two of them share. (Sixty-five thousand prefixes and fifteen hundred
# names: a shared one is certain in practice, and the case says so if it is not there.)
awk 'BEGIN { for (i = 1; i <= 1500; i++) { m = "commit " i
  printf "commit refs/heads/many\ncommitter T <t@example.invalid> %d +0000\ndata %d\n%s\n", 1700000000 + i, length(m), m
  if (i == 1) printf "M 100644 inline f\ndata 2\nx\n"
  printf "\n" } }' | git fast-import --quiet
AMBIG="$(git rev-list many | cut -c1-4 | LC_ALL=C sort | uniq -d | head -n 1)"
[ -n "$AMBIG" ] || { echo "    no two of the 1500 commits share a four-digit prefix; the fixture cannot show ambiguity"; exit 1; }
# and the abbreviation of HEAD must not be one of them
[ "$(git rev-parse --verify --quiet "$ABBR^{commit}")" = "$FULL" ] || { echo "    $ABBR does not name HEAD alone in this fixture"; exit 1; }

adr_count() { find "$ADRS" -maxdepth 1 -name '[0-9]*.md' | wc -l | tr -d ' '; }

# ---------------------------------------------------------------- adr propose: the writer
BEFORE="$(adr_count)"
expect_exit 0 "$MJ" adr propose "Abbreviations are written in full" --from "commit:$ABBR"
NEW="$(ls "$ADRS"/[0-9]*-abbreviations-are-written-in-full.md)"
expect_grep "^    - commit:$FULL\$" "$NEW"
expect_no_grep "commit:$ABBR([^0-9a-f]|\$)" "$NEW"
[ "$(adr_count)" = "$((BEFORE + 1))" ] || { echo "    one proposal wrote $(( $(adr_count) - BEFORE )) records"; exit 1; }

# a full name is written as it is
expect_exit 0 "$MJ" adr propose "A full name stays a full name" --from "commit:$FULL"
expect_grep "^    - commit:$FULL\$" "$(ls "$ADRS"/[0-9]*-a-full-name-stays-a-full-name.md)"

# absent, ambiguous, and not a name at all: each refused, each saying which, nothing written
BEFORE="$(adr_count)"
expect_exit 2 "$MJ" adr propose "Names nothing" --from "commit:$ABSENT"
expect_grep "commit:$ABSENT.* names no commit of this history"
expect_exit 2 "$MJ" adr propose "Names two" --from "commit:$AMBIG"
expect_grep "commit:$AMBIG.* is ambiguous: more than one commit"
expect_exit 2 "$MJ" adr propose "Names a moving thing" --from "commit:HEAD"
expect_grep "commit:HEAD.* is not a commit name"
expect_exit 2 "$MJ" adr propose "Names too little" --from "commit:abc"
expect_grep "commit:abc.* is not a commit name"
[ "$(adr_count)" = "$BEFORE" ] || { echo "    a refused proposal wrote a record"; exit 1; }

# `adr check` reads the same grammar without asking git, so a shallow clone judges alike:
# the two records above pass, and a record whose reference is not a name does not
# (a decision is discovered through the index, so each state is added before it is judged)
git add -A "$ADRS" >/dev/null
expect_exit 0 "$MJ" adr check
expect_grep 'examined 2 decision'
sed "s/commit:$FULL/commit:HEAD/" "$NEW" > "$NEW.tmp" && mv "$NEW.tmp" "$NEW"
git add -A "$ADRS" >/dev/null
expect_exit 10 "$MJ" adr check
expect_grep "commit:HEAD.* is not a commit name"
sed "s/commit:HEAD/commit:$ABBR/" "$NEW" > "$NEW.tmp" && mv "$NEW.tmp" "$NEW"
git add -A "$ADRS" >/dev/null
expect_exit 0 "$MJ" adr check

# ---------------------------------------------------------------- knowledge observe: the writer
"$MJ" session start >/dev/null
SID="$(sed -n 's/^session_id: //p' .ai/local/state/session-current.yaml | head -n 1)"
[ -n "$SID" ] || { echo "    no open episode to reference"; exit 1; }
observed() { grep -c '"event":"observation.recorded"' "$LEDGER" 2>/dev/null || true; }

expect_exit 0 "$MJ" knowledge observe --kind friction --subject lib --evidence "commit:$ABBR" "an abbreviation is stored in full"
grep '"event":"observation.recorded"' "$LEDGER" | tail -n 1 | grep -qF "\"evidence\":[\"commit:$FULL\"]" \
  || { echo "    the observation does not carry the full name: $(grep '"event":"observation.recorded"' "$LEDGER" | tail -n 1)"; exit 1; }
expect_exit 2 "$MJ" knowledge observe --kind friction --subject lib --evidence "commit:$AMBIG" "an ambiguous name"
expect_grep "commit:$AMBIG.* is ambiguous"
expect_exit 2 "$MJ" knowledge observe --kind friction --subject lib --evidence "commit:HEAD" "a moving name"
expect_grep "commit:HEAD.* is not a commit name"
[ "$(observed)" = 1 ] || { echo "    a refused observation was written"; exit 1; }
# evidence on the ledger need not resolve here (a commit of another clone, a path since
# removed): a well-formed name git does not hold is kept as given, and the deriver leaves it
# out of the record
expect_exit 0 "$MJ" knowledge observe --kind friction --subject lib --evidence "commit:$ABSENT" "a commit this clone lacks"
grep '"event":"observation.recorded"' "$LEDGER" | tail -n 1 | grep -qF "\"evidence\":[\"commit:$ABSENT\"]" \
  || { echo "    a well-formed name this clone lacks was not kept as given"; exit 1; }

# ---------------------------------------------------------------- knowledge check: the checker
# record ID REFERENCE — a valid candidate naming the episode and one commit reference
record() {
  {
    printf -- '---\nschema: knowledge/v1\nid: %s\nkind: knowledge\nclass: convention\n' "$1"
    printf 'title: "Every candidate is one assertion"\ndescription: "Hand-made for the grammar of a commit reference."\n'
    printf 'status: candidate\nepistemics: decided\ndate: 2026-10-13\ntags:\n  - derived\n  - decision\n'
    printf 'provenance:\n  origin: extracted\n  derived_from:\n    - session:%s\n    - %s\n---\n\n' "$SID" "$2"
    printf '# Every candidate is one assertion\n\nHand-made for the grammar of a commit reference.\n'
  } > "$CAND/$1.md"
}
record names-abbreviation "commit:$ABBR"
record names-full "commit:$FULL"
git add -A .ai/repo/knowledge >/dev/null
expect_exit 0 "$MJ" knowledge check
expect_grep '^knowledge check: 2 record\(s\), 0 failing'

record names-absent "commit:$ABSENT"
record names-ambiguous "commit:$AMBIG"
record names-moving "commit:HEAD"
git add -A .ai/repo/knowledge >/dev/null
expect_exit 10 "$MJ" knowledge check
expect_grep "FAIL +knowledge .*names-absent.*commit:$ABSENT.* names no commit of this history"
expect_grep "FAIL +knowledge .*names-ambiguous.*commit:$AMBIG.* is ambiguous: more than one commit"
expect_grep "FAIL +knowledge .*names-moving.*commit:HEAD.* is not a commit name"
expect_no_grep 'FAIL +knowledge .*names-(abbreviation|full)'
expect_grep '^knowledge check: 5 record\(s\), 3 failing'

# ---------------------------------------------------------------- knowledge derive: the writer of records
# An observation recorded before this rule carries the abbreviation on its ledger line. The
# deriver is what writes the record, so it writes the full name, and it leaves out the name
# this clone does not hold, as it always has.
sed "s/\"commit:$FULL\"/\"commit:$ABBR\"/" "$LEDGER" > "$LEDGER.tmp" && mv "$LEDGER.tmp" "$LEDGER"
grep -qF "\"evidence\":[\"commit:$ABBR\"]" "$LEDGER" || { echo "    the older ledger line was not reproduced"; exit 1; }
"$MJ" knowledge derive --episode "$SID" >/dev/null
DERIVED="$(grep -l 'an abbreviation is stored in full' "$CAND"/"$SID"-*.md | head -n 1)"
[ -n "$DERIVED" ] || { echo "    the observation derived no candidate"; exit 1; }
expect_grep "^    - commit:$FULL\$" "$DERIVED"
LACKING="$(grep -l 'a commit this clone lacks' "$CAND"/"$SID"-*.md | head -n 1)"
[ -n "$LACKING" ] || { echo "    the second observation derived no candidate"; exit 1; }
expect_no_grep '^    - commit:' "$LACKING"
git add -A .ai/repo/knowledge >/dev/null
expect_exit 10 "$MJ" knowledge check
expect_no_grep "FAIL +knowledge .*$SID-"

# ---------------------------------------------------------------- the executable: the other checker
git commit -qm "five candidates" >/dev/null
RB="$(rust_bin)" || rust_bin_exit $?
# verdict ID — the resolution and the reason the executable gives the record's commit reference
verdict() {
  MAJORDOMUS_SHARE="$ROOT/share" "$RB" knowledge record "$1" --format json 2>/dev/null \
    | jq -r '.derived_from[] | select(.reference | startswith("commit:")) | "\(.resolution) \(.target // "-") \(.reason // "")"'
}
command -v jq >/dev/null 2>&1 || skip "no jq to read the executable's answer"
for id in names-abbreviation names-full; do
  v="$(verdict "$id")"
  case "$v" in "external git"*) ;; *) echo "    the executable did not resolve $id as the shell check did: $v"; exit 1 ;; esac
done
v="$(verdict names-absent)"
case "$v" in "missing "*"names no commit of this history"*) ;; *) echo "    names-absent: $v"; exit 1 ;; esac
v="$(verdict names-ambiguous)"
case "$v" in "missing "*"is ambiguous: more than one commit"*) ;; *) echo "    names-ambiguous: $v"; exit 1 ;; esac
v="$(verdict names-moving)"
case "$v" in "missing "*"is not a commit name"*) ;; *) echo "    names-moving: $v"; exit 1 ;; esac
exit 0

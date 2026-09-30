# majordomus-covers: none
# The Overview of the Dashboard Suite (ADR 0088) through the built executable: every card is
# its source capability's answer at the card's own pointer, and nothing else.
#
#   1. `majordomus dashboard overview --format json` answers the four questions in order —
#      healthy, changed, broken, action — and every card names a source capability, an input,
#      an RFC 6901 pointer, a measure and a Cockpit route;
#   2. for EVERY card, naming none: the source capability, run with the card's input
#      (`majordomus run <capability> --input <json>`), read at the card's pointer by the
#      card's measure, is the card's value. A source that did not answer leaves its card
#      `unknown` with no value — never `ok`;
#   3. a question is its worst card, the overview its worst question, and the exit code is
#      the overview's own word: 0 for ok and warn, 10 for fail and unknown;
#   4. refusals: a subcommand the suite does not declare is a usage error (2), and a
#      directory that is no repository is refused (12) with nothing printed as an answer.
#      No source of the Overview refuses on a readable repository, so the other refusal —
#      a source that cannot answer makes its cards `unknown`, never `ok` — is held by the
#      module's own test `an_unanswered_source_is_unknown_with_its_reason`, and step 2
#      asserts it for any card whose source fails here.
#
# The measures are applied here with jq, written a second time on purpose: an oracle that
# called the implementation it judges would agree with any mistake in it.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

# A repository of the layer, from the distribution's own skeleton, committed: the layer's
# discovery contract is the version-control index.
R="$T/repo"; mkdir -p "$R"
cp -R "$ROOT/share/skeleton/ai" "$R/.ai"
( cd "$R" && git init -q . && git add -A >/dev/null 2>&1 \
  && git -c user.email=t@t -c user.name=t commit -qm init >/dev/null 2>&1 ) \
  || { echo "    could not make a fixture repository"; exit 1; }

# the value a card's measure reads at its pointer, RFC 6901 unescaped
READ='def ptr($p): $p | ltrimstr("/") | split("/") | map(gsub("~1"; "/") | gsub("~0"; "~"));
      def measure($a; $p; $m):
        ($a | getpath(ptr($p))) as $v
        | if $m == "exact" then $v
          elif $m == "count" then
            (if $v == null then 0
             elif ($v | type) == "array" or ($v | type) == "object" then ($v | length)
             else $v end)
          else error("a card declares a measure nobody defined: \($m)") end;'

# Ask the overview, then every source; fails with a message naming the card.
check_overview() {
  local o="$1" n i id cap input ptr m want got state
  jq -e '[.questions[].id] == ["healthy","changed","broken","action"]' "$o" >/dev/null \
    || { echo "    the four questions are not healthy, changed, broken, action:"; jq -c '[.questions[].id]' "$o"; return 1; }
  n="$(jq '[.questions[].cards[]] | length' "$o")"
  [ "$n" -gt 0 ] || { echo "    the overview carries no card"; return 1; }
  jq -e '[.questions[].cards[].id] | length == (unique | length)' "$o" >/dev/null \
    || { echo "    two cards share an id"; return 1; }
  i=0
  while [ "$i" -lt "$n" ]; do
    jq -c "[.questions[].cards[]][$i]" "$o" > card.json
    id="$(jq -r .id card.json)"; cap="$(jq -r .source.capability card.json)"
    input="$(jq -c .source.input card.json)"; ptr="$(jq -r .source.pointer card.json)"
    m="$(jq -r .source.measure card.json)"
    case "$ptr" in /*) ;; *) echo "    $id: $ptr is not a JSON pointer"; return 1 ;; esac
    jq -e '.route | startswith("/cockpit")' card.json >/dev/null \
      || { echo "    $id drills into no Cockpit page"; return 1; }
    rc=0; ( cd "$R" && "$RB" run "$cap" --input "$input" --format json --quiet ) > src.json 2>src.err || rc=$?
    state="$(jq -r '.state // "unreadable"' src.json 2>/dev/null || echo unreadable)"
    if [ "$rc" = 0 ] && [ "$state" = succeeded ]; then
      want="$(jq -c --arg p "$ptr" --arg m "$m" "$READ"' measure(.output; $p; $m)' src.json)"
      got="$(jq -c .value card.json)"
      [ "$want" = "$got" ] \
        || { echo "    $id is $got, and $cap at $ptr ($m) is $want: the card computed a value of its own"; return 1; }
    else
      jq -e '.status == "unknown" and .value == null' card.json >/dev/null \
        || { echo "    $id: $cap did not answer ($rc, $state) and the card says $(jq -c '{status,value}' card.json)"; return 1; }
    fi
    i=$((i + 1))
  done
  # a question is its worst card; the overview its worst question
  jq -e 'def r: {"ok":0,"warn":1,"fail":2,"unknown":3}[.];
         all(.questions[]; (.status | r) == ([.cards[].status | r] | max // 0))
         and ((.status | r) == ([.questions[].status | r] | max))' "$o" >/dev/null \
    || { echo "    a question or the overview is not its worst part"; return 1; }
}

# expected exit for an overview document: the overview's own word
exit_for() { case "$(jq -r .status "$1")" in ok|warn) echo 0 ;; *) echo 10 ;; esac; }

# ---------------------------------------------------------------- 1-3. the fixture as it is
rc=0; ( cd "$R" && "$RB" dashboard overview --format json ) > overview.json 2>overview.err || rc=$?
jq -e . overview.json >/dev/null 2>&1 || { echo "    the overview is not JSON ($rc):"; cat overview.err; exit 1; }
[ "$rc" = "$(exit_for overview.json)" ] \
  || { echo "    exit $rc for an overview that says $(jq -r .status overview.json)"; exit 1; }
check_overview overview.json

# the text rendering is the same answer: every card's source is named beside its value
rc=0; ( cd "$R" && "$RB" dashboard overview ) > overview.txt 2>/dev/null || rc=$?
[ "$rc" = "$(exit_for overview.json)" ] || { echo "    the text rendering exits $rc"; exit 1; }
for cap in $(jq -r '[.questions[].cards[].source.capability] | unique[]' overview.json); do
  grep -q "$cap" overview.txt || { echo "    the text rendering does not name $cap"; exit 1; }
done

# ---------------------------------------------------------------- 4. refusals
rc=0; ( cd "$R" && "$RB" dashboard nonesuch ) >/dev/null 2>&1 || rc=$?
[ "$rc" = 2 ] || { echo "    an undeclared dashboard page exited $rc, not 2"; exit 1; }

# no repository: nothing is answered, and nothing is printed as though it were
mkdir -p "$T/norepo"
rc=0; ( cd "$T/norepo" && "$RB" dashboard overview --format json ) > none.json 2>none.err || rc=$?
[ "$rc" = 12 ] || { echo "    an overview outside any repository exited $rc, not 12"; exit 1; }
[ ! -s none.json ] || { echo "    an overview outside any repository printed an answer:"; cat none.json; exit 1; }
grep -q 'no Majordomus repository' none.err || { echo "    the refusal does not say why:"; cat none.err; exit 1; }

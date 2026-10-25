# majordomus-covers: none
# majordomus-timeout: 300
# Work no intent links is classed, end to end, and only the `orphan` class is said to serve no
# intent (ADR 0075, amendment of 2026-10-15; slices 1 and 2 are #1201 and #1211).
#
# One repository declares one intent (created 2026-09-15) over one milestone and one issue,
# and holds four closed session records, each a mutation of a record the tool itself wrote:
#
#   s-empty    started after the intent; no issue, no commit, no changed path  -> empty
#   s-early    started 2026-09-01, before the intent; a commit and a path      -> before_intents
#   s-orphan   started after the intent; a commit and a path no issue owns     -> orphan
#   s-bound    started after the intent; moved the issue that serves it        -> linked
#
# `intent realization --format json` must count orphans 1, empty 1, before_intents 1, class
# each unlinked unit and link the fourth; its text must name each class and say only the
# orphan serves no intent; and the Cockpit's /cockpit/intents, served by `majordomus serve`,
# must show the same three counts and the card that explains them. Nothing is dialed: one
# server on loopback, stopped by the pid this case started.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "no curl"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID MAJORDOMUS_HTTP_HOST
S="$(mktemp -d "${TMPDIR:-/tmp}/mj3057.XXXXXX")"
SRV=""; trap 'rm -rf "$S"; [ -n "$SRV" ] && kill "$SRV" 2>/dev/null; true' EXIT

same() { [ "$2" = "$3" ] || { printf '    %s: expected %s, got %s\n' "$1" "$2" "$3"; exit 1; }; }

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A >/dev/null 2>&1; git commit -qm base >/dev/null 2>&1 || true

# ---------------------------------------------------------------- the plan and the intent
pj_init
mkdir -p .ai/repo/project/intents
pj_milestone probe
pj_issue I0001 probe
cat >> .ai/repo/project/issues/I0001.yaml <<'Y'
serves:
  - probe#a
Y
cat > .ai/repo/project/intents/probe.yaml <<'Y'
id: probe
title: The probe behaviour is true for the people who rely on it
statement: "Behaviour a exists, and the recorded run of its case says so now."
invariants:
  - No stage of this intent is written anywhere
milestones:
  - probe
satisfaction:
  - id: a
    criterion: Behaviour a exists
    evidence: test
    ref: test/cases/01_a.sh
created_at: 2026-09-15
Y
mkdir -p test/cases && printf 'true\n' > test/cases/01_a.sh
expect_exit 0 "$RB" intent validate

# ---------------------------------------------------------------- four session records
# Written by the tool itself, then mutated: a fixture that invents the shape it is testing
# tests the fixture.
STORE=.ai/repo/sessions
"$MJ" session start --owner tester >/dev/null
"$MJ" session close >/dev/null
good="$(find "$STORE" -maxdepth 1 -name '2*.md' | head -n 1)"
[ -n "$good" ] || { echo "    the close wrote no session record to build the fixtures from"; exit 1; }
cp "$good" "$S/template.md"
rm -f "$good"

# record <file> <session id> <started_at> <title> <commits> <changed files> <issues>
# Each list is space-separated, or empty for `[]`. Every other line is the template's.
record() {
  awk -v sid="$2" -v at="$3" -v title="$4" -v commits="$5" -v files="$6" -v issues="$7" '
    function list(key, items,   n, a, i) {
      n = split(items, a, " ")
      if (n == 0) { print key ": []"; return }
      print key ":"
      for (i = 1; i <= n; i++) print "  - \"" a[i] "\""
    }
    # a list the template wrote, as a block or inline, is dropped and written anew
    skipping && /^  - / { next }
    { skipping = 0 }
    /^session_id:/    { print "session_id: " sid; next }
    /^started_at:/    { print "started_at: " at; next }
    /^title:/         { print "title: \"" title "\""; next }
    /^commits:/       { list("commits", commits); skipping = 1; next }
    /^changed_files:/ { list("changed_files", files); skipping = 1; next }
    /^issues:/        { list("issues", issues); skipping = 1; next }
    { print }
  ' "$S/template.md" > "$STORE/$1.md"
  # a template that lacks a field still gets it: the class is read from these four
  for k in commits changed_files issues; do
    grep -q "^$k:" "$STORE/$1.md" || { echo "    the template record has no $k: field"; cat "$S/template.md"; exit 1; }
  done
}
record 20261001T000000Z--s-empty  s-empty  2026-10-01T00:00:00Z "Recovered episode s-empty" "" "" ""
record 20260901T000000Z--s-early  s-early  2026-09-01T00:00:00Z "Session s-early"  "aaa1111" "docs/early.md" ""
record 20261002T000000Z--s-orphan s-orphan 2026-10-02T00:00:00Z "Session s-orphan" "bbb2222" "docs/orphan.md" ""
record 20261003T000000Z--s-bound  s-bound  2026-10-03T00:00:00Z "Session s-bound"  "ccc3333" "src/I0001/x" "I0001"
git add -A >/dev/null && git commit -qm "the plan, the intent and four session records" >/dev/null

# ---------------------------------------------------------------- the join, as JSON
"$RB" intent realization --format json > "$S/real.json" 2> "$S/real.err" \
  || { echo "    intent realization --format json failed"; cat "$S/real.err"; exit 1; }
J="$S/real.json"
same "the units of work read" "s-bound,s-early,s-empty,s-orphan" \
  "$(jq -r '[.work[].work.id] | sort | join(",")' "$J")"
same "orphans" 1 "$(jq -r '.orphans' "$J")"
same "empty" 1 "$(jq -r '.empty' "$J")"
same "before_intents" 1 "$(jq -r '.before_intents' "$J")"
class() { jq -r --arg id "$1" '.work[] | select(.work.id == $id) | .class // "none"' "$J"; }
same "s-empty's class" empty "$(class s-empty)"
same "s-early's class" before_intents "$(class s-early)"
same "s-orphan's class" orphan "$(class s-orphan)"
same "a linked unit has no class" none "$(class s-bound)"
same "the bound record realises the intent" "probe" \
  "$(jq -r '.work[] | select(.work.id == "s-bound") | [.links[].intent] | unique | join(",")' "$J")"
same "the bound record's link is observed from the issue it moved" "observed" \
  "$(jq -r '.work[] | select(.work.id == "s-bound") | .links[0].provenance' "$J")"
# no class is a link: the three unlinked units each still say what is missing
same "every unlinked unit says what is missing" 3 \
  "$(jq -r '[.work[] | select(.links == [] and (.unlinked // "") != "")] | length' "$J")"
# a closed record is never warned about, whatever its class
same "no closed record is a finding" 0 \
  "$(jq -r '[.findings[] | select(.code == "work_serves_no_intent")] | length' "$J")"

# ---------------------------------------------------------------- the join, as text
expect_exit 0 "$RB" intent realization
expect_grep 'session_record s-empty  .*unlinked \(empty\): '
expect_grep 'session_record s-early  .*unlinked \(before_intents\): '
expect_grep 'session_record s-orphan  .*unlinked \(orphan\): '
expect_no_grep 'session_record s-bound  .*unlinked'
expect_grep '1 intent\(s\), 4 unit\(s\) of work, 1 serving no intent \(1 empty records, 1 before the first intent\)'
same "only the orphan is said to serve no intent" 1 \
  "$(printf '%s\n' "$LAST_OUT" | grep -c 'serving no intent')"

# ---------------------------------------------------------------- the Cockpit
serve_up "$S/out.txt" "$S/err.txt" || exit 1
code="$(curl -s -m 60 -o "$S/page.html" -w '%{http_code}' "$U/cockpit/intents")"
same "/cockpit/intents answers" 200 "$code"
stat_of() {
  grep -oE "mj-stat-value\">[^<]*</span><span class=\"mj-stat-label\">$1<" "$S/page.html" \
    | head -1 | sed -E 's/mj-stat-value">([^<]*)<.*/\1/'
}
same "the orphans statistic" 1 "$(stat_of 'units of work serving no intent')"
same "the empty statistic" 1 "$(stat_of 'empty session records, not orphan work')"
same "the before_intents statistic" 1 \
  "$(stat_of 'units started before the first intent, not orphan work')"
expect_grep 'Work no intent links' "$S/page.html"
expect_grep '1 orphan: work done' "$S/page.html"
expect_grep '1 empty: session records' "$S/page.html"
expect_grep '1 before the first intent: finished work' "$S/page.html"
# the page and the command line read one capability, so they agree
code="$(curl -s -m 60 -o "$S/http.json" -w '%{http_code}' "$U/api/v1/intents/realization")"
same "/api/v1/intents/realization answers" 200 "$code"
same "HTTP and the command line count alike" \
  "$(jq -c '[.orphans, .empty, .before_intents]' "$J")" \
  "$(jq -c '[.orphans, .empty, .before_intents]' "$S/http.json")"
serve_down

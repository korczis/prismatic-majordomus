# majordomus-covers: none
# Every judgement this subsystem makes must be able to say where it came from.
#
# The failure being corrected is not that continuity was wrong once. It is that it was
# confidently wrong for six days and nothing could be asked why. Between 2026-09-05 and
# 2026-09-11 `continuity.state` selected a handover, labelled it `advanced` — a true
# statement about git topology that says nothing about age — and every surface presented
# its `Next Action` as the thing to do. It was finished work. A worker who could have asked
# *why is this the record you are showing me* would have been told "tier 0, the newest of
# eighteen, asserted six days ago" and would have stopped at the third clause.
#
# `continuity.explain` is that question, following the idiom `environment.explain`,
# `design.explain` and `majordomus context explain` already set: an answer per question,
# with the evidence, and with every candidate that lost and the reason it lost.
#
# What this case holds it to is the standard in the mandate: an explanation that is merely
# plausible is worse than none. So the assertions here are almost all about *structure* —
# no answer without a reason, no answer without something a reader can go and open, and an
# unknown that names its own cause rather than an empty string dressed as an answer.
#
# The structural half never skips. The behavioural half skips itself when there is neither
# cargo nor a MAJORDOMUS_BIN to drive, as every other Rust case does.
. "$ROOT/test/lib.sh"

SRC="$ROOT/apps/majordomus-cli/src/capability/builtin/continuity.rs"

# ---------------------------------------------------------------- the declaration
# The capability exists in the one place it is allowed to exist. A transport registry, an
# OpenAPI document or a documentation table that grew this operation without the
# declaration growing it would be the drift ADR 0004 refuses, so the declaration is what is
# read here and the projections are checked against it further down.
expect_grep 'id: "continuity.explain"' "$SRC"
expect_grep 'majordomus_continuity_explain' "$SRC"
expect_grep '/api/v1/continuity/explain' "$SRC"

# No command line. Everything under .ai/local/ names this machine — a worktree path is a
# fact about a disk and not about the repository (ADR 0014) — and a command line is how a
# value reaches a script, a log and eventually a commit. `continuity.state` has none for
# the same reason; a future edit that gives this one a CLI has to come past this line.
awk '/id: "continuity.explain"/,/handler: explain/' "$SRC" | grep -q 'cli: None' || {
  echo "    continuity.explain declares a command line; the local half is served, never piped"; exit 1; }

# ---------------------------------------------------------------- one source for the rule
# The thresholds are the policy's. A literal 720 or 2880 in the provenance code would be
# the second copy of a number `session.freshness` exists in one place to prevent, and the
# answer would then cite a policy key while having judged against something else — which is
# precisely the shape of confident wrongness this whole case is about.
# Two regions are skipped by name, and each for a reason rather than for convenience.
# `span` renders whole minutes for a person and switches from hours to days at 2880, which
# is two days because that is where days start reading better than hours; it is a
# coincidence, not a copy, that the policy currently calls a record stale at the same span.
# The test module constructs thresholds explicitly, which is the only way to assert the
# stale boundary without waiting two days for the clock. Everything else — and in
# particular every line that answers a question — is scanned.
hits="$(awk '
  /^pub fn span\(/          { skip = 1 }
  skip && /^}/              { skip = 0; next }
  skip                      { next }
  /^mod tests \{|^#\[cfg\(test\)\]/ { exit }
  $0 ~ /(^|[^0-9])(720|2880)([^0-9]|$)/ { printf "%s: %s\n", FNR, $0 }
' "$SRC")"
[ -z "$hits" ] || {
  echo "    a freshness threshold is written into continuity.rs as a literal:"
  printf '%s\n' "$hits" | sed 's/^/    | /'; exit 1; }

# The resolution rule must remain single. A second function that re-derives which record
# wins would be a second account of one rule and would drift; the trace is produced by the
# selection itself.
n="$(grep -c 'fn resolve(' "$SRC")"
[ "$n" = 1 ] || { printf '    %s definitions of the resolution rule; there must be exactly one\n' "$n"; exit 1; }

# ---------------------------------------------------------------- behaviour
RB="$(rust_bin)" || rust_bin_exit $?
# The case runs in a disposable repository that has no share directory of its own, so the
# executable is pointed at the source tree's. Without it the binary refuses before it reads
# anything and every assertion below passes vacuously on empty output — which is how this
# was first written, and it "passed" until the output was checked for being non-empty.
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
git add -A && git commit -qm base

run_explain() { "$RB" run continuity.explain "$@" 2>/dev/null | sed -n '/^{/,$p'; }

# --- a checkout with no episode answers every question anyway
out="$(run_explain)"
[ -n "$out" ] || { echo "    continuity.explain produced nothing at all"; exit 1; }

printf '%s' "$out" > explain.json
python3 - <<'PY' || exit 1
import json, sys
d = json.load(open('explain.json'))

if d.get('schema') != 'majordomus/session-provenance/v1':
    print('    the answer declares no schema, or the wrong one:', d.get('schema')); sys.exit(1)

want = {
    'episode.current', 'episode.close', 'episode.task', 'episode.prompts',
    'episode.recovery', 'handover.selected', 'handover.freshness',
    'checkpoint.selected', 'checkpoint.freshness',
}
got = {a['question'] for a in d['answers']}
missing = want - got
if missing:
    print('    questions the mandate asks and this cannot answer:', sorted(missing)); sys.exit(1)

for a in d['answers']:
    q = a['question']
    # The invariant. A verdict a reader cannot check is the thing being fixed, so a reason
    # is mandatory whether the answer is known or not.
    if not a.get('reason'):
        print(f'    {q} gives a verdict with no reason'); sys.exit(1)
    # And a sentence with nothing behind it is an assertion. An absence counts — it is
    # recorded as evidence of kind `absent`, naming where it was looked for — but an empty
    # list does not.
    if not a.get('evidence'):
        print(f'    {q} cites nothing a reader could open'); sys.exit(1)
    for e in a['evidence']:
        if not e.get('locator'):
            print(f'    {q} has evidence that names no location'); sys.exit(1)
        if e['kind'] not in ('file', 'symlink', 'ledger_line', 'policy_key', 'absent'):
            print(f'    {q} cites evidence of an undeclared kind: {e["kind"]}'); sys.exit(1)
    # Absence is an answer; invention is not. An unknown answers nothing at all, rather
    # than offering a likely-looking string with `known: false` beside it.
    if not a['known'] and a.get('answer'):
        print(f'    {q} is not known and yet answers "{a["answer"]}"'); sys.exit(1)
    for c in a.get('considered', []):
        if not c.get('reason'):
            print(f'    {q} rejected {c["path"]} without saying why'); sys.exit(1)
        if c['outcome'] not in ('selected', 'superseded', 'rejected'):
            print(f'    {q} gave {c["path"]} an undeclared standing: {c["outcome"]}'); sys.exit(1)

# A checkout with no open episode is an ordinary state, not a failure, and the answer says
# what it looked for rather than shrugging.
cur = next(a for a in d['answers'] if a['question'] == 'episode.current')
if cur['known']:
    print('    a checkout with no pointer reported a current episode'); sys.exit(1)
if 'session-current.yaml' not in ' '.join(e['locator'] for e in cur['evidence']):
    print('    it did not say where it looked for the pointer'); sys.exit(1)
PY

# --- a question that does not exist is refused, and the refusal lists what does
"$RB" run continuity.explain --input '{"question":"nothing.like.this"}' >/dev/null 2>err.txt && {
  echo "    a question that does not exist was answered anyway"; exit 1; }
grep -q 'nothing.like.this' err.txt || { echo "    the refusal did not name the question asked"; exit 1; }

# --- an episode that does not exist is refused rather than quietly answered about another
"$RB" run continuity.explain --input '{"episode":"s-19700101000000-0000"}' >/dev/null 2>err2.txt && {
  echo "    an episode that does not exist was explained anyway"; exit 1; }
grep -q 's-19700101000000-0000' err2.txt || { echo "    the refusal did not name the episode asked for"; exit 1; }

# ---------------------------------------------------------------- the reason it exists
# The 2026-09-05 record, to the day. A handover selected by the rule and past the stale
# threshold must be explainable as both at once: `advanced` against git, `stale` against the
# clock. Collapsing that pair into "trustworthy" is what carried a finished instruction
# into every new episode for six days.
mkdir -p .ai/local/state/handovers
head="$(git rev-parse HEAD)"
wt="$(pwd -P)"
br="$(git rev-parse --abbrev-ref HEAD)"
cat > .ai/local/state/handovers/20260905T035003Z--stale.md <<EOF
---
schema_version: 1
created_at: 2026-09-05T03:50:02Z
head: $head
branch: $br
worktree: $wt
working_tree: clean
---

# Next Action

Finish the thing that was already finished.
EOF

run_explain > explain2.json
python3 - <<'PY' || exit 1
import json, sys
d = json.load(open('explain2.json'))
by = {a['question']: a for a in d['answers']}

sel = by['handover.selected']
if not sel['known'] or 'stale.md' not in sel['answer']:
    print('    the seeded handover was not selected:', sel); sys.exit(1)
# The tie-break and the tier are the workings, and they must be in the sentence.
if 'tier 0' not in sel['reason']:
    print('    the selection did not say which tier matched:', sel['reason']); sys.exit(1)

fr = by['handover.freshness']
# Either the policy declares the thresholds and the record is judged stale, or it does not
# and the answer is `unknown` naming the missing key. Both are correct; a verdict of
# `fresh` for a record from 2026-09-05, or a silent default, is not.
if fr['known']:
    if not fr['answer'].startswith('stale'):
        print('    a record from 2026-09-05 was judged', fr['answer']); sys.exit(1)
    keys = ' '.join(e['locator'] for e in fr['evidence'])
    if 'session.freshness.stale_minutes' not in keys:
        print('    the verdict did not name the threshold it crossed:', keys); sys.exit(1)
    if 'policy.yaml' not in keys:
        print('    the threshold was not traced to the file that declares it:', keys); sys.exit(1)
else:
    if 'session.freshness' not in fr['reason']:
        print('    an unjudgeable record did not name the key that is missing:', fr['reason']); sys.exit(1)
PY

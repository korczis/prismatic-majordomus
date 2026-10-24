# majordomus-timeout: 300
# majordomus-covers: none
# claims: none
# Proceed.step answers one idempotent work order (I4005, ADR 0177).
#
# proceed.step composes what exists — proceed.plan's claim-aware decision, the intent's
# realisation, models.route — into one work order: `do` (devtask.implement for the issue, under
# devtask.implement's own key), `fan_out`, `report` and `next_due_at`. Proved through the built
# executable, in a repository that is not this one:
#
#   1. repeating the step at the same state answers the same order, byte for byte, with the
#      other free parallel-safe issue fanned out beside the one ordered; a later
#      moment moves only next_due_at, never the key; a changed state is another order;
#   2. an intent orders only the issues serving its unmet criteria, and once its realisation is
#      met (a clean recorded pass of its criterion) the step is done and schedules nothing;
#   3. /proceed is generated from the declaration for every provider that reads skills, and a
#      hand edit of either fails `generate editors --check`; this repository holds no second
#      body for it;
#   4. the stack is extended, not duplicated: one `proceed` module holds proceed.plan and
#      proceed.step, the step's decision is proceed.plan's own, and nothing in the fixture is
#      written by a step.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
W="$(mktemp -d "${TMPDIR:-/tmp}/mj4005.XXXXXX")"; trap 'rm -rf "$W"' EXIT

fail=0
note() { printf '    %s\n' "$1"; fail=1; }
commit() { git add -A >/dev/null && git commit -qm "$1" >/dev/null; }
step() { rc=0; "$RB" proceed step "$@" --format json > "$W/out" 2>"$W/err" || rc=$?; }
q() { jq -r "$1" "$W/out"; }

"$MJ" init >/dev/null; "$MJ" update >/dev/null
"$MJ" capture install >/dev/null

# ---------------------------------------------------------------- the fixture
# I0001 serves nothing; I0002 serves the intent's one criterion. The plan hands out I0001
# first, so an order for the intent that names I0002 can only have come from the intent.
pj_init
pj_milestone probe
pj_issue I0001 probe
pj_issue I0002 probe
printf 'serves:\n  - probe#a\n' >> .ai/repo/project/issues/I0002.yaml
mkdir -p .ai/repo/project/intents test/cases lib
printf 'grep -qx ok lib/a\n' > test/cases/01_a.sh
echo ok > lib/a
cat > .ai/repo/project/intents/probe.yaml <<'Y'
id: probe
title: Behaviour a is true for the people who rely on it
statement: "Behaviour a exists, and the recorded run of its case says so at the checkout."
invariants:
  - No stage of this intent is written anywhere
milestones:
  - probe
satisfaction:
  - id: a
    criterion: Behaviour a exists
    evidence: test
    ref: test/cases/01_a.sh
Y
commit "declare the plan and the intent"

# the skeleton declares an enabled mesh, and no server runs here: the decision is provisional,
# so the step orders nothing, waits and says why (exit 10)
step --at 2026-10-23T10:00:00Z
[ "$rc" = 10 ] && [ "$(q .outcome)" = wait ] && [ "$(q .do)" = null ] && q .reason | grep -q 'provisional' \
  || { note "a step over an unread declared mesh did not wait (exit $rc):"; cat "$W/out"; }
# the rest is about the order, not the mesh: this fixture declares none
mesh_decl=.ai/repo/mesh/mesh.yaml
if [ -f "$mesh_decl" ]; then
  sed 's/^enabled: true$/enabled: false/' "$mesh_decl" > "$W/mesh.yaml" && cp "$W/mesh.yaml" "$mesh_decl"
  commit "no mesh here"
fi

AT=2026-10-23T10:00:00Z

# ---------------------------------------------------------------- 1. the same state, the same order
step --at "$AT"; cp "$W/out" "$W/first.json"
[ "$rc" = 0 ] || { echo "    a step that orders work must exit 0, got $rc:"; cat "$W/out" "$W/err"; exit 1; }
step --at "$AT"
cmp -s "$W/out" "$W/first.json" || { note "the same state at the same moment answered another order:"; diff "$W/first.json" "$W/out" | head -20; }
jq -e '.outcome == "implement" and (.done | not)
  and .do.capability == "devtask.implement"
  and .do.input == {"issue": "I0001", "idempotency_key": "implement-I0001"}
  and .do.issue == "I0001" and .do.profile == "implementation"
  and ([.fan_out[] | [.capability, .input.issue, .input.idempotency_key]]
       == [["devtask.implement", "I0002", "implement-I0002"]])
  and .report.claim == "proceed" and (.report.intent | startswith("I0001: ") and contains("and 1 beside it"))
  and .report.scope == ["src/I0001", "src/I0002"]
  and .next_due_at == "2026-10-23T10:05:00Z" and .at == "2026-10-23T10:00:00Z"
  and (.key | test("^step-[0-9a-f]{16}$"))
  and .intent == null' "$W/first.json" >/dev/null \
  || { note "the whole plan's order is not devtask.implement for plan.next's issue, with the other free issue beside it, due in five minutes:"; cat "$W/first.json"; }
key="$(jq -r .key "$W/first.json")"
# the same capability over MCP, as a client calls it, answers the same order
{ printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case4005","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_proceed_step","arguments":{"at":"%s","local":true}}}\n' "$AT"
} | "$RB" mcp 2>"$W/mcp.err" | tail -n 1 | jq '.result.structuredContent' > "$W/mcp.json" \
  || note "proceed.step could not be asked over MCP"
[ "$(jq -r .key "$W/mcp.json")" = "$key" ] && jq -e '.do.input.issue == "I0001"' "$W/mcp.json" >/dev/null \
  || { note "the MCP tool majordomus_proceed_step answered another order:"; cat "$W/mcp.json"; }
step --at 2026-10-23T10:04:59Z
[ "$(q .key)" = "$key" ] && [ "$(q .next_due_at)" = 2026-10-23T10:05:00Z ] \
  || note "a moment inside the same interval answered another order: $(q .key) $(q .next_due_at)"
step --at 2026-10-23T10:07:00Z
[ "$(q .key)" = "$key" ] && [ "$(q .next_due_at)" = 2026-10-23T10:10:00Z ] \
  || note "a later interval must keep the key and move only next_due_at: $(q .key) $(q .next_due_at)"
step; k1="$(q .key)"; step; k2="$(q .key)"
[ "$k1" = "$key" ] && [ "$k2" = "$key" ] || note "the clock's own moment changed the key: $k1 $k2, not $key"
step --at noon
[ "$rc" != 0 ] && grep -q 'RFC 3339' "$W/err" || note "a moment that is not RFC 3339 was not refused by name (exit $rc)"

# a changed state is another order: an issue of a higher priority comes first
pj_issue I0000 probe
sed -i.bak 's/^priority: p1$/priority: p0/' .ai/repo/project/issues/I0000.yaml && rm -f .ai/repo/project/issues/I0000.yaml.bak
commit "a p0 issue"
step --at "$AT"
[ "$(q .do.issue)" = I0000 ] && [ "$(q .key)" != "$key" ] \
  || note "a changed plan answered the old order: $(q .do.issue) $(q .key)"
git rm -q .ai/repo/project/issues/I0000.yaml && commit "the p0 issue is withdrawn"
step --at "$AT"
cmp -s "$W/out" "$W/first.json" || note "the state restored did not answer the first order again"

# ---------------------------------------------------------------- 2. an intent, until it is met
step --intent probe --at "$AT"
jq -e '.outcome == "implement" and .do.issue == "I0002"
  and .do.input == {"issue": "I0002", "idempotency_key": "implement-I0002"}
  and .intent.intent == "probe" and .intent.unmet == ["a"] and .intent.serving == ["I0002"]
  and .intent.met == 0 and .intent.criteria == 1 and .next_due_at != null
  and .fan_out == []' "$W/out" >/dev/null \
  || { note "the intent's order is not the issue serving its unmet criterion:"; cat "$W/out"; }
# a clean, recorded pass of the criterion's case: the runner's own report, stamped and recorded
bash test/cases/01_a.sh || { echo "    the fixture's case does not pass"; exit 1; }
printf '01_a\tok\t0\tserial\n' > "$W/report.tsv"
"$RB" evidence stamp --producer suite --report "$W/report.tsv" --out "$W/report.provenance.json" >/dev/null
"$RB" evidence record --suite "$W/report.tsv" --provenance "suite=$W/report.provenance.json" >/dev/null
commit "record 01_a"
[ -z "$("$RB" intent realization --intent probe --format json | jq -r '[.intents[0].unmet[].id] | join(",")')" ] \
  || { echo "    the fixture's criterion is not met after a clean recorded pass"; exit 1; }
step --intent probe --at "$AT"; cp "$W/out" "$W/done.json"
[ "$rc" = 0 ] || note "a done order must exit 0, got $rc"
jq -e '.outcome == "done" and .done and .do == null and .fan_out == [] and .next_due_at == null
  and (.reason | startswith("the intent probe is"))
  and .report.scope == [] and .report.intent == .reason
  and .intent.unmet == [] and .intent.met == 1' "$W/done.json" >/dev/null \
  || { note "a met intent did not answer done with nothing ordered and nothing scheduled:"; cat "$W/done.json"; }
step --intent probe --at 2026-10-24T10:00:00Z
[ "$(q .key)" = "$(jq -r .key "$W/done.json")" ] && [ "$(q .next_due_at)" = null ] \
  || note "a done order is not the same order a day later"
step --intent no-such-intent --at "$AT"
[ "$rc" != 0 ] || note "an intent that does not exist was answered"
"$RB" proceed step --intent probe --at "$AT" > "$W/text" 2>&1 || true
grep -q '^next      nothing scheduled$' "$W/text" || { note "the text form does not say nothing is scheduled:"; cat "$W/text"; }

# ---------------------------------------------------------------- 3. /proceed, generated for every provider
"$RB" generate editors >"$W/gen.out" 2>"$W/gen.err" || { note "generate editors failed:"; tail -5 "$W/gen.err"; }
for skill in .claude/skills/proceed/SKILL.md .agents/skills/proceed/SKILL.md; do
  [ -f "$skill" ] || { note "generate editors wrote no $skill"; continue; }
  grep -qx 'name: proceed' "$skill" || note "$skill is not named proceed"
  grep -q 'GENERATED FILE' "$skill" || note "$skill is not generated"
  grep -q 'Source: apps/majordomus-cli/src/capability/builtin/proceed.rs' "$skill" \
    || note "$skill is not generated from proceed.step's declaration"
  grep -q 'capability `proceed.step`' "$skill" || note "$skill does not name proceed.step"
  grep -q 'the tool `majordomus_proceed_step`' "$skill" || note "$skill does not name the MCP tool"
  grep -q '`majordomus proceed step`' "$skill" || note "$skill does not name the command line"
  grep -q 'call it without `intent`' "$skill" || note "$skill asks for an intent the step does not need"
  grep -q 'step again at `next_due_at`' "$skill" || note "$skill does not carry the declared follow"
  ! grep -q '^# Procedure' "$skill" || note "$skill carries a procedure of its own"
  cp "$skill" "$W/orig"
  printf '\nAlso take the next issue by hand.\n' >> "$skill"
  st=0; "$RB" generate editors --check >"$W/check.out" 2>&1 || st=$?
  [ "$st" = 10 ] || note "generate --check exits $st on a hand edit of $skill, not 10"
  grep -q "$skill" "$W/check.out" || note "generate --check does not name $skill"
  "$RB" generate editors >/dev/null 2>&1 || note "generate did not restore $skill"
  cmp -s "$skill" "$W/orig" || note "$skill was not rewritten to what the declaration says"
done
grep -q '^argument-hint: "<intent>"$' .claude/skills/proceed/SKILL.md || note "Claude Code is not told the text is the intent"
! grep -q 'argument-hint\|ARGUMENTS' .agents/skills/proceed/SKILL.md || note "a key only Claude Code reads reached .agents/"
# this repository: the committed /proceed is the declaration's, and no second body exists
for skill in .claude/skills/proceed/SKILL.md .agents/skills/proceed/SKILL.md; do
  grep -q 'Source: apps/majordomus-cli/src/capability/builtin/proceed.rs' "$ROOT/$skill" \
    || note "the committed $skill is not generated from proceed.step's declaration"
done
[ ! -e "$ROOT/.ai/repo/skills/proceed" ] || note "a hand-written proceed skill still stands beside the declaration"

# ---------------------------------------------------------------- 4. extended, not duplicated
commit "the generated skills"
before="$(git status --porcelain)"
"$RB" capabilities list --format json > "$W/caps.json" 2>/dev/null || note "capabilities list failed"
jq -e '[.. | objects | select(has("id") and (.id | type == "string") and (.id | startswith("proceed.")) and has("module")) | .id] | unique == ["proceed.plan", "proceed.step"]' "$W/caps.json" >/dev/null \
  || note "the proceed module is not exactly proceed.plan and proceed.step: $(jq -c '[.. | objects | select(has("id") and (.id|type=="string") and (.id|startswith("proceed."))) | .id] | unique' "$W/caps.json")"
"$RB" capabilities describe proceed.step --format json > "$W/step.json" 2>/dev/null || note "capabilities describe proceed.step failed"
jq -e '.. | objects | select(has("skill")) | .skill | (.alias == "proceed" and .argument == "intent" and (.follow | type == "string"))' "$W/step.json" >/dev/null \
  || note "the descriptor of proceed.step does not carry its skill"
rc=0; "$RB" proceed plan --format json > "$W/plan.json" 2>/dev/null || rc=$?
step --at "$AT"
[ "$(jq -r .selected.issue "$W/plan.json")" = "$(q .do.issue)" ] \
  || note "the step ordered $(q .do.issue), and proceed.plan selects $(jq -r .selected.issue "$W/plan.json")"
[ "$(jq -c .sources "$W/plan.json")" = "$(q '.sources | tojson')" ] \
  || note "the step's sources are not proceed.plan's"
[ "$(git status --porcelain)" = "$before" ] || { note "a step wrote the repository:"; git status --porcelain; }

[ "$fail" = 0 ] || exit 1
echo "    proceed.step: the same state answers the same order and key, an intent is done once met, /proceed is generated for every provider and checked, and the step is proceed.plan's decision"

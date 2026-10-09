# majordomus-covers: finish check
# majordomus-negative: finish
# claims: finish-reads-what-its-work-serves
# A finish reads what its work serves, and writes none of it (ADR 0115).
#
# A task that named the issue it executes could be finished `completed` while the criterion
# that issue serves had never been run, and while a guard of the intent it serves was
# failing: the completion report asked about gates, commits and handovers, and nothing about
# the intent the work was for (case 895 proves the other half, that a finished task
# satisfies no intent). Two questions of share/completion.yaml now ask it, `criteria-served`
# and `guards-hold`, answered from `intents.binding` for what the active task names, and the
# policy key `intent.completion` decides whether they are asked, reported or held.
#
# The fixture is the one case 880 builds — a disposable repository with a CI model, the
# planner and the dispatcher CI runs, a remote, and `completed_means_complete: true` — with a
# plan on top of it: one intent with three criteria and a guard, and two issues.
#
#   required, the issue's criterion never run    criteria-served is queued; `completed` is
#                                                refused (10) naming that criterion and no
#                                                other, with the commands that settle it, and
#                                                no finish record is written
#   the guard never run                          guards-hold passes: a guard nobody ran is not
#                                                judged
#   the guard's case fails                       guards-hold fails; `completed` is refused
#                                                naming the guard
#   the guard's case passes again                guards-hold passes
#   advisory                                     the same verdict is reached, reported by
#                                                `check` and withheld: exempt, carrying what
#                                                it would have been
#   the criterion's case recorded on the         criteria-served passes and `completed` is
#   committed tree, with its stamp               accepted, complete
#   another issue, its criterion never run       `partial` is not refused
#   a task that is exempt, names nothing, or     held to no criterion; only the task naming
#   names an intent and no issue                 an intent is asked about its guards
#
# and that the finish only reads: every file under .ai/repo/project/intents/ is byte for byte
# what was declared after the refusals and after the acceptance, and what `intent show`
# answers is the same before and after a refused finish.
. "$ROOT/test/lib.sh"
BIN="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_BIN="$BIN" MAJORDOMUS_SHARE="$ROOT/share"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
# the runner's report, its stamp and every kept copy live outside the repository: a file
# written inside it is a pending change, and a run recorded over a dirty tree is not current
# evidence of anything
S="$(mktemp -d "${TMPDIR:-/tmp}/mj966.XXXXXX")"; trap 'rm -rf "$S"' EXIT
git config user.email a@b.c; git config user.name a

same() { [ "$2" = "$3" ] || { printf '    %s: expected %s, got %s\n' "$1" "$2" "$3"; exit 1; }; }
commit() { git add -A >/dev/null && git commit -qm "$1" >/dev/null; }

"$MJ" init >/dev/null
# the two bits this case is about: `completed` is earned, and what the work serves is held
completion_mode() { # <off|advisory|required>
  sed -i.bak "s/^  completion: [a-z]*\$/  completion: $1/" .ai/repo/policy.yaml && rm .ai/repo/policy.yaml.bak
  grep -q "^  completion: $1\$" .ai/repo/policy.yaml || { echo "    the policy carries no intent.completion key to set to $1"; exit 1; }
  "$MJ" update >/dev/null
}
sed -i.bak 's/^  completed_means_complete: false$/  completed_means_complete: true/' .ai/repo/policy.yaml && rm .ai/repo/policy.yaml.bak
grep -q '^  completed_means_complete: true$' .ai/repo/policy.yaml || { echo "    the skeleton policy carries no completed_means_complete key"; exit 1; }
grep -q '^  completion: off$' .ai/repo/policy.yaml || { echo "    the skeleton policy does not ship intent.completion off"; exit 1; }
completion_mode required

# the fixture's own CI model, planner and dispatcher, as in case 880. The planner sources the
# tool's lib/ through its own root, so the fixture borrows the tool's lib/ and bin/ and keeps
# its sources in src/ and its probe cases in test/cases/.
mkdir -p .ai/repo/ci src test/cases scripts/ci
ln -s "$ROOT/lib" lib; ln -s "$ROOT/bin" bin; printf 'lib\nbin\n' >> .gitignore
cat > .ai/repo/ci/gates.yaml <<'YAML'
version: 1
gates:
  - id: lint
    job: structure
    always: true
    runs: "true"
    summary: every script parses
  - id: unit
    job: suite
    runs: sh test/unit.sh
    summary: the unit tests pass
classes:
  - id: src
    paths: [src/**, test/**]
    gates: [unit]
  - id: layer
    paths: [.ai/repo/**, AGENTS.md, CLAUDE.md, .bb/**, .gitignore, scripts/**]
    gates: []
YAML
cp "$ROOT/scripts/ci-plan" scripts/ci-plan; cp "$ROOT/scripts/ci/run-plan" scripts/ci/run-plan
echo 'exit 0' > test/unit.sh

# ---------------------------------------------------------------- the plan the work serves
# One intent: criterion a and the optional criterion c are served by I0001, criterion b by
# I0002, and a guard says behaviour g must keep working. The optional criterion is given to
# the issue under test on purpose: a criterion nobody serves would be absent from the answer
# whatever the report did with optional ones.
pj_init
mkdir -p .ai/repo/project/intents .ai/repo/project/critiques
cat > .ai/repo/project/milestones/probe.yaml <<'YAML'
id: probe
title: The probe reaches its outcome
slug: probe
order: 0
priority: p1
problem: "Two behaviours are missing."
outcome: "Both behaviours exist and are proven."
acceptance_criteria:
  - Both behaviours exist
validation:
  - "true"
evidence_required: []
YAML
issue() { # <id> <path> <serves-block>
  cat > ".ai/repo/project/issues/$1.yaml" <<YAML
id: $1
milestone: probe
title: Issue $1
slug: issue-$1
priority: p1
profile: implementation
objective: "Do the work of $1."
scope:
  - $2
$3
acceptance_criteria:
  - The behaviour exists
validation:
  - "true"
evidence_required:
  - proof
YAML
}
issue I0001 src/a 'serves:
  - probe#a
  - probe#c'
issue I0002 src/b 'serves:
  - probe#b'
cat > .ai/repo/project/intents/probe.yaml <<'YAML'
id: probe
title: Both behaviours are true for the people who rely on them
statement: "Behaviour a and behaviour b exist, and the recorded runs of their cases say so."
invariants:
  - No stage of this intent is written anywhere
guards:
  - id: g-stays
    invariant: Behaviour g keeps working while a and b are built
    evidence: test
    ref: test/cases/04_g.sh
milestones:
  - probe
satisfaction:
  - id: a
    criterion: Behaviour a exists
    evidence: test
    ref: test/cases/01_a.sh
  - id: b
    criterion: Behaviour b exists
    evidence: test
    ref: test/cases/02_b.sh
  - id: c
    criterion: Behaviour c would be welcome
    evidence: test
    ref: test/cases/03_c.sh
    optional: true
YAML
# The critique is what lets the task start at all: a binding with no critique is refused, and
# `start` refuses (10) a task whose named binding is refused under every policy, so no task
# of this case would exist to be finished without it.
cat > .ai/repo/project/critiques/probe.yaml <<'YAML'
intent: probe
reviewed_at: HEAD
reviewed_by: case 966
findings:
  - id: thin
    class: insufficient_work
    subject: probe#a
    finding: One case may not be enough
    blocking: true
    resolution:
      state: planned
      issue: I0001
YAML
printf 'grep -qx ok src/a\n' > test/cases/01_a.sh
printf 'grep -qx ok src/b\n' > test/cases/02_b.sh
printf 'grep -qx ok src/c\n' > test/cases/03_c.sh
printf 'grep -qx ok src/g\n' > test/cases/04_g.sh
echo pending > src/a; echo pending > src/b; echo ok > src/g
commit base
# a remote with a default branch, so that push and target can be established live
git init -q --bare "$S/remote.git"; git remote add origin "$S/remote.git"
branch="$(git rev-parse --abbrev-ref HEAD)"
git push -q -u origin "$branch"; git remote set-head origin "$branch"
expect_exit 0 "$BIN" intent validate

completion() { "$BIN" run gates.completion --input '{}' --quiet --format json --repo . 2>/dev/null | jq -c '.output'; }
question() { completion | jq -c --arg q "$1" '.questions[] | select(.id == $q)'; }
q() { question "$1" | jq -r '.status'; }
said() { question "$1" | jq -r '.evidence'; }
withheld() { question "$1" | jq -r '.withheld // "null"'; }
finished() { grep -c '"task.finished"' .ai/local/state/ledger.jsonl || true; }
# every intent record, by path and digest: what must not move
intents() {
  find .ai/repo/project/intents -type f | LC_ALL=C sort | while IFS= read -r f; do
    printf '%s %s\n' "$f" "$(sha256_of_file "$f")"
  done
}
show() { "$BIN" intent show probe --format json; }
# run a probe case the way the runner does and record it the way the runner does: the report,
# the stamp of the tree the run left, the record made with that stamp, and the ledger committed
run() { # <case name>
  local word=ok
  bash "test/cases/$1.sh" || word=FAIL
  printf '%s\t%s\t0\tserial\n' "$1" "$word" > "$S/report.tsv"
  "$BIN" evidence stamp --producer suite --report "$S/report.tsv" --out "$S/report.provenance.json" >/dev/null
  "$BIN" evidence record --suite "$S/report.tsv" --provenance "suite=$S/report.provenance.json" >/dev/null
  commit "record $1: $word"
}
note() { printf '# Objective\n\nx\n\n# Current State\n\nx\n\n# Next Action\n\n%s\n' "${1:-x}"; }
note > "$S/note.md"
# stop a task short, the way case 880 does: a handover, then an outcome that claims nothing
stop() { note "carry on" | "$MJ" handover >/dev/null; expect_exit 0 "$MJ" finish --outcome partial --note "$S/note.md"; }

DECLARED="$(intents)"
[ -n "$DECLARED" ] || { echo "    the fixture declared no intent record to hold still"; exit 1; }

# ---------------------------------------------------------------- required: a served criterion never run
expect_exit 0 "$MJ" start "make behaviour a exist" --scope src,test,.ai/repo --requires tests --issue I0001
echo ok > src/a; echo 'exit 0 # touched' > test/unit.sh
same "a served criterion nobody ran is owed" queued "$(q criteria-served)"
same "required holds what it finds" null "$(withheld criteria-served)"
SHOWN="$(show)"
before="$(finished)"
expect_exit 10 "$MJ" finish --outcome completed --verify-command 'sh test/unit.sh' --note "$S/note.md"
expect_grep 'FAIL +done +criteria-served — queued: .*probe#a '
line="$(grep -E 'done +criteria-served' <<<"$LAST_OUT")"
grep -qE 'probe#b|probe#c' <<<"$line" \
  && { echo "    the refusal names a criterion another issue serves, or an optional one: $line"; exit 1; }
expect_grep 'evidence stamp'
expect_grep '--outcome partial'
expect_grep 'finish: refused'
same "a refused finish wrote no finish record" "$before" "$(finished)"
# nothing was written to the intent, and nothing the intent answers moved
same "the intent records after a refused finish" "$DECLARED" "$(intents)"
same "what intent show answers after a refused finish" "$SHOWN" "$(show)"

# ---------------------------------------------------------------- advisory: reached, reported, withheld
completion_mode advisory
same "advisory holds nothing" exempt "$(q criteria-served)"
same "and carries the verdict it reached" queued "$(withheld criteria-served)"
case "$(said criteria-served)" in
  "reported and not held (intent.completion: advisory); it would be queued: "*probe#a*) ;;
  *) echo "    the advisory answer does not say what it would have been: $(said criteria-served)"; exit 1 ;;
esac
expect_exit 0 "$MJ" check
expect_grep 'INFO +done +criteria-served — withheld \(would be queued\): .*probe#a '
expect_no_grep 'INFO +done +guards-hold — withheld'
completion_mode required
same "required holds it again" queued "$(q criteria-served)"

# ---------------------------------------------------------------- a guard nobody ran, a guard that fails, a guard that holds
same "a guard nobody ran violates nothing" pass "$(q guards-hold)"
case "$(said guards-hold)" in
  *'guard probe!g-stays (test test/cases/04_g.sh) is not judged'*) ;;
  *) echo "    a guard nobody ran is not reported as not judged: $(said guards-hold)"; exit 1 ;;
esac
commit "feat: behaviour a"
echo broken > src/g
commit "behaviour g breaks"
run 04_g
same "a guard whose recorded run failed" fail "$(q guards-hold)"
before="$(finished)"
expect_exit 10 "$MJ" finish --outcome completed --verify-command 'sh test/unit.sh' --note "$S/note.md"
expect_grep 'FAIL +done +guards-hold — fail: guard probe!g-stays \(test test/cases/04_g.sh\) is violated'
expect_grep 'repair what the guard.s failing test names'
same "the refusal over a guard wrote no finish record" "$before" "$(finished)"
same "the intent records after the second refusal" "$DECLARED" "$(intents)"
echo ok > src/g
commit "behaviour g is repaired"
run 04_g
same "a guard whose recorded run passed" pass "$(q guards-hold)"
case "$(said guards-hold)" in
  *'guard probe!g-stays (test test/cases/04_g.sh) holds'*) ;;
  *) echo "    a guard whose run passed is not reported as holding: $(said guards-hold)"; exit 1 ;;
esac

# ---------------------------------------------------------------- accepted by the documented path
# What the remediation says, in its order: the work is committed, the criterion's case runs
# on the clean tree, the tree the run left is stamped, the run is recorded with that stamp,
# and the ledger is committed. It comes first because the ledger is part of the tree: the
# gates and the obligations below are recorded over the tree that carries it.
[ -z "$(git status --porcelain)" ] || { echo "    the tree is not clean before the criterion's run: $(git status --porcelain)"; exit 1; }
run 01_a
# everything else the report asks, settled as case 880 settles it
expect_exit 0 "$MJ" evidence --run-gates
expect_exit 0 "$MJ" evidence --covers implementation --command 'git diff --stat'
expect_exit 0 "$MJ" evidence --covers tests --command 'sh test/unit.sh'
# The ledger the remediation has the worker commit lives under .ai/repo/, which the
# `generated` obligation is taken over: recording the criterion's run is what makes that
# obligation implied, and the questions `generated` and `openapi` owed. It is discharged the
# way any implied obligation is, by the command that settles it.
same "committing the ledger implies the generated obligation" queued "$(q generated)"
expect_exit 0 "$MJ" evidence --covers generated --command "$BIN generate --check"
[ -z "$(git status --porcelain)" ] || { echo "    settling the rest left the tree dirty: $(git status --porcelain)"; exit 1; }
git push -q origin "$branch"
note "finish" | "$MJ" handover >/dev/null
same "a served criterion with current evidence" pass "$(q criteria-served)"
case "$(said criteria-served)" in
  '1 served criterion(s) have current evidence: probe#a') ;;
  *) echo "    the passing answer does not name the one criterion it read: $(said criteria-served)"; exit 1 ;;
esac
[ "$(completion | jq -r .complete)" = true ] || { echo "    the criterion is proven and the report is not complete: $(completion | jq -c '[.questions[] | select(.status != "pass" and .status != "exempt") | {id, status, evidence}]')"; exit 1; }
expect_exit 0 "$MJ" finish --outcome completed --verify-command 'sh test/unit.sh' --note "$S/note.md"
grep '"task.finished"' .ai/local/state/ledger.jsonl | tail -1 | grep -q '"complete":true' \
  || { echo "    the finish record does not carry complete:true"; exit 1; }
same "the intent records after the accepted finish" "$DECLARED" "$(intents)"

# ---------------------------------------------------------------- a scoped finish is not refused
expect_exit 0 "$MJ" start "make behaviour b exist" --scope src,test,.ai/repo --issue I0002
echo nearly > src/b
same "the other issue's criterion was never run" queued "$(q criteria-served)"
case "$(said criteria-served)" in
  *probe#b*) ;;
  *) echo "    the owed criterion is not the one this issue serves: $(said criteria-served)"; exit 1 ;;
esac
stop
grep '"task.finished"' .ai/local/state/ledger.jsonl | tail -1 | grep -q '"outcome":"partial".*"complete":false' \
  || { echo "    a partial finish over an owed criterion is not recorded as partial and incomplete"; exit 1; }

# ---------------------------------------------------------------- held to nothing, and saying why
both_exempt() { # <what>
  local id
  for id in criteria-served guards-hold; do
    same "$id for $1" exempt "$(q "$id")"
    same "$id for $1 withholds nothing" null "$(withheld "$id")"
  done
}
expect_exit 0 "$MJ" start "upkeep" --scope src --exempt maintenance --because "no issue covers this upkeep"
both_exempt "an exempt task"
case "$(said criteria-served)" in
  'the task is exempt as maintenance: no issue covers this upkeep') ;;
  *) echo "    the exemption is not what the answer says: $(said criteria-served)"; exit 1 ;;
esac
stop
expect_exit 0 "$MJ" start "unnamed" --scope src
both_exempt "a task naming nothing"
same "what a task naming nothing owes is the issue question's to say" queued "$(q issue)"
stop
# an intent named directly: no criterion is this task's to prove, and the guards are asked
expect_exit 0 "$MJ" start "look after the probe" --scope src --intent probe
same "a task naming an intent and no issue proves no criterion" exempt "$(q criteria-served)"
same "and nothing was withheld to say so" null "$(withheld criteria-served)"
same "its intent's guards are asked all the same" pass "$(q guards-hold)"
stop
same "the intent records at the end" "$DECLARED" "$(intents)"
echo "    ok"

# majordomus-covers: start plan
# An issue another open pull request already executes is refused at start, naming its holder,
# and --alongside starts it anyway with the reason recorded (I4775, ADR 0238).
#
# A fixture plan with I0001 and I0002 READY, and a forge observation in which pull request #9,
# from the branch fix/I0001-elsewhere, is open. `start --issue I0001` asks plan.collisions:
#
#   - I0001 is executed by #9, so start refuses (15) and names #9 and its branch;
#   - `--alongside "<reason>"` starts it, and the task record keeps the reason;
#   - I0002, which nothing holds, starts as before;
#   - `--alongside` without `--issue` is a usage error.
. "$ROOT/test/lib.sh"
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_PREFIX
command -v jq >/dev/null 2>&1 || skip "jq absent"
fail() { printf '    %s\n' "$*"; exit 1; }

"$MJ" init >/dev/null
pj_init; pj_milestone M1
for i in I0001 I0002; do pj_issue "$i" M1; done
git add -A >/dev/null && git commit -qm "plan: two issues"
mkdir -p .ai/local/state/integration
cat > .ai/local/state/integration/observation.json <<J
{"schema":4,"repository":"someone/fixture","base":"master","base_sha":"0000000000000000000000000000000000000000",
 "observed_at":"$(date -u +%Y-%m-%dT%H:%M:%SZ)","required_checks":null,"review_policy":null,"merge_methods":["merge"],
 "pull_requests":[{"number":9,"title":"fix: elsewhere","author":"someone","head_ref":"fix/I0001-elsewhere",
  "head_sha":"1111111111111111111111111111111111111111","base_ref":"master","draft":false,
  "created_at":"2026-11-25T07:00:00Z","updated_at":"2026-11-25T07:30:00Z"}]}
J

rc=0; err="$("$MJ" start "the work" --scope src --issue I0001 2>&1 >/dev/null)" || rc=$?
[ "$rc" = 15 ] || fail "start of an issue #9 executes is refused with 15, got $rc: $err"
printf '%s\n' "$err" | grep -q 'REFUSED  claimed_elsewhere  open pull request #9 (fix/I0001-elsewhere' \
  || fail "the refusal names the holder: $err"
printf '%s\n' "$err" | grep -q -- '--alongside' || fail "the refusal names the way to start anyway: $err"
[ -f .ai/local/state/current.yaml ] && grep -q '^outcome: active' .ai/local/state/current.yaml \
  && fail "a refused start writes no active task"

rc=0; "$MJ" start "x" --scope src --alongside "why" >/dev/null 2>&1 || rc=$?
[ "$rc" = 2 ] || fail "--alongside without --issue is a usage error, got $rc"

out="$("$MJ" start "the work" --scope src --issue I0001 --alongside "pairing with #9 on the parser half" 2>&1)" \
  || fail "start --alongside failed: $out"
grep -q '^alongside: "pairing with #9 on the parser half"' .ai/local/state/current.yaml \
  || fail "the task keeps the reason: $(cat .ai/local/state/current.yaml)"
printf '# Next Action\nThe fixture ends here.\n' > .ai/local/note.md
"$MJ" finish --outcome blocked --note .ai/local/note.md >/dev/null 2>&1 \
  || fail "the alongside task could not be closed"

out="$("$MJ" start "other work" --scope src --issue I0002 2>&1)" || fail "an issue nobody holds starts: $out"

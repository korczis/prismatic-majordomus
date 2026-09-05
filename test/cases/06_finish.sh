# majordomus-covers: finish
# majordomus-negative: finish
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib test && echo a > lib/a && echo t > test/a_test && git add . && git commit -qm base
# --check with no task is not a failure
expect_exit 0 "$MJ" finish --check
expect_grep 'nothing to enforce'
expect_exit 12 "$MJ" finish --outcome completed
"$MJ" start "t1" --scope lib,test --profile debugging >/dev/null
expect_exit 2 "$MJ" finish
expect_exit 2 "$MJ" finish --outcome nonsense
echo b >> lib/a
# contract printed line by line; verification required by profile
expect_exit 10 "$MJ" finish --outcome completed
expect_grep 'OK +scope'
expect_grep 'FAIL verification .* requires --verify-command'
expect_grep 'FAIL note .* no handover'
expect_grep 'finish: refused, 3 unmet'
expect_grep '^outcome: active$' .ai/local/state/current.yaml
# failing verify command is recorded as a failure
expect_exit 10 "$MJ" finish --outcome completed --verify-command "false"
expect_grep 'FAIL verification .* false — exit 1'
# handover supplies the note; regression test path is required by debugging profile
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null
expect_exit 10 "$MJ" finish --outcome completed --verify-command "true"
expect_grep 'OK +note'
expect_grep 'FAIL regression .* no test path'
echo t2 >> test/a_test
expect_exit 0 "$MJ" finish --outcome completed --verify-command "true"
expect_grep 'OK +regression'
expect_grep 'finish: t-.* completed'
expect_grep '^outcome: completed$' .ai/local/state/current.yaml
expect_grep '"event":"task.finished".*"outcome":"completed".*"majordomus.verification-integrity":"pass".*"verify":\{"command":"true","exit":0' .ai/local/state/ledger.jsonl
# finishing twice is refused; --check on a finished task passes
expect_exit 15 "$MJ" finish --outcome completed
expect_exit 0 "$MJ" finish --check
# new task archives the finished one (commit t1's work first so it is not "touched" by t2)
git add -A && git commit -qm t1
expect_exit 0 "$MJ" start "t2" --scope lib
[ "$(find .ai/local/state/archive -name '*.yaml' | wc -l | tr -d ' ')" -ge 1 ]
# blocked: needs a note with Next Action, blockers expected
id=$(sed -n 's/^id: //p' .ai/local/state/current.yaml)
printf -- '- [unresolved] %s — which db? (2026-09-03)\n' "$id" >> .ai/local/state/open-questions.md
note="$(mktemp "${TMPDIR:-/tmp}/mj-note.XXXXXX")"; printf '# Objective\no\n# Current State\nwaiting\n' > "$note"
expect_exit 10 "$MJ" finish --outcome blocked --note "$note"
expect_grep 'lacks section\(s\): Next Action'
printf '# Next Action\nask\n' >> "$note"
expect_exit 0 "$MJ" finish --outcome blocked --note "$note"
expect_grep 'INFO blockers .* open questions do not refuse it'
[ -f ".ai/local/state/completed/$id.md" ]
# no_match needs # Reason; verification skipped
"$MJ" start "t3" --scope lib >/dev/null
n2="$(mktemp "${TMPDIR:-/tmp}/mj-note.XXXXXX")"; printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' > "$n2"
expect_exit 10 "$MJ" finish --outcome no_match --note "$n2"
expect_grep 'lacks section\(s\): Reason'
printf '# Reason\nnot there\n' >> "$n2"
expect_exit 0 "$MJ" finish --outcome no_match --note "$n2"
expect_grep 'INFO verification .* skipped for outcome no_match'
# --check fails on out-of-scope work
"$MJ" start "t4" --scope lib >/dev/null
echo x > test/other && git add test/other && git commit -qm oos
expect_exit 10 "$MJ" finish --check
expect_grep 'FAIL scope +test/other'
# a scope failure blocks a completed finish even with everything else present
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null
expect_exit 10 "$MJ" finish --outcome completed --verify-command true
expect_grep 'FAIL scope'

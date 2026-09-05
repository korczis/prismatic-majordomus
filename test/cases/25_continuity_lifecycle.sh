# majordomus-covers: start context checkpoint decision question handover history search check finish
# majordomus-negative: check finish doctor
# majordomus-lifecycle: accepted
# Integration: whole lifecycles through the real CLI, not modules in isolation.
#
# The point of the subsystem is that a worker who has lost its conversation can resume from
# records. These sequences prove that end to end, in both directions: work that continues
# through a handover, and work that completes and leaves a reconstructable history.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib/auth test && echo a > lib/auth/callback && echo t > test/auth_test && git add . && git commit -qm base

# =================================================================== 1. start → handover → resume
"$MJ" start "fix the OAuth callback" --scope lib/auth,test --profile debugging --owner alice >/dev/null
id1=$(sed -n 's/^id: //p' .ai/local/state/current.yaml)
expect_grep '"event":"task.started"' .ai/local/state/ledger.jsonl

# the worker gets its briefing, does work, records what it learned and what blocks it
expect_exit 0 "$MJ" context
expect_grep '^task         fix the OAuth callback$'
echo "normalise" >> lib/auth/callback
printf 'state mismatch reproduced with the fixture\ncause is in normalisation, not comparison\n' | "$MJ" checkpoint >/dev/null
"$MJ" decision add "Normalise the callback URI before comparing state" \
  --why "the mismatch is a trailing slash, not a forged state parameter" \
  --evidence "test/auth_test" >/dev/null
"$MJ" question add "does the legacy mobile client still send the old URI form?" >/dev/null

# check sees the work, the blocker, and the newest checkpoint
expect_exit 10 "$MJ" check
expect_grep 'OK +scope .* all within scope'
expect_grep 'FAIL blockers .* does the legacy mobile client'
expect_grep 'INFO checkpoint +\.ai/local/state/checkpoints/'

# the blocker refuses completion, exactly as the contract says
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null
expect_exit 10 "$MJ" finish --outcome completed --verify-command true
expect_grep 'FAIL blockers'
expect_grep '^outcome: active$' .ai/local/state/current.yaml

# the worker stops instead, handing over
git commit -qam wip
"$MJ" handover --close <<'EOF' >/dev/null
# Objective
Fix the OAuth callback state mismatch.
# Current State
Reproduced. The cause is URI normalisation in the callback handler, not the comparison.
# Next Action
Write the failing regression test in test/auth_test before changing the handler.
EOF
expect_grep '^outcome: handed_over$' .ai/local/state/current.yaml

# --- a new session, which knows nothing except the repository -------------------------
# everything it needs is in one command, resolved to this worktree and branch
expect_exit 0 "$MJ" context
expect_grep '^## LATEST COMPATIBLE HANDOVER'
expect_grep '^match        same_worktree_same_branch$'
expect_grep 'Write the failing regression test'
expect_grep '^## LATEST CHECKPOINT'
expect_grep 'cause is in normalisation'
expect_grep '^## DECISIONS'
expect_grep 'Normalise the callback URI'
expect_grep '^## OPEN QUESTIONS'
expect_grep 'legacy mobile client'
# the resolver agrees, and shows how far git has moved since
expect_exit 0 "$MJ" handover --resolve
expect_grep '^Git state: exact'
expect_grep '^Task: '"$id1"'$'

# it continues the same work as a new task; start names the record it would resume from
expect_exit 0 "$MJ" start "continue the OAuth callback fix" --scope lib/auth,test --profile debugging --owner bob
expect_grep 'INFO handover +\.ai/local/state/handovers/.* prior record, same_worktree_same_branch'
id2=$(sed -n 's/^id: //p' .ai/local/state/current.yaml)
[ "$id1" != "$id2" ]

# the earlier task's blocker is still open, and it still blocks: the work moved to a new
# task, the question did not stop being unanswered, and a gate that forgot at the task
# boundary would be a gate a handover walks past. See M001 and docs/CONTINUITY.md.
expect_exit 10 "$MJ" check
expect_grep 'FAIL +blockers +'"$id2"' — 1 unresolved question\(s\) on this branch'
expect_grep 'legacy mobile client'
expect_exit 0 "$MJ" question list
expect_grep 'legacy mobile client'
expect_exit 0 "$MJ" question list --all
expect_grep 'legacy mobile client'

# do the work the handover asked for, answer the question, and complete
echo "regression" >> test/auth_test
printf 'test written and failing for the recorded reason\n' | "$MJ" checkpoint >/dev/null
"$MJ" question add "should the old URI form keep working after 4.2?" >/dev/null
expect_exit 10 "$MJ" finish --outcome completed --verify-command true
expect_grep 'FAIL blockers'
# a selector that matches two open questions is refused rather than resolving the wrong one
expect_exit 2 "$MJ" question resolve "old URI form" --answer "no"
expect_grep 'matches more than one question'
"$MJ" question resolve "keep working after 4.2" --answer "no, it was retired in 4.2" >/dev/null
# the blocker the first task opened is still open, so completion is still refused: both have
# to be answered, and the one inherited across the handover is not exempt for being older
expect_exit 10 "$MJ" finish --outcome completed --verify-command true
expect_grep 'legacy mobile client'
"$MJ" question resolve "legacy mobile client" --answer "no; the 3.x client was retired before 4.0" >/dev/null
printf '# Objective\nFix the OAuth callback.\n# Current State\nFixed and covered.\n# Next Action\nnone\n' | "$MJ" handover >/dev/null
expect_exit 0 "$MJ" finish --outcome completed --verify-command "true"
expect_grep 'OK +verification .* exit 0'
expect_grep 'OK +regression .* a test path was touched'
expect_grep '^outcome: completed$' .ai/local/state/current.yaml

# =================================================================== 2. history reconstructs it
# every step of both tasks is in the ledger, in order, with its git head
expect_exit 0 "$MJ" history --all
for e in task.started task.checkpoint decision.recorded question.opened question.resolved task.handed_over task.finished; do
  expect_grep "$e"
done
expect_exit 0 "$MJ" history --task "$id1" --all
expect_grep 'task.handed_over'
expect_no_grep "$id2"
expect_exit 0 "$MJ" history --task "$id2" --all
expect_grep 'outcome=completed verify_exit=0'
expect_grep 'question.resolved'
# the verification that was accepted is recorded with its command and exit code
expect_grep '"event":"task.finished".*"verify":\{"command":"true","exit":0' .ai/local/state/ledger.jsonl
expect_grep '"checkpoints":[0-9]' .ai/local/state/ledger.jsonl

# search finds the durable knowledge without reading any of the files
expect_exit 0 "$MJ" search "normalisation"
expect_grep '^checkpoint '
# search prints the line that matched, so the rationale is what comes back for a rationale term
expect_exit 0 "$MJ" search "trailing slash" --kind decision
expect_grep '^decision +\.ai/local/state/decisions.md:[0-9]+ +Why: the mismatch is a trailing slash'
expect_exit 0 "$MJ" search "Normalise the callback" --kind decision
expect_grep 'Normalise the callback URI before comparing state'

# =================================================================== 3. a stopped task
git add -A && git commit -qm complete
"$MJ" start "investigate the token store" --scope lib/auth --profile implementation >/dev/null
id3=$(sed -n 's/^id: //p' .ai/local/state/current.yaml)
"$MJ" question add "which team owns the token store?" >/dev/null
expect_exit 0 "$MJ" question list
expect_grep 'which team owns the token store' 
note="$(mktemp "${TMPDIR:-/tmp}/mj-note.XXXXXX")"
printf '# Objective\nFind the token store owner.\n# Current State\nBlocked on ownership.\n' > "$note"
# blocked still needs a next action for whoever picks it up
expect_exit 10 "$MJ" finish --outcome blocked --note "$note"
expect_grep 'lacks section\(s\): Next Action'
printf '# Next Action\nAsk in the platform channel.\n' >> "$note"
expect_exit 0 "$MJ" finish --outcome blocked --note "$note"
expect_grep 'INFO blockers .* open questions do not refuse it'
# with no handover for this task, the continuity finding says the note is the only record
expect_grep 'WARN continuity .* the note.s Next Action is the only continuation record'
expect_grep '^outcome: blocked$' .ai/local/state/current.yaml
[ -f ".ai/local/state/completed/$id3.md" ]
expect_exit 0 "$MJ" history --task "$id3" --all
expect_grep 'outcome=blocked' 

# =================================================================== 4. the whole tree is healthy
expect_exit 0 "$MJ" watch
expect_grep 'OK +records +open-questions.md'
expect_grep 'OK +records +ledger.jsonl'
expect_grep 'OK +context +builder'
expect_grep 'watch: 0 drift'

# and doctor proves each store is reachable through its own command, not merely present
expect_exit 10 "$MJ" doctor          # hooks are not wired in a test repository
expect_grep 'OK +layout +\.ai/local/state/checkpoints'
expect_grep 'OK +layout +\.ai/repo/prompts'
expect_grep 'OK +records +open-questions.md'
expect_grep 'OK +records +ledger.jsonl'
expect_grep 'OK +prompt +[0-9]+ asset'
expect_grep 'OK +resolver +handovers'
expect_grep 'OK +context +builder .* budget'
expect_grep 'OK +retention +checkpoints'

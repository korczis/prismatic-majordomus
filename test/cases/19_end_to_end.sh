# majordomus-covers: start check finish handover question doctor watch doctrine update
# majordomus-negative: check finish doctor
# majordomus-lifecycle: accepted
# One complete workflow, start to finish, through the real commands only — the shape a
# repository actually adopts. Every refusal below is a doctrine refusing, and the reason
# is asserted by name rather than by exit code alone.
. "$ROOT/test/lib.sh"

# --- install and wire
expect_exit 12 "$MJ" doctor
expect_grep 'run: majordomus init'
"$MJ" init >/dev/null
mkdir -p .git/hooks
printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .git/hooks/pre-commit
printf '#!/usr/bin/env bash\nmajordomus finish --check\n' > .git/hooks/pre-push
chmod +x .git/hooks/pre-commit .git/hooks/pre-push
PATH="$(dirname "$MJ"):$PATH"; export PATH
# projections do not exist yet: a missing artifact, not a passing installation
expect_exit 12 "$MJ" doctor
expect_grep 'FAIL projection +CLAUDE.md — missing'
"$MJ" update >/dev/null
expect_exit 0 "$MJ" doctor
expect_grep 'OK +wiring +doctor-on-commit'
expect_grep 'OK +doctrine'

mkdir -p src test && echo 'x' > src/a.py && echo 'x' > test/a_test.py
git add -A && git commit -qm base

# --- start a scoped task
"$MJ" start "add the retry seam" --scope src,test --profile debugging >/dev/null
id="$(awk '/^id:/{print $2}' .ai/local/state/current.yaml)"
expect_exit 0 "$MJ" check
expect_grep 'OK +scope'

# --- the profile and the effective policy are readable without reading the repository
expect_exit 0 "$MJ" check --explain
expect_grep 'doctrines enforced by check'
expect_grep 'majordomus.checkpoint-freshness \(advisory\)'

# --- work inside scope; checkpoint
echo 'y' >> src/a.py
expect_exit 0 "$MJ" check --checkpoint
expect_grep 'INFO checkpoint'

# --- a decision is externalised, not left in the conversation
printf 'Task: %s\n\nChose an in-band retry seam over a wrapper: the wrapper duplicated the\nbackoff policy in two places.\n\nRejected: a decorator around the client.\n' "$id" >> .ai/local/state/decisions.md

# --- a blocking question refuses completion, by name
"$MJ" question add "does the upstream rate limit reset per minute or per hour?" >/dev/null
expect_exit 10 "$MJ" check
expect_grep 'FAIL blockers'
expect_exit 10 "$MJ" check --rule majordomus.blocker-resolution
expect_grep 'FAIL blockers'
# a doctrine that this command does not enforce is a usage error, not a silent pass
expect_exit 2 "$MJ" check --rule majordomus.verification-integrity
expect_grep 'is not enforced by check'

# --- resolve it; check passes again
"$MJ" question resolve 1 --answer "per minute; confirmed against the published limits" >/dev/null
expect_exit 0 "$MJ" check

# --- work outside the claimed scope is refused
echo 'z' > README.md
expect_exit 10 "$MJ" check
expect_grep 'FAIL scope +README.md'
rm -f README.md
expect_exit 0 "$MJ" check

# --- hand over, then resolve the handover in this same checkout
printf '# Objective\nAdd a retry seam.\n# Current State\nSeam added; the regression test is not written.\n# Next Action\nWrite the regression test.\n' | "$MJ" handover --close >/dev/null
expect_grep '^outcome: handed_over$' .ai/local/state/current.yaml
expect_exit 0 "$MJ" watch
expect_grep 'OK +handover'
# the continuing session finds the record for this worktree and branch
expect_exit 0 "$MJ" handover --resolve
expect_grep 'Next Action'

# --- finish refuses without verification, and names the doctrine
"$MJ" start "finish the retry seam" --scope src,test --profile debugging >/dev/null
id2="$(awk '/^id:/{print $2}' .ai/local/state/current.yaml)"
echo 'retry' >> src/a.py
printf 'Task: %s\n\nKept the seam.\n' "$id2" >> .ai/local/state/decisions.md
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null
expect_exit 10 "$MJ" finish --outcome completed
expect_grep 'FAIL verification .* requires --verify-command'
expect_grep 'blocking doctrines:'
expect_grep 'majordomus.verification-integrity'

# --- a verification that fails is a failure, recorded as one
expect_exit 10 "$MJ" finish --outcome completed --verify-command "exit 3"
expect_grep 'FAIL verification .* exit 3'

# --- the debugging profile also demands a regression test
expect_exit 10 "$MJ" finish --outcome completed --verify-command "true"
expect_grep 'FAIL regression'
echo 'assert' >> test/a_test.py

# --- everything satisfied: finish is accepted and recorded
expect_exit 0 "$MJ" finish --outcome completed --verify-command "true"
expect_grep 'OK +verification .* exit 0'
expect_grep "finish: $id2 completed"
expect_grep '^outcome: completed$' .ai/local/state/current.yaml

# --- the ledger records the contract line by line, under doctrine ids
expect_grep '"event":"task.finished"' .ai/local/state/ledger.jsonl
expect_grep '"majordomus.scope-integrity":"pass"' .ai/local/state/ledger.jsonl
expect_grep '"majordomus.verification-integrity":"pass"' .ai/local/state/ledger.jsonl
expect_grep '"verify":\{"command":"true","exit":0' .ai/local/state/ledger.jsonl

# --- and the installation is still healthy at the end of it
expect_exit 0 "$MJ" doctor
expect_exit 0 "$MJ" watch
expect_exit 0 "$MJ" doctrine status

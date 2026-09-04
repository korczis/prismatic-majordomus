# Status derivation. No issue file carries a status; every state below is computed from
# what was recorded about the issue and from the state of its dependencies.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
pj_init
pj_milestone M000
pj_issue I0001 M000
pj_issue I0002 M000 I0001
expect_exit 0 "$MJ" plan validate

# --- a root with nothing recorded is READY; anything waiting on it is BLOCKED
[ "$(pj_status I0001)" = READY ]   || { echo "    I0001 should be READY, is $(pj_status I0001)"; exit 1; }
[ "$(pj_status I0002)" = BLOCKED ] || { echo "    I0002 should be BLOCKED, is $(pj_status I0002)"; exit 1; }
expect_exit 0 "$MJ" plan blocked
expect_grep 'waiting on: I0001'

# --- a BLOCKED issue cannot be started, and the refusal names what it waits on
expect_exit 15 "$MJ" plan start I0002
expect_grep 'I0002 is BLOCKED, not READY \(waiting on I0001\)'

# --- start records a fact; the status follows from it
expect_exit 0 "$MJ" plan start I0001
[ "$(pj_status I0001)" = ACTIVE ] || { echo "    I0001 should be ACTIVE, is $(pj_status I0001)"; exit 1; }
grep -q '^started_at: ' .majordomus/project/issues/I0001.yaml || { echo "    started_at was not recorded"; exit 1; }
expect_no_grep '^status:' .majordomus/project/issues/I0001.yaml

# --- starting twice is refused: ACTIVE is not READY
expect_exit 15 "$MJ" plan start I0001
expect_grep 'I0001 is ACTIVE, not READY'

# --- implementation complete, evidence pending
expect_exit 0 "$MJ" plan verify I0001
[ "$(pj_status I0001)" = VERIFY ] || { echo "    I0001 should be VERIFY, is $(pj_status I0001)"; exit 1; }

# --- DONE is gated on evidence, not on a claim
expect_exit 10 "$MJ" plan "done" I0001
expect_grep 'I0001 has no evidence for: proof'
[ "$(pj_status I0001)" = VERIFY ] || { echo "    a refused done must not change the status"; exit 1; }

# --- evidence must name a token the issue actually requires
expect_exit 15 "$MJ" plan evidence I0001 --covers invented --type test --command true
expect_grep 'does not require evidence'

# --- narrative is not evidence: a command or an artifact is required
expect_exit 2 "$MJ" plan evidence I0001 --covers proof --type test --result "it worked"
expect_grep 'narrative is not evidence'

expect_exit 0 "$MJ" plan evidence I0001 --covers proof --type test --command "true" --result "exit 0"
expect_exit 0 "$MJ" plan "done" I0001
[ "$(pj_status I0001)" = DONE ] || { echo "    I0001 should be DONE, is $(pj_status I0001)"; exit 1; }

# --- the graph recomputes: what was blocked is now ready, and the tool says so unprompted
expect_grep 'next ready issue: I0002'
[ "$(pj_status I0002)" = READY ] || { echo "    I0002 should be READY, is $(pj_status I0002)"; exit 1; }

# --- completed_at without its evidence derives VERIFY, never DONE
sed '/^evidence:/,$d' .majordomus/project/issues/I0001.yaml > /tmp/i.$$ && mv /tmp/i.$$ .majordomus/project/issues/I0001.yaml
printf 'completed_at: 2026-01-01T00:00:00Z\n' >> .majordomus/project/issues/I0001.yaml
[ "$(pj_status I0001)" = VERIFY ] || { echo "    completed without evidence must be VERIFY, is $(pj_status I0001)"; exit 1; }
expect_exit 0 "$MJ" plan validate
expect_grep 'evidence is missing for proof'

# --- a cancelled issue leaves the graph without blocking anything
pj_issue I0005 M000
printf 'cancelled: true\n' >> .majordomus/project/issues/I0005.yaml
[ "$(pj_status I0005)" = CANCELLED ] || { echo "    I0005 should be CANCELLED, is $(pj_status I0005)"; exit 1; }

# --- milestone status is derived from its issues, not from a count of closed tickets
expect_exit 0 "$MJ" plan status
expect_grep '^M000   ACTIVE'

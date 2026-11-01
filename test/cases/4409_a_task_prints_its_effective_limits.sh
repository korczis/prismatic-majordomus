# majordomus-covers: doctrine
# majordomus-negative: doctrine
. "$ROOT/test/lib.sh"
# `majordomus doctrine limits` prints the limits a profile derives, as text and as JSON, for
# the active task's profile or for the default profile when no task is active (I0202). The
# numbers are mj_limits_resolve's, and the code that applies a limit asks the same resolver,
# so each assertion below compares the enforced value with the command's output, never with
# a constant.
"$MJ" init >/dev/null
limit() { "$MJ" doctrine limits --json | jq -r --arg l "$1" '.limits[] | select(.id == $l) | .value'; }

# no task: the default profile is resolved and named as the default
expect_exit 0 "$MJ" doctrine limits
expect_grep "^profile implementation, the default: no task is active$"
expect_grep "^  checkpoint_max_age_seconds +900 seconds +from checkpoint_interval: 15m$"
text_ids="$(printf '%s\n' "$LAST_OUT" | awk '/^  / { print $1 }' | sort | tr '\n' ' ')"
expect_exit 0 "$MJ" doctrine limits --json
printf '%s' "$LAST_OUT" | jq -e '.source == "default" and .task == null and .profile == "implementation" and .complete' >/dev/null
# the JSON carries the same set of limits the text does, and every declared one
json_ids="$(printf '%s' "$LAST_OUT" | jq -r '.limits[].id' | sort | tr '\n' ' ')"
[ "$text_ids" = "$json_ids" ] || { printf '    text %s, json %s\n' "$text_ids" "$json_ids"; exit 1; }
[ "$json_ids" = "checkpoint_max_age_seconds history_events knowledge_pointers " ]

# inside a task, its profile's limits; the history section is as deep as the command says
expect_exit 0 "$MJ" start "limits" --scope lib --profile debugging
expect_exit 0 "$MJ" doctrine limits
expect_grep "^profile debugging, of task t-"
"$MJ" doctrine limits --json | jq -e '.source == "task" and (.task | startswith("t-"))' >/dev/null
depth="$(limit history_events)"; [ "$depth" = 50 ]
expect_exit 0 "$MJ" context
expect_grep "^## RECENT HISTORY \(last $depth events\)$"

# a changed axis changes the command's number and the enforced one together
sed 's/recent_history_depth: .*/recent_history_depth: 3/' .ai/repo/profiles/debugging.yaml > "$T/p" && mv "$T/p" .ai/repo/profiles/debugging.yaml
depth="$(limit history_events)"; [ "$depth" = 3 ]
expect_exit 0 "$MJ" context
expect_grep "^## RECENT HISTORY \(last 3 events\)$"

# a declared cap holds: a profile cannot ask past the policy's context.max_list_items
sed 's/knowledge_max_items: .*/knowledge_max_items: 999/' .ai/repo/profiles/debugging.yaml > "$T/p" && mv "$T/p" .ai/repo/profiles/debugging.yaml
cap="$(sed -n 's/^ *max_list_items: *\([0-9]*\).*/\1/p' .ai/repo/policy.yaml)"
[ -n "$cap" ] && [ "$(limit knowledge_pointers)" = "$cap" ]
expect_exit 0 "$MJ" doctrine limits
expect_grep "^  knowledge_pointers +$cap knowledge records +from context.knowledge_max_items: 999, never above policy context.max_list_items$"

# a value the derivation cannot read is no limit at all: said by name, exit 10, never zero
sed 's/^checkpoint_interval: .*/checkpoint_interval: soon/' .ai/repo/profiles/debugging.yaml > "$T/p" && mv "$T/p" .ai/repo/profiles/debugging.yaml
expect_exit 10 "$MJ" doctrine limits
expect_grep "^  checkpoint_max_age_seconds +- seconds +checkpoint_interval: 'soon' is not a duration_seconds value$"
expect_exit 10 "$MJ" doctrine limits --json
printf '%s' "$LAST_OUT" | jq -e '(.complete | not) and ([.limits[] | select(.id == "checkpoint_max_age_seconds")][0] | .status == "invalid" and .value == null)' >/dev/null

expect_exit 2 "$MJ" doctrine limits --bogus

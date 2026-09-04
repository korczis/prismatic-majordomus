# Prompt assets and search: discovery, safe rendering with a closed token set, refusal of
# anything outside it, and literal retrieval across the record kinds in authority order.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base

# ---------------------------------------------------------------- prompt
expect_exit 2 "$MJ" prompt
expect_grep 'usage: majordomus prompt list'
expect_exit 2 "$MJ" prompt nonsense
expect_exit 2 "$MJ" prompt show
expect_grep 'a name is required'

# init installed the skeleton assets, and list shows each with its description
expect_exit 0 "$MJ" prompt list
for n in continue debug handover review; do expect_grep "^$n +"; [ -f ".majordomus/prompts/$n.md" ]; done
expect_exit 0 "$MJ" prompt show debug
expect_grep '^name: debug$'
expect_exit 12 "$MJ" prompt show nosuch
expect_grep 'no prompt asset'
expect_exit 12 "$MJ" prompt render nosuch
# a name that could escape the directory is refused before any file is touched
expect_exit 2 "$MJ" prompt show ../../etc/passwd
expect_grep 'not a valid asset name'
expect_exit 2 "$MJ" prompt show .hidden

"$MJ" start "fix the callback" --scope lib --profile debugging --owner alice >/dev/null
id=$(sed -n 's/^id: //p' .majordomus/state/current.yaml)
"$MJ" question add "which environments still run the old client?" >/dev/null

# every inline token is substituted with state, and none survives into the output
cat > .majordomus/prompts/probe.md <<'P'
---
name: probe
description: exercises every inline token
---
task={{TASK}} id={{TASK_ID}} profile={{PROFILE}} scope={{SCOPE}} owner={{OWNER}}
branch={{BRANCH}} head={{HEAD}} tree={{WORKING_TREE}} repo={{REPOSITORY}} now={{NOW}}
repeated: {{TASK_ID}} and {{TASK_ID}}
P
expect_exit 0 "$MJ" prompt render probe
expect_grep "task=fix the callback id=$id profile=debugging scope=lib owner=alice"
expect_grep "branch=$(git branch --show-current) head=$(git rev-parse HEAD) tree=$(git status --porcelain | grep -q . && echo dirty || echo clean)"
expect_grep "repeated: $id and $id"
expect_no_grep '\{\{'

# block tokens expand to state, on their own line
cat > .majordomus/prompts/blocky.md <<'P'
---
name: blocky
description: exercises the block tokens
---
questions:
{{OPEN_QUESTIONS}}
decisions:
{{DECISIONS}}
P
expect_exit 0 "$MJ" prompt render blocky
expect_grep '^- which environments still run the old client\? \([0-9-]+\)$'   # the date stays: a question's age matters
expect_grep '^\(none\)$'
expect_no_grep '\{\{'
"$MJ" decision add "one store, not two" --why "a second source of truth drifts" >/dev/null
expect_exit 0 "$MJ" prompt render blocky
expect_grep 'one store, not two'

# an unknown token is an error, not silently emitted as literal text
printf -- '---\nname: typo\ndescription: d\n---\nhello {{TSAK}}\n' > .majordomus/prompts/typo.md
expect_exit 10 "$MJ" prompt render typo
expect_grep 'unknown token \{\{TSAK\}\}'
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL prompt +typo .* unknown token'
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT prompt +typo'
rm .majordomus/prompts/typo.md

# a block token used inline is an error: it would expand many lines mid-sentence
printf -- '---\nname: inline\ndescription: d\n---\nsee {{DECISIONS}} here\n' > .majordomus/prompts/inline.md
expect_exit 10 "$MJ" prompt render inline
expect_grep 'must be alone on its line'
rm .majordomus/prompts/inline.md

# front matter is validated the way every other Majordomus file is
printf -- '---\nname: wrong\ndescription: d\n---\nbody\n' > .majordomus/prompts/mismatch.md
expect_exit 10 "$MJ" prompt render mismatch
expect_grep 'does not match filename'
printf -- '---\nname: nodesc\ndescription:\n---\nbody\n' > .majordomus/prompts/nodesc.md
expect_exit 10 "$MJ" prompt render nodesc
expect_grep 'description is empty'
printf -- '---\nname: extra\ndescription: d\ncolour: blue\n---\nbody\n' > .majordomus/prompts/extra.md
expect_exit 10 "$MJ" prompt render extra
expect_grep 'unknown front-matter key'
printf 'no front matter at all\n' > .majordomus/prompts/bare.md
expect_exit 10 "$MJ" prompt render bare
expect_grep 'no front matter'
rm .majordomus/prompts/mismatch.md .majordomus/prompts/nodesc.md .majordomus/prompts/extra.md .majordomus/prompts/bare.md

# {{CONTEXT}} embeds the assembled context, and context --prompt embeds the asset
printf -- '---\nname: withctx\ndescription: d\n---\nbefore\n{{CONTEXT}}\nafter\n' > .majordomus/prompts/withctx.md
expect_exit 0 "$MJ" prompt render withctx
expect_grep '^before$'
expect_grep '^## GIT'
expect_grep '^after$'
expect_exit 0 "$MJ" context --prompt probe
expect_grep '^## PROMPT probe'
expect_grep "task=fix the callback"
# a prompt cannot include the context it is being rendered into: the recursion is named
expect_exit 0 "$MJ" context --prompt withctx
expect_grep '^- prompt withctx — a prompt asset cannot include the context'
expect_exit 12 "$MJ" context --prompt nosuch
rm .majordomus/prompts/withctx.md

# the shipped assets all render against real state
for n in continue debug handover review; do expect_exit 0 "$MJ" prompt render "$n"; expect_no_grep '\{\{'; done

# ---------------------------------------------------------------- search
expect_exit 2 "$MJ" search
expect_grep 'a search term is required'
expect_exit 2 "$MJ" search x --kind nosuch
expect_grep "unknown kind 'nosuch'"
expect_exit 12 "$MJ" search "a phrase that appears nowhere at all"
expect_grep '^no match for'

printf 'reproduced the callback fault with the fixture\n' | "$MJ" checkpoint >/dev/null
printf '# Objective\nfix the callback\n# Current State\nhalf done\n# Next Action\nwrite the test\n' | "$MJ" handover >/dev/null

# finds the term in each kind, labelled, with a path and a line number
expect_exit 0 "$MJ" search "callback"
expect_grep '^checkpoint +\.majordomus/state/checkpoints/.*:[0-9]+ '
expect_grep '^handover +\.majordomus/state/handovers/.*:[0-9]+ '
expect_grep '^search: [0-9]+ match'
expect_exit 0 "$MJ" search "one store"
expect_grep '^decision +\.majordomus/state/decisions.md:[0-9]+'
expect_exit 0 "$MJ" search "old client"
expect_grep '^question +\.majordomus/state/open-questions.md:[0-9]+'
expect_exit 0 "$MJ" search "task.started" --kind history
expect_grep '^history +\.majordomus/state/ledger.jsonl:[0-9]+'
expect_exit 0 "$MJ" search "durable state" --kind prompt
expect_grep '^prompt +\.majordomus/prompts/continue.md:[0-9]+'

# case-insensitive and literal: a regular expression is matched as text, not as a pattern
expect_exit 0 "$MJ" search "CALLBACK"
expect_exit 12 "$MJ" search "call.ack"

# --kind restricts, and is repeatable
expect_exit 0 "$MJ" search "callback" --kind decision --kind checkpoint
expect_no_grep '^handover '
expect_exit 12 "$MJ" search "reproduced the callback" --kind decision

# --task narrows to one task's records
expect_exit 0 "$MJ" search "callback" --task "$id"
expect_grep '^checkpoint '
expect_exit 12 "$MJ" search "callback" --task t-nosuchtask-0000

# --json is one object per hit
if command -v jq >/dev/null; then
  "$MJ" --json search "callback" > s.json
  jq -e -s 'length > 0 and all(has("kind") and has("path") and has("line"))' s.json >/dev/null
fi

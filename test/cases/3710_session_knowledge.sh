# majordomus-covers: knowledge
# A closed session is a node of the knowledge graph that points at what its episode named, and
# the graph learns nothing from it but the fields that state those references (I0813).
#
# The four things this case holds:
#
#   which sessions touched an issue is a query, and every answer names the session record and
#   the field it was observed in, so nothing in it is inferred;
#
#   a session node carries its id, its window and its outcome and no word of the body below
#   its front matter;
#
#   a checkpoint, the most numerous record a checkout writes, is a reference the session holds
#   rather than one node per file;
#
#   a tracked session is shared (ADR 0014) while the handovers, checkpoints, tasks and ledger
#   entries it names are operational: the shared projection carries no node and no edge of an
#   operational kind, and the scope its sources declare is the only thing that decides that.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
"$MJ" update >/dev/null
mkdir -p lib .ai/repo/project/milestones .ai/repo/project/issues
echo a > lib/a
cat > .ai/repo/project/project.yaml <<'EOF'
schema_version: 1
name: Fixture
repository: example/fixture
default_branch: master
EOF
cat > .ai/repo/project/milestones/M100.yaml <<'EOF'
id: M100
title: An outcome
slug: an-outcome
order: 0
priority: p1
problem: "A problem."
outcome: "An outcome."
acceptance_criteria:
  - It is reached
validation:
  - "true"
evidence_required:
  - proof
EOF
cat > .ai/repo/project/issues/I100.yaml <<'EOF'
id: I100
milestone: M100
title: A piece of work
slug: a-piece-of-work
priority: p1
profile: implementation
objective: "Do the work."
scope:
  - lib
acceptance_criteria:
  - It is done
validation:
  - "true"
evidence_required:
  - proof
EOF
git add -A >/dev/null; git commit -qm base

# ---------------------------------------------------------------- the operational records
# Written by the tool, so they are what a real episode leaves under the state directory.
expect_exit 0 "$MJ" start "fixture work" --scope lib || exit 1
printf 'Halfway.\n' | "$MJ" checkpoint >/dev/null || { echo "    checkpoint failed"; exit 1; }
printf '# Objective\nFinish it.\n\n# Current State\nHalf done.\n\n# Next Action\nThe rest.\n' \
  | "$MJ" handover >/dev/null || { echo "    handover failed"; exit 1; }
cp_file="$(find .ai/local/state/checkpoints -name '*.md' | head -1)"
ho_file="$(find .ai/local/state/handovers -name '*.md' | head -1)"
[ -n "$cp_file" ] && [ -n "$ho_file" ] || { echo "    the fixture wrote no checkpoint or no handover"; exit 1; }
cp_id="$(basename "$cp_file" .md)"; ho_id="$(basename "$ho_file" .md)"
task_id="$(sed -n 's/^task_id: *//p' "$cp_file" | head -1)"
[ -n "$task_id" ] || { echo "    the checkpoint names no task"; exit 1; }

# ---------------------------------------------------------------- a closed session
head="$(git rev-parse HEAD)"
sid=s-20261020000000-ab12
rec=".ai/repo/sessions/20261020T000500Z--$sid--master--${head:0:7}--0123456789abcdef.md"
mkdir -p .ai/repo/sessions
cat > "$rec" <<EOF
---
schema: session/v1
kind: session
created_at: 2026-10-20T00:05:00Z
task_id: $task_id
profile: implementation
repository_id: https://github.com/example/fixture
worktree_id: 0123456789abcdef
branch: master
head: $head
working_tree: clean
changed_files:
session_id: $sid
started_at: 2026-10-20T00:00:00Z
closed_at: 2026-10-20T00:05:00Z
outcome: closed
title: "Session $sid on master"
start_head: $head
start_working_tree: clean
commits: []
tasks:
  - "$task_id"
issues:
  - "I100"
milestones:
  - "M100"
checkpoints:
  - "$cp_file"
handovers:
  - "$ho_file"
decisions:
  - "Chose the narrower design"
questions:
  - "Is the bound right?"
evidence: []
---

# What Happened

BODY-SENTINEL-NEVER-IN-THE-GRAPH: the composed body of the record.
EOF
git add -A >/dev/null; git commit -qm "a closed session" >/dev/null

# ---------------------------------------------------------------- which sessions touched it
expect_exit 0 "$MJ" knowledge related issue:I100 --scope shared \
  || { "$MJ" knowledge nodes --scope shared 2>&1 | grep -vE '^(policy|scope|context|profile|prompt|rule|skill|document) ' | tail -12; exit 1; }
expect_grep "^in +references +session:$sid +\\.ai/repo/sessions/[^ ]+:issues\\.0$"
expect_exit 0 "$MJ" knowledge related milestone:M100
expect_grep "^in +references +session:$sid +\\.ai/repo/sessions/[^ ]+:milestones\\.0$"

# ---------------------------------------------------------------- the node: id, window, outcome
expect_exit 0 "$MJ" knowledge nodes --kind session
expect_grep "session:$sid +Session $sid on master \\[2026-10-20T00:00:00Z\\.\\.2026-10-20T00:05:00Z, closed\\]"
expect_exit 0 "$MJ" knowledge nodes --scope all --json
expect_no_grep 'BODY-SENTINEL'
expect_exit 0 "$MJ" knowledge edges --scope all --json
expect_no_grep 'BODY-SENTINEL'

# ---------------------------------------------------------------- a checkpoint is a reference
# In the projection that holds operational records, the session reaches its checkpoint, its
# handover, its task and its ledger entries, each from the field that names it ...
expect_exit 0 "$MJ" knowledge edges --scope all
for want in "checkpoint:$cp_id .*:checkpoints\\.0" "handover:$ho_id .*:handovers\\.0" \
            "task:$task_id .*:tasks\\.0" "decision:Chose the narrower design .*:decisions\\.0" \
            "question:Is the bound right\\? .*:questions\\.0"; do
  expect_grep "^references +session:$sid +$want$"
done
# ... and the checkpoint is held there, not as a node of its own, while the file stays a
# discovered source
expect_exit 0 "$MJ" knowledge nodes --scope all --kind checkpoint
expect_grep '^knowledge nodes: 0 in scope all, kind checkpoint$'
expect_exit 0 "$MJ" knowledge sources --scope operational
expect_grep "$cp_id"

# ---------------------------------------------------------------- the shared projection
expect_exit 0 "$MJ" knowledge edges --scope shared
expect_grep "^references +session:$sid +issue:I100 "
expect_no_grep ' (handover|checkpoint|task|decision|question):'
expect_exit 0 "$MJ" knowledge nodes --scope shared --json
expect_no_grep '"kind":"(handover|checkpoint|decision|question)"'
expect_no_grep "$ho_id|$cp_id"

# the scope a source declares is the only thing that decides: declared shared, the handover
# kind is projected there and the session's edge to it appears with it
cp .ai/repo/knowledge/sources.yaml keep-sources.yaml
cat >> .ai/repo/knowledge/sources.yaml <<'SRC'

  - id: shared_handover
    kind: handover
    discovery: vcs
    pathspec: ':(glob)docs/handovers/*.md'
    required: false
SRC
expect_exit 0 "$MJ" knowledge edges --scope shared
expect_grep "^references +session:$sid +handover:$ho_id "
expect_no_grep ' checkpoint:'
cp keep-sources.yaml .ai/repo/knowledge/sources.yaml

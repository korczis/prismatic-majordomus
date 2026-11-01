# A closed session is a knowledge node whose edges point at what it touched (I0813): the
# graph answers which sessions touched an issue, without ever reading a session's body.
#
# Checkpoints and handovers are numerous and individually low-value, so a session names
# them by reference rather than each becoming its own node — proved here by their kind
# never producing a node merely because a session pointed at one.
#
# Sessions carry operational detail (who touched what, when) a shared projection must not
# surface, even though the closed record itself is a tracked file: the node's scope is
# operational, not shared, proved here too.
. "$ROOT/test/lib.sh"
mkdir -p "$T/tool"
fixture_repo "$T/tool" docs
MJ="$T/tool/bin/majordomus"

"$MJ" init >/dev/null

mkdir -p .ai/repo/project/milestones .ai/repo/project/issues .ai/repo/sessions
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
cat > .ai/repo/project/issues/I0101.yaml <<'EOF'
id: I0101
milestone: M100
title: An issue a session touched
slug: an-issue-a-session-touched
priority: p1
profile: implementation
objective: "Do the piece."
scope:
  - src/1
acceptance_criteria:
  - Done
validation:
  - "true"
EOF

cat > .ai/repo/sessions/20260101T000000Z--s-20260101000000-aaaa--master--abc1234--fixture0001.md <<'EOF'
---
schema: session/v1
kind: session
created_at: 2026-01-01T00:00:00Z
task_id: none
profile: implementation
session_id: s-20260101000000-aaaa
started_at: 2026-01-01T00:00:00Z
closed_at: 2026-01-01T01:00:00Z
outcome: completed
title: "A fixture session"
tasks:
  - "t-20260101000000-bbbb"
issues:
  - "I0101"
milestones: []
checkpoints:
  - ".ai/local/state/checkpoints/20260101T003000Z--master--abc1234--cafef00dcafef00d.md"
handovers:
  - ".ai/local/state/handovers/20260101T004500Z--master--abc1234--d00df00dd00df00d.md"
decisions:
  - "built the fixture this way"
questions:
  - "was the fixture built this way?"
evidence: []
---

MARKER-5a1b2c3d is a word from this session's body, never from its front matter: the case
greps the knowledge surfaces for it to prove none of them read past the front matter.
EOF
git add -A >/dev/null && git commit -q -m fixture

"$MJ" knowledge nodes --kind session --scope shared > shared.txt
expect_grep '^knowledge nodes: 0 in scope shared, kind session$' shared.txt

"$MJ" knowledge nodes --kind session --scope operational > operational.txt
expect_grep '^session +operational .* session:s-20260101000000-aaaa .*A fixture session$' operational.txt

"$MJ" knowledge edges --scope operational > edges.txt
PROV='\.ai/repo/sessions/20260101T000000Z--s-20260101000000-aaaa--master--abc1234--fixture0001\.md'
expect_grep "^references +session:s-20260101000000-aaaa +task:t-20260101000000-bbbb +$PROV:tasks\.0\$" edges.txt
expect_grep "^references +session:s-20260101000000-aaaa +issue:I0101 +$PROV:issues\.0\$" edges.txt
expect_grep "^references +session:s-20260101000000-aaaa +checkpoint:20260101T003000Z--master--abc1234--cafef00dcafef00d +$PROV:checkpoints\.0\$" edges.txt
expect_grep "^references +session:s-20260101000000-aaaa +handover:20260101T004500Z--master--abc1234--d00df00dd00df00d +$PROV:handovers\.0\$" edges.txt
expect_grep "^references +session:s-20260101000000-aaaa +decision:built the fixture this way +$PROV:decisions\.0\$" edges.txt
expect_grep "^references +session:s-20260101000000-aaaa +question:was the fixture built this way\? +$PROV:questions\.0\$" edges.txt

# A query for the issue names the session that touched it, carrying the session record.
expect_grep "^references +session:s-20260101000000-aaaa +issue:I0101 " edges.txt

# Checkpoints and handovers: referenced, never their own node — a high-volume kind does
# not dominate the graph because a reference list is sufficient (I0813's third criterion).
"$MJ" knowledge nodes --kind checkpoint --scope all > checkpoints.txt
expect_grep '^knowledge nodes: 0 in scope all, kind checkpoint$' checkpoints.txt
"$MJ" knowledge nodes --kind handover --scope all > handovers.txt
expect_grep '^knowledge nodes: 0 in scope all, kind handover$' handovers.txt

# The session's own body is never read for a node or an edge.
expect_no_grep "MARKER-5a1b2c3d" shared.txt
expect_no_grep "MARKER-5a1b2c3d" operational.txt
expect_no_grep "MARKER-5a1b2c3d" edges.txt

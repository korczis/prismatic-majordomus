# majordomus-covers: knowledge check
# A record derived under a task resolves in every clone, not only on the machine whose
# ledger holds the task (I2791).
#
# The deriver stamps a candidate with `task:<id>` (and a decision with `decision:<id>`) when a
# task was active, and both resolved only through the ledger and the task store, which are
# local: a fresh clone — every CI runner — refused every such candidate as dangling. The
# episode's tracked session record names its task in `task_id` and `tasks`, and both
# resolvers now read it. Held here, against the shell check and the Rust resolver alike:
#
#   1  a candidate citing a session and a task that no tracked record names fails the check
#   2  once the session record naming both is tracked, the check passes with no ledger at all
#   3  a task named only in the record's `tasks` list resolves too
#   4  `knowledge record` (the Rust resolver) answers the task through the session record
#   5  a task no record names still fails
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A >/dev/null 2>&1; git commit -qm "the layer" >/dev/null 2>&1 || true
# nothing local vouches for anything below: the ledger and the task store are absent
rm -rf .ai/local/state/ledger.jsonl .ai/local/state/completed .ai/local/state/current.yaml

CAN=.ai/repo/knowledge/candidates
SES=.ai/repo/sessions
mkdir -p "$CAN" "$SES"

# candidate ID TASK-REFERENCE
candidate() {
  {
    printf -- '---\nschema: knowledge/v1\nid: %s\nkind: knowledge\nclass: lesson\n' "$1"
    printf 'title: "A loop fed by a pipe loses its counter"\ndescription: "Observed in episode e7."\n'
    printf 'status: candidate\nepistemics: observed\ndate: 2026-10-17\ntags:\n  - derived\n'
    printf 'provenance:\n  origin: extracted\n  derived_from:\n    - session:s-20261017000000-e7e7\n    - %s\n' "$2"
    printf -- '---\n\n# A loop fed by a pipe loses its counter\n'
  } > "$CAN/$1.md"
}

candidate s-20261017000000-e7e7-aaaaaaaaaaaa task:t-20261017000000-aaaa
git add -A >/dev/null; git commit -qm "a candidate, derived elsewhere" >/dev/null

# ---------------------------------------------------------------- 1. nothing names it
expect_exit 10 "$MJ" knowledge check \
  || { echo "    a candidate citing an unknown session and task passed"; exit 1; }
expect_grep 'task:t-20261017000000-aaaa.*resolves to nothing' \
  || { echo "    the dangling task was not named"; exit 1; }

# ---------------------------------------------------------------- 2. the session record names both
cat > "$SES/20261017T000100Z--s-20261017000000-e7e7--feature-x--0000000--0000000000000000.md" <<'EOF'
---
schema: session/v1
kind: session
created_at: 2026-10-17T00:01:00Z
task_id: t-20261017000000-aaaa
profile: implementation
repository_id: fixture
worktree_id: a15e803315404067
branch: feature/x
head: 01227a0f74ab9de734edd3672dcc9d8d3a2d666c
working_tree: clean
changed_files:
session_id: s-20261017000000-e7e7
started_at: 2026-10-17T00:00:00Z
closed_at: 2026-10-17T00:01:00Z
outcome: closed
title: "Session s-20261017000000-e7e7 on feature/x"
start_head: 01227a0f74ab9de734edd3672dcc9d8d3a2d666c
start_working_tree: clean
commits: []
tasks:
  - "t-20261017000000-aaaa"
  - "t-20261017000000-bbbb"
issues: []
milestones: []
checkpoints: []
handovers: []
decisions: []
questions: []
evidence: []
---

# What Happened

A fixture episode.
EOF
git add -A >/dev/null; git commit -qm "the episode's session record" >/dev/null
expect_exit 0 "$MJ" knowledge check \
  || { echo "    a task the tracked session record names did not resolve without a ledger"; exit 1; }
expect_grep '^knowledge check: 1 record\(s\), 0 failing' || exit 1

# ---------------------------------------------------------------- 3. named in the list alone
candidate s-20261017000000-e7e7-bbbbbbbbbbbb task:t-20261017000000-bbbb
git add -A >/dev/null; git commit -qm "a second candidate" >/dev/null
expect_exit 0 "$MJ" knowledge check \
  || { echo "    a task named only in the record's tasks list did not resolve"; exit 1; }

# ---------------------------------------------------------------- 4. the Rust resolver agrees
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share" "$RB" knowledge record s-20261017000000-e7e7-bbbbbbbbbbbb --format json > record.json 2> record.err \
  || { echo "    knowledge record failed: $(cat record.err)"; exit 1; }
[ "$(jq -r '.resolved' record.json)" = true ] \
  || { echo "    the Rust resolver left a reference dangling: $(jq -c '.derived_from' record.json)"; exit 1; }
[ "$(jq -r '.derived_from[1].resolution' record.json)" = object ] \
  || { echo "    the task did not resolve through the session record: $(jq -c '.derived_from[1]' record.json)"; exit 1; }

# ---------------------------------------------------------------- 5. what nothing names still fails
candidate s-20261017000000-e7e7-cccccccccccc task:t-20261017000000-cccc
git add -A >/dev/null; git commit -qm "a candidate nothing vouches for" >/dev/null
expect_exit 10 "$MJ" knowledge check \
  || { echo "    a task no record names resolved"; exit 1; }
expect_grep 'task:t-20261017000000-cccc.*resolves to nothing' || exit 1

echo "    a task reference resolves through the tracked session record that names it, in shell and in Rust, and nothing else does"

# majordomus-covers: plan
# claims: issue-body-names-what-it-serves
# The body of an issue names what the issue serves (ADR 0116).
#
# An issue on GitHub said what it was and nothing of why it existed: the criterion it serves
# was in the record and invisible where the conversation happens. The generated body now
# lists what the record declares under `serves`, as authored and unjudged. It is inside the
# generated region, so the states the projection already has judge it: a changed mapping
# moves the hash and reads behind, and no second mechanism was needed.
#
# What it deliberately does not print is where a criterion stands. That moves with the
# evidence, and a body that moved whenever a run went stale would read behind on every
# branch with no record edited. Offline: the remote is a fixture file.
. "$ROOT/test/lib.sh"
SYNC="$ROOT/scripts/github-sync"
W="$(mktemp -d "${TMPDIR:-/tmp}/mj968.XXXXXX")"; trap 'rm -rf "$W"' EXIT
"$MJ" init >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
pj_init
pj_milestone M000
pj_issue I0001 M000
pj_issue I0002 M000
printf 'serves:\n  - probe#a\n  - probe#b\n' >> .ai/repo/project/issues/I0002.yaml

body() { "$SYNC" --render "$1" | sed '1d;$d' | grep -v '^<!-- majordomus:record '; }
hash_of() { "$SYNC" --render "$1" | sed -n 's/^<!-- majordomus:begin \([0-9a-f]*\) -->$/\1/p'; }
model() { find .ai/repo/project -type f -exec cksum {} + | sort -k3 | cksum; }

# --- an issue that declares what it serves says so, entry by entry, as the record states it
expect_exit 0 "$SYNC" --render I0002
expect_grep '^## Serves$'
expect_grep '^- `probe#a`$'
expect_grep '^- `probe#b`$'
expect_grep 'Where each criterion stands: `majordomus intent coverage`'
# the section sits between what is out of scope and what the issue depends on
order="$("$SYNC" --render I0002 | grep -nE '^## (Scope|Serves|Dependencies)$' | cut -d: -f2 | tr '\n' ' ')"
[ "$order" = "## Scope ## Serves ## Dependencies " ] || { echo "    the sections are in the order: $order"; exit 1; }

# --- the command line prints the same body the adapter would post
body I0002 > "$W/rendered"; "$MJ" plan body I0002 > "$W/cli"
diff -q "$W/rendered" "$W/cli" >/dev/null \
  || { echo "    the projection body and 'plan body' differ:"; diff "$W/rendered" "$W/cli" | head -5; exit 1; }

# --- an issue that serves nothing has no such section, and no sentence about one
body I0001 > "$W/none"
if grep -qE 'Serves|intent coverage' "$W/none"; then
  echo "    an issue that declares no serves has a section about it:"; grep -nE 'Serves|intent coverage' "$W/none"; exit 1
fi

# --- no state and no verdict is in the body: it would move with the evidence
sed -n '/^## Serves$/,/^## Dependencies$/p' "$W/rendered" > "$W/section"
if grep -qiE 'satisfied|current|stale|not_run|not run|failing|met\b' "$W/section"; then
  echo "    the Serves section states where a criterion stands:"; cat "$W/section"; exit 1
fi

# --- the mapping is part of what the body is generated from: changing it moves the hash,
#     and the remote that still holds the old body reads behind
before="$(hash_of I0002)"
[ -n "$before" ] || { echo "    no hash in the begin marker"; exit 1; }
row() { # <number> <title> <state> <record id> — a remote issue row; the body on stdin
  local b; b="$(cat | base64 | tr -d '\n')"
  printf '%s\t%s\t%s\t%s\tM000 — Milestone M000\t%s\n' "$1" "$2" "$3" "$b" "$4"
}
printf '1\tM000 — Milestone M000\topen\n' > "$W/ms.tsv"
{ "$SYNC" --render I0001 | row 7 'I0001 — Issue I0001' open I0001
  "$SYNC" --render I0002 | row 8 'I0002 — Issue I0002' open I0002; } > "$W/issues.tsv"
check() { MJ_GH_FIXTURE_ISSUES="$W/issues.tsv" MJ_GH_FIXTURE_MILESTONES="$W/ms.tsv" "$SYNC" --check; }
m0="$(model)"
expect_exit 0 check
expect_grep 'OK +insync +issue I0002'
sed 's/^  - probe#b$/  - probe#c/' .ai/repo/project/issues/I0002.yaml > "$W/i" && mv "$W/i" .ai/repo/project/issues/I0002.yaml
after="$(hash_of I0002)"
[ "$before" != "$after" ] || { echo "    changing what the issue serves did not move the body's hash"; exit 1; }
expect_exit 11 check
expect_grep 'DRIFT +behind +issue I0002'
expect_grep 'OK +insync +issue I0001'
# ...and declaring it for the first time does the same to an issue that had none
printf 'serves:\n  - probe#a\n' >> .ai/repo/project/issues/I0001.yaml
expect_exit 11 check
expect_grep 'DRIFT +behind +issue I0001'

# --- reading the remote wrote nothing into the canonical model
sed 's/^  - probe#c$/  - probe#b/' .ai/repo/project/issues/I0002.yaml > "$W/i" && mv "$W/i" .ai/repo/project/issues/I0002.yaml
sed '/^serves:$/,$d' .ai/repo/project/issues/I0001.yaml > "$W/i" && mv "$W/i" .ai/repo/project/issues/I0001.yaml
[ "$(model)" = "$m0" ] || { echo "    the fixture did not return to the model it started from"; exit 1; }
expect_exit 0 check
[ "$(model)" = "$m0" ] || { echo "    a check wrote into the canonical model"; exit 1; }

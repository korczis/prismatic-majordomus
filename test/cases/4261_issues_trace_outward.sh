# majordomus-covers: plan
# claims: github-projection
# An issue names what it answers to outside the layer, and a milestone the day it is due
# (I4261). An adopter's requirements lived in an external tracker, one issue with requirement
# ids, and its phases had client dates: the issue schema had no field for either reference,
# so they were written into `why`, and no date reached the forge's milestones.
#
#   `traces` is accepted and rendered in the issue body as written; a body without it is
#   unchanged;
#   `due_on` is accepted, shown in the milestone body, and changes no derived status;
#   github-sync --apply posts it as the forge milestone's due date.
#
# Offline: `gh` is a stub on PATH that answers the listings from fixture files and logs every
# call; the remote is a bare repository in the fixture.
. "$ROOT/test/lib.sh"
SYNC="$ROOT/scripts/github-sync"

"$MJ" init >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
pj_init
pj_milestone M000
pj_issue I0001 M000
pj_issue I0002 M000
git add -A && git commit -qm plan
"$MJ" plan body I0002 > "$T/plain.before"
"$MJ" plan status > "$T/status.before"

printf 'traces:\n  - PROJ-1\n  - A7\n  - https://tracker.example.org/browse/PROJ-1\n' >> .ai/repo/project/issues/I0001.yaml
printf 'due_on: 2026-11-11\n' >> .ai/repo/project/milestones/M000.yaml
git add -A && git commit -qm "traces and a due day"
expect_exit 0 "$MJ" plan validate

"$MJ" plan body I0001 > "$T/traced"
grep -q '^## Traces$' "$T/traced" && grep -q '^- PROJ-1$' "$T/traced" \
  && grep -q '^- https://tracker.example.org/browse/PROJ-1$' "$T/traced" \
  || { echo "    the traces did not reach the issue body:"; cat "$T/traced"; exit 1; }
"$MJ" plan body I0002 > "$T/plain.after"
cmp -s "$T/plain.before" "$T/plain.after" \
  || { echo "    a body with no traces changed:"; diff "$T/plain.before" "$T/plain.after"; exit 1; }
"$MJ" plan body M000 | grep -q '^| due | 2026-11-11 (declared) |$' \
  || { echo "    the due day is not in the milestone body"; "$MJ" plan body M000; exit 1; }
"$MJ" plan status > "$T/status.after"
cmp -s "$T/status.before" "$T/status.after" \
  || { echo "    a due day changed a derived status:"; diff "$T/status.before" "$T/status.after"; exit 1; }

# --- the projection: the missing milestone is created with its due date
STUB="$T/stub"; mkdir -p "$STUB"
MS_TSV="$T/ms.tsv"; IS_TSV="$T/issues.tsv"; LOG="$T/gh.log"
printf '/stub/\n/remote.git/\n/*.tsv\n/gh.log\n/plain.*\n/status.*\n/traced\n/apply.out\n' >> .git/info/exclude
cat > "$STUB/gh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$MJ_STUB_LOG"
case "$1 $2" in
  "auth status") exit 0 ;;
  "label list")  exit 0 ;;
esac
case "$*" in
  *"--paginate repos/"*"/milestones"*) cat "$MJ_STUB_MS"; exit 0 ;;
  *"--paginate repos/"*"/issues"*)     cat "$MJ_STUB_IS"; exit 0 ;;
  *"issue list"*)                      cat "$MJ_STUB_IS"; exit 0 ;;
  *"api graphql"*)                     wc -l < "$MJ_STUB_IS" | tr -d ' '; exit 0 ;;
esac
exit 0
STUB
chmod +x "$STUB/gh"
: > "$MS_TSV"; : > "$IS_TSV"
git init -q --bare "$T/remote.git"
git -C "$T/remote.git" fetch -q --no-prune "$T" HEAD:refs/heads/trunk
git -C "$T/remote.git" symbolic-ref HEAD refs/heads/trunk
git remote add origin "$T/remote.git"; git fetch -q origin; git remote set-head origin trunk >/dev/null
PATH="$STUB:$PATH" MJ_GH_PACE=0 MJ_STUB_MS="$MS_TSV" MJ_STUB_IS="$IS_TSV" MJ_STUB_LOG="$LOG" \
  "$SYNC" --apply > "$T/apply.out" 2>&1 || true
grep 'repos/example/fixture/milestones' "$LOG" | grep -- '-X POST' | grep -q 'due_on=2026-11-11T00:00:00Z' \
  || { echo "    the milestone was not created with its due date:"; cat "$LOG"; cat "$T/apply.out"; exit 1; }
echo "ok: traces reach the issue body, a due day reaches the forge and no status"

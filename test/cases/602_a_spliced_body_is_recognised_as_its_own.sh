# majordomus-covers: plan
# claims: github-projection
# A body the adapter splices into an existing issue is a body the adapter recognises as its
# own.
#
# scripts/github-sync composed the generated region twice: once for --render and once for
# the splice that --apply uses on an issue that already exists. The splice wrote the record
# marker without the newline that ends its line, so every body it posted carried the
# record's first line glued to the marker. region_intact() drops the marker line before it
# hashes the region back, and took that glued line with it, so the region never hashed to
# what its own marker claimed: every spliced issue was reported `edited` — a person's
# hand-edit — for ever, and --force rewrote it the same way. Case 45 reads the adapter's
# text; this case drives the splice itself.
#
# Offline: `gh` is a stub on PATH that answers the listings from fixture files and captures
# the body --apply posts. The captured body is then handed back to --check as the remote,
# and must come back insync.
. "$ROOT/test/lib.sh"
SYNC="$ROOT/scripts/github-sync"

"$MJ" init >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
pj_init
pj_milestone M000
pj_issue I0001 M000

STUB="$T/stub"; mkdir -p "$STUB"
MS_TSV="$T/ms.tsv"; IS_TSV="$T/issues.tsv"; POSTED="$T/posted.md"
printf '1\tM000 — Milestone M000\topen\n' > "$MS_TSV"
row() {
  b="$(cat | base64 | tr -d '\n')"
  printf '%s\t%s\topen\t%s\tM000 — Milestone M000\t%s\n' "$1" "$2" "$b" "$3"
}

# The transport, and nothing else: listings come from the fixture files, `issue edit`
# keeps the body it was given, every other mutation succeeds without doing anything.
cat > "$STUB/gh" <<'STUB'
#!/usr/bin/env bash
case "$1 $2" in
  "auth status") exit 0 ;;
  "label list")  exit 0 ;;
esac
case "$*" in
  *"--paginate repos/"*"/milestones"*) cat "$MJ_STUB_MS"; exit 0 ;;
  *"--paginate repos/"*"/issues"*)     cat "$MJ_STUB_IS"; exit 0 ;;
esac
if [ "$1 $2" = "issue edit" ]; then
  while [ $# -gt 0 ]; do
    [ "$1" = --body-file ] && { cp "$2" "$MJ_STUB_POSTED"; exit 0; }
    shift
  done
fi
exit 0
STUB
chmod +x "$STUB/gh"

# --- the remote carries the adapter's own earlier rendering, and the plan has moved since:
#     the record is `behind`, which is the state --apply splices without --force
"$SYNC" --render I0001 | row 7 'I0001 — Issue I0001' I0001 > "$IS_TSV"
sed 's/^objective: .*/objective: "Moved after it was projected."/' \
  .ai/repo/project/issues/I0001.yaml > "$T/i.yaml" && mv "$T/i.yaml" .ai/repo/project/issues/I0001.yaml

# --apply projects only a clean tree at the tip of the remote's default branch, so the moved
# plan is committed and published to a local bare remote as its trunk; the stub's files are
# no part of the tree.
printf '/stub/\n/remote.git/\n/*.tsv\n/posted.md\n' >> .git/info/exclude
git add -A >/dev/null; git commit -qm "the plan moves" >/dev/null
git init -q --bare "$T/remote.git"
# --no-prune: a global fetch.prune deletes a ref an explicit refspec has just written
git -C "$T/remote.git" fetch -q --no-prune "$T" HEAD:refs/heads/trunk
git -C "$T/remote.git" symbolic-ref HEAD refs/heads/trunk
git remote add origin "$T/remote.git"; git fetch -q origin; git remote set-head origin trunk >/dev/null

apply() {
  PATH="$STUB:$PATH" MJ_GH_PACE=0 MJ_STUB_MS="$MS_TSV" MJ_STUB_IS="$IS_TSV" \
    MJ_STUB_POSTED="$POSTED" "$SYNC" --apply
}
expect_exit 0 apply
expect_grep 'DRIFT +behind +issue I0001 \(#7\)'
expect_grep 'OK +updated +issue I0001 \(#7\)'
[ -s "$POSTED" ] || { echo "    --apply spliced no body into issue #7"; exit 1; }

# --- the marker is a whole line of its own in what was posted
rc=0; grep -qx '<!-- majordomus:record I0001 -->' "$POSTED" || rc=$?
[ "$rc" = 0 ] || { echo "    the posted body carries no record marker on a line of its own:"; \
                   grep -n 'majordomus:record' "$POSTED" || true; exit 1; }

# --- and the adapter reads what it posted back as its own: insync, not edited
row 7 'I0001 — Issue I0001' I0001 < "$POSTED" > "$IS_TSV"
check() { MJ_GH_FIXTURE_ISSUES="$IS_TSV" MJ_GH_FIXTURE_MILESTONES="$MS_TSV" "$SYNC" --check; }
expect_exit 0 check
expect_grep 'OK +insync +issue I0001 \(#7\)'
expect_grep 'github-sync check: 0 drift finding'

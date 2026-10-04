# majordomus-covers: plan
# claims: github-projection
# scripts/github-sync --apply projects the trunk and nothing else, one apply at a time, and
# every write it makes is a line saying what changed and why, or a failure that decides the
# exit.
#
# GitHub is shared and the trunk is the plan everybody shares. An apply had no guard: from a
# feature branch, or over uncommitted edits, it published that tree's plan as the shared one,
# and the live remote carried state, closed and unmanaged findings that came from a tree that
# was never the trunk. Close and reopen ended in `|| true`, so a write GitHub refused printed
# nothing and the run exited 0 over a half-applied remote. The adapter's own tree is checked
# as well: a branch's copy of it, run against a clean trunk, posted that branch's renderer.
#
# Offline: `gh` is a stub on PATH that answers the listings from fixture files, logs every
# call, and refuses the subcommand MJ_STUB_FAIL names. The remote is a bare repository in
# the fixture; no case here reaches the network.
. "$ROOT/test/lib.sh"
SYNC="$ROOT/scripts/github-sync"

"$MJ" init >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
pj_init
pj_milestone M000
pj_issue I0001 M000
pj_issue I0002 M000
"$MJ" plan start I0001 >/dev/null
"$MJ" plan evidence I0001 --covers proof --type test --command "true" --result ok >/dev/null
"$MJ" plan "done" I0001 >/dev/null

STUB="$T/stub"; mkdir -p "$STUB"
MS_TSV="$T/ms.tsv"; IS_TSV="$T/issues.tsv"; LOG="$T/gh.log"
printf '/stub/\n/remote.git/\n/*.tsv\n/gh.log\n' >> .git/info/exclude
cat > "$STUB/gh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$MJ_STUB_LOG"
if [ -n "${MJ_STUB_FAIL:-}" ] && [ "$1 $2" = "$MJ_STUB_FAIL" ]; then
  echo "HTTP 403: refused by the stub" >&2; exit 1
fi
case "$1 $2" in
  "auth status") exit 0 ;;
  "label list")  exit 0 ;;
esac
case "$*" in
  *"--paginate repos/"*"/milestones"*) cat "$MJ_STUB_MS"; exit 0 ;;
  *"--paginate repos/"*"/issues"*)     cat "$MJ_STUB_IS"; exit 0 ;;
esac
exit 0
STUB
chmod +x "$STUB/gh"

# The remote as the adapter itself would have left it, except for each issue's state: I0001
# is DONE and open there, I0002 is READY and closed there. An apply closes one, reopens the
# other, and writes no body.
row() {
  b="$(cat | base64 | tr -d '\n')"
  printf '%s\t%s\t%s\t%s\tM000 — Milestone M000\t%s\n' "$1" "$2" "$3" "$b" "$4"
}
printf '1\tM000 — Milestone M000\topen\n' > "$MS_TSV"
{ "$SYNC" --render I0001 | row 7 'I0001 — Issue I0001' open I0001
  "$SYNC" --render I0002 | row 8 'I0002 — Issue I0002' closed I0002; } > "$IS_TSV"

# the plan is committed and published to a bare remote as its default branch, `trunk`
git add -A >/dev/null; git commit -qm "the plan" >/dev/null
git init -q --bare "$T/remote.git"
# --no-prune: a global fetch.prune deletes a ref an explicit refspec has just written
git -C "$T/remote.git" fetch -q --no-prune "$T" HEAD:refs/heads/trunk
git -C "$T/remote.git" symbolic-ref HEAD refs/heads/trunk
git remote add origin "$T/remote.git"; git fetch -q origin; git remote set-head origin trunk >/dev/null
TRUNK="$(git rev-parse HEAD)"; HOME_BRANCH="$(git symbolic-ref --short HEAD)"
LOCK="$T/.git/majordomus-github-sync.lock"

apply() {
  PATH="$STUB:$PATH" MJ_GH_PACE=0 MJ_STUB_MS="$MS_TSV" MJ_STUB_IS="$IS_TSV" MJ_STUB_LOG="$LOG" \
    "$SYNC" --apply
}
no_gh_call() {
  [ ! -s "$LOG" ] || { echo "    a refused --apply called gh:"; cat "$LOG"; exit 1; }
}

# --- from a feature branch: refused before anything reads or writes the remote
git checkout -qb feature/elsewhere
git commit -q --allow-empty -m "a branch's own work"
: > "$LOG"
expect_exit 15 apply
expect_grep "refused: HEAD \([0-9a-f]{12} on feature/elsewhere\) is not the tip of origin/trunk"
expect_grep 'reproduce: git -C .* rev-parse HEAD refs/remotes/origin/trunk'
no_gh_call
# ...and the read-only modes still run from it
expect_exit 0 "$SYNC" --plan
expect_grep 'I0001 +closed +DONE'
fx_check() { MJ_GH_FIXTURE_ISSUES="$IS_TSV" MJ_GH_FIXTURE_MILESTONES="$MS_TSV" "$SYNC" --check; }
expect_exit 11 fx_check
expect_grep 'DRIFT +state +issue I0001 \(#7\) is open on GitHub, canonically closed'
git checkout -q "$HOME_BRANCH"
[ "$(git rev-parse HEAD)" = "$TRUNK" ] || { echo "    the fixture could not return to the trunk"; exit 1; }

# --- over uncommitted work, even an untracked file: refused
echo "a thought" > notes.txt
: > "$LOG"
expect_exit 15 apply
expect_grep 'refused: the tree has 1 uncommitted path\(s\) \(first: notes.txt\)'
expect_grep 'reproduce: git -C .* status --porcelain'
no_gh_call
rm -f notes.txt

# --- with no known default branch for the remote: refused
git remote set-head origin -d
: > "$LOG"
expect_exit 15 apply
expect_grep "refused: remote 'origin' has no known default branch"
no_gh_call
git remote set-head origin trunk >/dev/null

# --- the trunk moved on the remote and this checkout has not fetched: refused, because the
#     tracking ref is only this checkout's memory of the remote
git checkout -q -b ahead
git commit -q --allow-empty -m "somebody else's merge"
git -C "$T/remote.git" fetch -q --no-prune "$T" ahead:refs/heads/trunk
git checkout -q "$HOME_BRANCH"; git branch -q -D ahead
: > "$LOG"
expect_exit 15 apply
expect_grep 'refused: origin/trunk is at [0-9a-f]{12} on the remote and this tree is at [0-9a-f]{12}; the local view of the trunk is stale'
expect_grep 'reproduce: git -C .* ls-remote origin refs/heads/trunk'
no_gh_call
git -C "$T/remote.git" update-ref refs/heads/trunk "$TRUNK"

# --- while another apply holds the lock: refused, and the holder is named
mkdir "$LOCK"; printf 'host=elsewhere pid=4242 head=%s since=then\n' "$TRUNK" > "$LOCK/owner"
: > "$LOG"
expect_exit 15 apply
expect_grep 'refused: another --apply holds .*majordomus-github-sync.lock \(host=elsewhere pid=4242 '
no_gh_call
[ -d "$LOCK" ] || { echo "    a refused apply removed a lock it did not hold"; exit 1; }
rm -rf "$LOCK"

# --- at the trunk, clean: every write is a line naming what changed and why
: > "$LOG"
expect_exit 0 apply
expect_grep '^github-sync apply: example/fixture at [0-9a-f]{12} \(origin/trunk\)'
expect_grep 'OK +closed +issue I0001 \(#7\) — canonically DONE'
expect_grep 'OK +reopened +issue I0002 \(#8\) — canonically READY'
expect_grep 'OK +updated +milestone M000 \(#1\) — description rewritten from the canonical record'
expect_no_grep 'OK +updated +issue I000[12] \(#[78]\) — body'
expect_no_grep '^FAIL'
grep -qx 'issue close 7 --repo example/fixture' "$LOG" || { echo "    no close of #7 was sent:"; cat "$LOG"; exit 1; }
grep -qx 'issue reopen 8 --repo example/fixture' "$LOG" || { echo "    no reopen of #8 was sent:"; cat "$LOG"; exit 1; }
[ ! -e "$LOCK" ] || { echo "    the apply left its lock behind"; exit 1; }

# --- a write GitHub refuses is reported, the run goes on, and the exit says so. Close and
#     reopen used to end in `|| true`: this run printed nothing about #7 and exited 0.
: > "$LOG"
fail_close() { MJ_STUB_FAIL="issue close" apply; }
expect_exit 13 fail_close
expect_grep "FAIL +closed +issue I0001 \(#7\) — canonically DONE: 'gh issue close 7 --repo example/fixture' exited 1: HTTP 403: refused by the stub"
expect_grep 'OK +reopened +issue I0002 \(#8\)'
expect_grep '1 mutation\(s\) failed'
[ ! -e "$LOCK" ] || { echo "    a failed apply left its lock behind"; exit 1; }

fail_reopen() { MJ_STUB_FAIL="issue reopen" apply; }
expect_exit 13 fail_reopen
expect_grep "FAIL +reopened +issue I0002 \(#8\) — canonically READY: 'gh issue reopen 8"
expect_grep 'OK +closed +issue I0001 \(#7\)'

# a milestone edit GitHub refuses is a failure too, not a silent `&&` that never ran
fail_api() { MJ_STUB_FAIL="api -X" apply; }
expect_exit 13 fail_api
expect_grep 'FAIL +updated +milestone M000 \(#1\)'

# a creation GitHub refuses is a FAIL line too: under `set -e` the bare assignment ended the
# run there, with exit 1 and no word about the records after it
grep -v 'I0002 — ' "$IS_TSV" > "$T/one.tsv"
fail_create() {
  PATH="$STUB:$PATH" MJ_GH_PACE=0 MJ_STUB_MS="$MS_TSV" MJ_STUB_IS="$T/one.tsv" MJ_STUB_LOG="$LOG" \
    MJ_STUB_FAIL="issue create" "$SYNC" --apply
}
expect_exit 13 fail_create
expect_grep "FAIL +created +issue I0002 — the canonical record has no counterpart on GitHub: 'gh issue create --repo example/fixture' exited 1: HTTP 403: refused by the stub"
expect_grep 'OK +closed +issue I0001 \(#7\)'
[ ! -e "$LOCK" ] || { echo "    a failed apply left its lock behind"; exit 1; }

# --- a body and a milestone assignment that differ are written, each with its own line, and
#     a refused `issue edit` is a FAIL for each. I0002 here is as an early projection left
#     it: open, no identity marker, no milestone.
ED_TSV="$T/edit.tsv"
{ grep 'I0001 — ' "$IS_TSV"
  printf '8\tI0002 — Issue I0002\topen\t%s\t\t\n' "$(printf 'written before the marker\n' | base64 | tr -d '\n')"; } > "$ED_TSV"
apply_edit() {
  PATH="$STUB:$PATH" MJ_GH_PACE=0 MJ_STUB_MS="$MS_TSV" MJ_STUB_IS="$ED_TSV" MJ_STUB_LOG="$LOG" \
    "$SYNC" --apply
}
: > "$LOG"
expect_exit 0 apply_edit
expect_grep 'OK +updated +issue I0002 \(#8\) milestone — set to .M000 — Milestone M000., its canonical milestone'
expect_grep 'OK +updated +issue I0002 \(#8\) — body given its identity marker; it was matched by title'
grep -q '^issue edit 8 --repo example/fixture --body-file ' "$LOG" || { echo "    no body edit of #8 was sent:"; cat "$LOG"; exit 1; }
grep -qx 'issue edit 8 --repo example/fixture --milestone M000 — Milestone M000' "$LOG" \
  || { echo "    no milestone edit of #8 was sent:"; cat "$LOG"; exit 1; }
fail_edit() { MJ_STUB_FAIL="issue edit" apply_edit; }
expect_exit 13 fail_edit
expect_grep "FAIL +updated +issue I0002 \(#8\) milestone — set to .M000 — Milestone M000., its canonical milestone: 'gh issue edit 8 --repo example/fixture --milestone M000 — Milestone M000' exited 1: HTTP 403"
expect_grep "FAIL +updated +issue I0002 \(#8\) — body given its identity marker; it was matched by title: 'gh issue edit 8 --repo example/fixture --body-file "
expect_grep 'OK +closed +issue I0001 \(#7\)'
expect_grep '2 mutation\(s\) failed'

# --- the adapter's own tree is projected too: its renderer, hashes and state mapping. A
#     feature branch's copy run with its cwd in the clean trunk passed every check above.
#     Here the adapter is loaded from a worktree of the same repository on a feature branch.
printf '/wt/\n' >> .git/info/exclude
git worktree add -q -b feature/adapter "$T/wt" "$TRUNK"
mkdir -p "$T/wt/scripts"
ln -s "$SYNC" "$T/wt/scripts/github-sync"
ln -s "$ROOT/lib" "$T/wt/lib"; ln -s "$ROOT/bin" "$T/wt/bin"; ln -s "$ROOT/share" "$T/wt/share"
git -C "$T/wt" add -A >/dev/null; git -C "$T/wt" commit -qm "a branch's own adapter" >/dev/null
apply_from_wt() {
  PATH="$STUB:$PATH" MJ_GH_PACE=0 MJ_STUB_MS="$MS_TSV" MJ_STUB_IS="$IS_TSV" MJ_STUB_LOG="$LOG" \
    "$T/wt/scripts/github-sync" --apply
}
: > "$LOG"
expect_exit 15 apply_from_wt
expect_grep "refused: the adapter runs from .*/wt \([0-9a-f]{12} on feature/adapter\), not from the trunk at [0-9a-f]{12}"
expect_grep 'reproduce: git -C .*/wt rev-parse HEAD'
no_gh_call
# ...and once that tree is the trunk, at its tip and clean, it may project it
git merge -q --ff-only feature/adapter
git -C "$T/remote.git" fetch -q --no-prune "$T" HEAD:refs/heads/trunk
git fetch -q origin
: > "$LOG"
expect_exit 0 apply_from_wt
expect_grep 'OK +closed +issue I0001 \(#7\)'
# ...but not with uncommitted edits in it
echo "an edit" > "$T/wt/notes.txt"
: > "$LOG"
expect_exit 15 apply_from_wt
expect_grep "refused: the adapter runs from .*/wt \([0-9a-f]{12} on feature/adapter, with uncommitted changes\)"
no_gh_call

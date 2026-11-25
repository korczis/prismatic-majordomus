# majordomus-covers: plan
# majordomus-negative: plan
# Evidence records the trunk commit it is given (I4410). `plan evidence` wrote HEAD into the
# entry's commit: and nothing else, so evidence recorded on a branch named a branch commit the
# trunk never reaches. OSCILLA's own test (V272 D4) caught that repeatedly, and the only way
# to stamp a commit of main was a fresh branch per record. `--commit <rev>` names the commit;
# it is accepted only when the trunk's remote-tracking branch reaches it, and a clone that
# cannot judge (shallow, or no refs/remotes/<remote>/HEAD) writes nothing.
#
# What it proves, in order:
#   1  on a branch two commits ahead of origin/main, --commit origin/main~1 records the full
#      sha of origin/main~1, not HEAD, and the entry is otherwise the entry of today
#   2  two such records for two issues on one branch, committed in one commit, both carry
#      their named trunk commits
#   3  --commit HEAD (a commit origin/main does not reach) exits 15, names the commit and
#      refs/remotes/origin/main, and leaves the issue file unchanged
#   4  an unknown revision exits 2, and so does --commit without a value
#   5  without refs/remotes/origin/HEAD the commit cannot be judged: refused, nothing written
#   6  in a shallow clone the commit cannot be judged: refused, nothing written
#   7  without --commit the entry records HEAD, byte for byte what it recorded before
#   8  the plan_evidence ledger event carries the recorded commit and its source
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
pj_init
pj_milestone M000
pj_issue I0001 M000
pj_issue I0002 M000
pj_issue I0003 M000
git add -A >/dev/null
git commit -qm "the plan"
git branch -M main

# --- a bare origin whose refs/remotes/origin/HEAD names main, and a trunk with history
git commit -q --allow-empty -m "trunk: the first landing"
git commit -q --allow-empty -m "trunk: the second landing"
git init -q --bare "$T/remote.git"
git remote add origin "$T/remote.git"
git push -q -u origin main
git remote set-head origin main >/dev/null
TRUNK1="$(git rev-parse origin/main~1)"
TRUNK0="$(git rev-parse origin/main)"

# --- a branch two commits ahead of the trunk
git checkout -q -b feature/stamps
git commit -q --allow-empty -m "branch: one"
git commit -q --allow-empty -m "branch: two"
BRANCH_HEAD="$(git rev-parse HEAD)"

S="$T/.git/case4778"; mkdir -p "$S"
F1=.ai/repo/project/issues/I0001.yaml
F2=.ai/repo/project/issues/I0002.yaml
F3=.ai/repo/project/issues/I0003.yaml
cp "$F1" "$S/I0001.before"; cp "$F2" "$S/I0002.before"; cp "$F3" "$S/I0003.before"
export MAJORDOMUS_NOW=2026-11-25T10:00:00Z
L=.ai/local/state/ledger.jsonl

# golden <file before> <commit> [result] — the record plan evidence writes today, with the
# commit it names: the issue's file, its updated_at, then one entry
golden() {
  cat "$1"
  printf 'updated_at: %s\nevidence:\n' "$MAJORDOMUS_NOW"
  printf '  - covers: "proof"\n    type: manual\n    command: "x"\n'
  [ -z "${3:-}" ] || printf '    result: "%s"\n' "$3"
  printf '    commit: %s\n    recorded_at: %s\n' "$2" "$MAJORDOMUS_NOW"
}
same_bytes() {
  cmp -s "$1" "$2" || { echo "    $2 is not the expected record:"; diff "$1" "$2" || true; exit 1; }
}
unchanged() {
  cmp -s "$S/$(basename "$1" .yaml).before" "$1" \
    || { echo "    $1 changed although the record was refused:"; cat "$1"; exit 1; }
}

# --- 1  a named trunk commit is recorded as its full sha, not HEAD
expect_exit 0 "$MJ" plan evidence I0001 --covers proof --type manual --command x \
  --commit origin/main~1
grep -qxF "    commit: $TRUNK1" "$F1" \
  || { echo "    the entry does not name origin/main~1 ($TRUNK1):"; cat "$F1"; exit 1; }
expect_no_grep "$BRANCH_HEAD" "$F1"
golden "$S/I0001.before" "$TRUNK1" > "$S/I0001.want"
same_bytes "$S/I0001.want" "$F1"

# --- 2  a second issue on the same branch, by an abbreviated name; both in one commit
expect_exit 0 "$MJ" plan evidence I0002 --covers proof --type manual --command x \
  --commit "${TRUNK0:0:12}"
golden "$S/I0002.before" "$TRUNK0" > "$S/I0002.want"
same_bytes "$S/I0002.want" "$F2"
git add "$F1" "$F2"
git commit -qm "evidence: two stamps at their trunk commits"
[ "$(git diff --name-only HEAD~1 HEAD | sort | tr '\n' ' ')" = "$F1 $F2 " ] \
  || { echo "    the commit does not carry exactly the two records"; exit 1; }
git show "HEAD:$F1" | grep -qxF "    commit: $TRUNK1" || { echo "    I0001 lost its commit"; exit 1; }
git show "HEAD:$F2" | grep -qxF "    commit: $TRUNK0" || { echo "    I0002 lost its commit"; exit 1; }
BRANCH_HEAD="$(git rev-parse HEAD)"

# --- 3  a commit the trunk does not reach is refused, named, and nothing is written
expect_exit 15 "$MJ" plan evidence I0003 --covers proof --type manual --command x --commit HEAD
expect_grep "$BRANCH_HEAD"
expect_grep 'refs/remotes/origin/main'
unchanged "$F3"

# --- 4  an unknown revision is a usage error, and so is a missing value
expect_exit 2 "$MJ" plan evidence I0003 --covers proof --type manual --command x \
  --commit no-such-revision
expect_grep 'no-such-revision'
unchanged "$F3"
expect_exit 2 "$MJ" plan evidence I0003 --covers proof --type manual --command x --commit
unchanged "$F3"

# --- 5  no refs/remotes/origin/HEAD: no trunk is known, so the commit cannot be judged
git remote set-head origin -d >/dev/null
expect_exit 15 "$MJ" plan evidence I0003 --covers proof --type manual --command x \
  --commit "$TRUNK1"
expect_grep 'refs/remotes/<remote>/HEAD|no trunk'
unchanged "$F3"
git remote set-head origin main >/dev/null

# --- 6  a shallow clone cannot judge reachability: refused, nothing written
git clone -q --depth 1 "file://$T/remote.git" "$T/shallow"
cd "$T/shallow" || exit 1
[ "$(git rev-parse --is-shallow-repository)" = true ] || { echo "    the clone is not shallow"; exit 1; }
cp "$F1" "$S/shallow.before"
expect_exit 15 "$MJ" plan evidence I0001 --covers proof --type manual --command x \
  --commit origin/main
expect_grep 'shallow'
cmp -s "$S/shallow.before" "$F1" || { echo "    the shallow clone's record changed"; exit 1; }
cd "$T" || exit 1

# --- 7  without --commit the entry records HEAD, exactly as before
expect_exit 0 "$MJ" plan evidence I0003 --covers proof --type manual --command x --result ok
golden "$S/I0003.before" "$BRANCH_HEAD" ok > "$S/I0003.want"
same_bytes "$S/I0003.want" "$F3"

# --- 8  the ledger carries the commit and where it came from; the refusals wrote no line
grep -qF "\"issue\":\"I0001\",\"covers\":\"proof\",\"type\":\"manual\",\"commit\":\"$TRUNK1\",\"commit_source\":\"named\"" "$L" \
  || { echo "    no named plan_evidence line for I0001:"; grep plan_evidence "$L"; exit 1; }
grep -qF "\"issue\":\"I0002\",\"covers\":\"proof\",\"type\":\"manual\",\"commit\":\"$TRUNK0\",\"commit_source\":\"named\"" "$L" \
  || { echo "    no named plan_evidence line for I0002:"; grep plan_evidence "$L"; exit 1; }
grep -qF "\"issue\":\"I0003\",\"covers\":\"proof\",\"type\":\"manual\",\"commit\":\"$BRANCH_HEAD\",\"commit_source\":\"head\"" "$L" \
  || { echo "    no head plan_evidence line for I0003:"; grep plan_evidence "$L"; exit 1; }
[ "$(grep -c '"event":"plan_evidence"' "$L")" = 3 ] \
  || { echo "    expected three plan_evidence lines:"; grep plan_evidence "$L"; exit 1; }

expect_exit 0 "$MJ" plan validate

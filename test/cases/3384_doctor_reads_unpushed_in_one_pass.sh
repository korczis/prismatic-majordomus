# majordomus-covers: none
# doctor counts the work no remote holds in one walk, whatever the number of branches (I2993).
#
# `clone unpushed` ran one `git rev-list --count <branch> --not --remotes` per local branch.
# On a clone of six hundred branches that was minutes of every commit hook, in every seat
# (828 s for one doctor on 2026-10-18 against a budget of 100 s). What is decided here:
#
#   1. a branch that is on a remote is not named, a branch two commits ahead is named with 2,
#      a branch that merged another unpushed branch counts each commit it reaches once, and a
#      second branch at the same commit is counted alike;
#   2. the answer is the one the per-branch count gave, branch for branch;
#   3. the clone is asked once, not once per branch: a hundred and fifty more branches add no
#      walk of the commits no remote holds.
. "$ROOT/test/lib.sh"

REAL_GIT="$(command -v git)"
"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install
git init -q --bare "$T/remote.git"
git remote add origin "$T/remote.git"
git push -q origin HEAD:refs/heads/master

# ---------------------------------------------------------------- the branches
base="$(git rev-parse HEAD)"
git branch pushed "$base" && git push -q origin pushed
git checkout -q -b ahead "$base"
echo a > a.txt && git add a.txt && git commit -qm a1
echo b > b.txt && git add b.txt && git commit -qm a2
git checkout -q -b side "$base"
echo s > s.txt && git add s.txt && git commit -qm s1
git checkout -q -b merged ahead
git merge -q --no-edit side
git branch twin merged
git checkout -q master 2>/dev/null || git checkout -q -
i=0
while [ "$i" -lt 150 ]; do git branch "many-$i" "$base"; i=$((i + 1)); done

# ---------------------------------------------------------------- a git that counts its walks
mkdir -p "$T/shim"
cat > "$T/shim/git" <<EOF
#!/bin/sh
case " \$* " in *" --not --remotes "*) echo walk >> "$T/walks.log" ;; esac
exec "$REAL_GIT" "\$@"
EOF
chmod +x "$T/shim/git"
: > "$T/walks.log"
PATH="$T/shim:$PATH" "$MJ" doctor > doctor.txt 2>&1 || true
line="$(grep -E '(WARN|OK) +clone +unpushed' doctor.txt || true)"
[ -n "$line" ] || { echo "    doctor said nothing about unpushed work:"; tail -20 doctor.txt | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- 1. what it names
for want in ' ahead(2)' ' side(1)' ' merged(4)' ' twin(4)'; do
  case "$line" in *"$want"*) ;; *) echo "    the unpushed line lacks$want: $line"; exit 1 ;; esac
done
case "$line" in *" pushed("*|*" many-"*|*" master("*) echo "    a branch a remote holds is named: $line"; exit 1 ;; esac
echo "    a branch ahead, a side branch, the branch that merged it and its twin are named with their counts"

# ---------------------------------------------------------------- 2. the per-branch answer
expected=""
for b in $(git for-each-ref --format='%(refname:short)' refs/heads); do
  n="$(git rev-list --count "$b" --not --remotes)"
  [ "$n" -gt 0 ] && expected="$expected $b($n)"
done
case "$line" in *"commits on no remote:$expected — "*) ;; *)
  echo "    the counts are not the per-branch counts:"
  echo "    | doctor:   $line"
  echo "    | expected: $expected"
  exit 1 ;;
esac
echo "    every count is the one a per-branch rev-list gives"

# ---------------------------------------------------------------- 3. one walk
walks="$(wc -l < "$T/walks.log" | tr -d ' ')"
[ "$walks" = 1 ] || { echo "    doctor walked the commits no remote holds $walks times for $(git for-each-ref refs/heads | wc -l | tr -d ' ') branches"; exit 1; }
echo "    one walk for $(git for-each-ref refs/heads | wc -l | tr -d ' ') branches"
echo "    doctor counts the work no remote holds in one walk"

# An adopter's commit message is judged against the grammar its policy declares, at commit
# time, and a published commit whose message must change is superseded, never rewritten.
#
# The motivating adopter keeps its upstream's grammar, `<Keyword>( #<issue>|<hash>, ...)?:
# [<Component>] <Details>`. Before `commit.grammar` existed, `commit validate` refused every
# valid message of it, so its `commit-msg` hook was never wired. A word outside the grammar
# (`Test:`) was then found by the forge's checker after the push, and the force push the fix
# needed was refused. Everything here runs against a real repository, the real hook and a
# real bare remote.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
LAST_OUT="$("$MJ" init 2>&1)"

echo "  init offers the commit-msg line, with the measurement to take before wiring it"
expect_grep 'commit-msg \(optional, doctor does not require it\): .* commit validate "\$1" \|\| exit \$\?'
expect_grep 'commit history HEAD~50\.\.HEAD'
expect_grep 'commit\.grammar'

git init -q . 2>/dev/null || true
git config user.email t@example.org
git config user.name Tester
git config commit.gpgsign false
# Scratch files of the case itself stay out of every commit it makes.
mkdir -p .scratch && printf '/.scratch/\n' >> .git/info/exclude
commit() { git add -A; git -c core.hooksPath=/dev/null commit -q --no-verify -m "$1"; }

echo "  a keyword repository's valid message is refused while no grammar is declared"
expect_exit 10 "$RB" commit validate --repo . <<'EOF'
Fix #12: [YAPF] Infinite loop in pathfinder
EOF
expect_grep 'commit\.not_conventional'

# The grammar, declared where the policy's commit block is.
awk '{ print } /^commit:$/ { print "  grammar:\n    style: keyword\n    words:\n      - word: Fix\n        kind: fix\n      - word: Codechange\n        kind: refactor\n      - word: Doc\n        kind: docs" }' \
  .ai/repo/policy.yaml > .scratch/policy && cp .scratch/policy .ai/repo/policy.yaml
grep -q '^  grammar:$' .ai/repo/policy.yaml || { echo "    the fixture's policy has no commit block to declare a grammar in"; exit 1; }
LAST_OUT="$("$MJ" doctor 2>&1)" || true
expect_no_grep 'unknown key.*commit\.'

echo "  declared, the same message passes, with the upstream's own full stop"
expect_exit 0 "$RB" commit validate --repo . <<'EOF'
Fix #12: [YAPF] Infinite loop in pathfinder.
EOF
expect_grep 'nothing to report'

echo "  a word the grammar does not hold is refused, naming the words it does"
expect_exit 10 "$RB" commit validate --repo . <<'EOF'
Test: [PrismaticMCP] The approver runs under a fake panel
EOF
expect_grep 'commit\.unknown_word'
expect_grep "'Test' is not a word of this repository's commit grammar; the words are Fix, Codechange, Doc"

echo "  a malformed prefix and a line the upstream checker refuses are each named"
expect_exit 10 "$RB" commit validate --repo . <<'EOF'
Fix #12, #13: two issues without their word
EOF
expect_grep 'commit\.header_malformed'
expect_exit 10 "$RB" commit validate --repo . < <(printf 'Fix: [Net] A thing \n\nA\ttab\n')
expect_grep 'commit\.trailing_whitespace.*line 1 ends in whitespace'
expect_grep 'commit\.non_ascii.*line 3, column 2: U\+0009'

echo "  the commit-msg hook refuses the commit while git still holds it"
mkdir -p .scratch/hooks
printf '#!/bin/sh\n"%s" commit validate --repo "%s" "$1" || exit $?\n' "$RB" "$PWD" > .scratch/hooks/commit-msg
chmod +x .scratch/hooks/commit-msg
echo a > a.txt; git add -A
expect_exit 1 git -c core.hooksPath=.scratch/hooks commit -q -m "Test: [Net] A thing"
expect_grep 'commit\.unknown_word'
expect_exit 0 git -c core.hooksPath=.scratch/hooks commit -q -m "Codechange: [Net] A thing"

echo "  the scope vocabulary learns the components of the history"
echo y1 > y1.txt; commit "Fix #1: [YAPF] One"
echo y2 > y2.txt; commit "Codechange: [YAPF] Two"
expect_exit 0 "$RB" commit scopes --repo .
expect_grep 'YAPF'

echo "  a pushed commit with a refused message is superseded on a new branch"
git init -q --bare .scratch/remote.git
git remote add origin "$PWD/.scratch/remote.git"
BASE="$(git rev-parse HEAD)"
echo c > c.txt; commit "Test: [Net] A pushed mistake"
BAD="$(git rev-parse HEAD)"
echo d > d.txt; commit "Fix #3: [Net] After the mistake"
git push -q origin HEAD:refs/heads/feature/x
OLD="$(git rev-parse HEAD)"
expect_exit 10 "$RB" commit reword --repo . "$BASE..HEAD"
expect_grep "FAIL .* Test: \[Net\] A pushed mistake"
printf 'Codechange: [Net] A pushed mistake, reworded\n' > .scratch/reworded.txt
printf 'Why the change was made.\n\nSuperseded by #99\n' > .scratch/old.md
expect_exit 0 "$RB" commit reword --repo . "$BASE..HEAD" --message "$BAD=.scratch/reworded.txt" \
  --supersedes 7 --url https://example.org/pull/7 --body .scratch/old.md --body-out .scratch/succ.md \
  --apply --branch feature/x-reworded
expect_grep 'created  feature/x-reworded'
expect_grep 'git push -u origin feature/x-reworded'
expect_grep 'gh pr merge 7 --disable-auto; gh pr ready 7 --undo'
[ "$(git rev-parse HEAD)" = "$OLD" ] || { echo "    the current branch moved"; exit 1; }
[ "$(git ls-remote origin refs/heads/feature/x | cut -f1)" = "$OLD" ] || { echo "    the published branch moved"; exit 1; }
[ "$(git rev-parse "feature/x-reworded^{tree}")" = "$(git rev-parse "$OLD^{tree}")" ] || { echo "    the replay carries another tree"; exit 1; }
[ "$(git log -1 --format=%s feature/x-reworded~1)" = "Codechange: [Net] A pushed mistake, reworded" ] || { echo "    the message was not reworded"; exit 1; }
expect_exit 0 "$RB" commit history --repo . "$BASE..feature/x-reworded"
head -n 1 .scratch/succ.md | grep -qx 'Supersedes #7' || { echo "    the successor body does not open with its marker"; cat .scratch/succ.md; exit 1; }
grep -q 'Why the change was made.' .scratch/succ.md || { echo "    the old body was not carried"; exit 1; }
! grep -q 'Superseded by #99' .scratch/succ.md || { echo "    the old body's own successor line was carried"; exit 1; }

echo "  a second replay onto the same branch is refused and moves nothing"
expect_exit 13 "$RB" commit reword --repo . "$BASE..$OLD" --message "$BAD=.scratch/reworded.txt" \
  --apply --branch feature/x-reworded
expect_grep 'exists'

echo "  where the policy requires an issue and refuses a closing keyword, both are judged"
sed -e 's/^  reference_required: off$/  reference_required: error/' \
    -e 's/^  closing_keyword: off$/  closing_keyword: error/' .ai/repo/policy.yaml > .scratch/policy
cp .scratch/policy .ai/repo/policy.yaml
grep -q '^  reference_required: error$' .ai/repo/policy.yaml || { echo "    the fixture's policy has no reference_required to set"; exit 1; }
expect_exit 10 "$RB" commit validate --repo . <<'MSG'
Codechange: [Net] A change that names no issue
MSG
expect_grep 'commit\.reference_missing'
expect_exit 10 "$RB" commit validate --repo . <<'MSG'
Doc #4: [Net] A page

Closes #4
MSG
expect_grep "commit\.closing_keyword.*'Closes #4'"
expect_no_grep 'commit\.reference_missing'

# majordomus-covers: init finish
# majordomus-negative: init
#
# The pre-push hook init prints for adopters passes `finish --check` the commits the push
# publishes from this branch (I4619). I4610 taught the check to compare a pushed tree with
# the tree the branch's last finished task verified, and this repository's own pre-push
# reads git's stdin for it; the one line init printed compared HEAD, which is not what
# `git push origin <commit>:<branch>` publishes. The hook here is whatever init prints,
# installed as it is and driven with git's pre-push stdin, so the one line fails this case:
# it warns for a verified commit pushed from behind an unverified HEAD.
. "$ROOT/test/lib.sh"
unset MAJORDOMUS_PUSHED
expect_exit 0 "$MJ" init
init_out="$LAST_OUT"
"$MJ" update >/dev/null

# The pre-push advice, as a person would copy it: the indented block under `pre-push:`, or
# the text after it on the same line when there is no block.
hook_body() {
  awk '
    /^  pre-push: / { rest = $0; sub(/^  pre-push: +/, "", rest)
                      if (rest ~ /these lines/) { block = 1; next } print rest; exit }
    block && /^    / { sub(/^    /, ""); print; next }
    block { exit }'
}
body="$(printf '%s\n' "$init_out" | hook_body)"
[ -n "$body" ] || { printf '    init printed no pre-push advice:\n%s\n' "$init_out"; exit 1; }
# The line doctor verifies and case 3948 matches is the block's last, exactly. Doctor is not
# run here: its wiring check looks for `majordomus finish` in the hook (lib/doctor.sh), cases
# 2979 and 4265 prove that against an installed hook, and a doctor run would cost this case
# more than all of its pushes together on a loaded machine.
[ "$(printf '%s\n' "$body" | tail -n 1)" = "$MJ finish --check || exit \$?" ] \
  || { printf '    the advice does not end with the finish --check line:\n%s\n' "$body"; exit 1; }
printf '#!/bin/sh\n%s\n' "$body" > .git/hooks/pre-push && chmod +x .git/hooks/pre-push

mkdir -p lib && echo a > lib/a && git add . && git commit -qm base --no-verify
note() { printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null; }
branch="$(git symbolic-ref --short HEAD)"
Z=0000000000000000000000000000000000000000

"$MJ" start "t1" --scope lib >/dev/null
id1="$(sed -n 's/^id: //p' .ai/local/state/current.yaml)"
echo b >> lib/a
note
expect_exit 0 "$MJ" finish --outcome completed --verify-command "test -d .ai"
git add -A && git commit -qm verified --no-verify
c1="$(git rev-parse HEAD)"
echo c >> lib/a && git add -A && git commit -qm after-verify --no-verify
c2="$(git rev-parse HEAD)"

# pre_push <local ref> <local sha> <remote ref>: the hook as git runs it, one ref on stdin
pre_push() { printf '%s %s %s %s\n' "$1" "$2" "$3" "$Z" | .git/hooks/pre-push origin url; }

# HEAD is c2, which nobody verified, and the push publishes c1, which was verified: silent.
# The one line init printed before compares HEAD and warns here.
expect_exit 0 pre_push "refs/heads/$branch" "$c1" "refs/heads/$branch"
expect_no_grep 'pushed-tree'

# The unverified commit pushed from this branch is named.
expect_exit 0 pre_push "refs/heads/$branch" "$c2" "refs/heads/$branch"
expect_grep "^WARN pushed-tree +$id1 — commit $(git rev-parse --short=12 "$c2") "

# Not compared: a deletion (all-zero local sha), another branch, the tool's own refs.
expect_exit 0 pre_push "(delete)" "$Z" "refs/heads/$branch"
expect_no_grep 'pushed-tree'
git branch -q other "$c2"
expect_exit 0 pre_push refs/heads/other "$c2" refs/heads/other
expect_no_grep 'pushed-tree'
expect_exit 0 pre_push "refs/heads/$branch" "$c2" refs/majordomus/continuity/x
expect_no_grep 'pushed-tree'

# The refusal under `error` reaches git through the hook's `|| exit $?`.
cp .ai/repo/policy.yaml policy.orig
awk '{ print } /^verification:$/ { print "  pushed_tree_unverified: error" }' policy.orig > .ai/repo/policy.yaml
expect_exit 10 pre_push "refs/heads/$branch" "$c2" "refs/heads/$branch"
expect_grep "^FAIL pushed-tree +$id1 — "
expect_exit 0 pre_push "refs/heads/$branch" "$c1" "refs/heads/$branch"
cp policy.orig .ai/repo/policy.yaml && rm -f policy.orig

# A personal layer whose hooks are the project's: the drop-in is told the same block.
S="$T/shared" && mkdir -p "$S/.githooks/pre-push.d" && git -C "$S" init -q
printf '#!/bin/sh\nfor f in .githooks/pre-push.d/*; do [ -x "$f" ] && "$f" "$@"; done\n' > "$S/.githooks/pre-push"
cp "$S/.githooks/pre-push" "$S/.githooks/pre-commit"
mkdir -p "$S/.githooks/pre-commit.d" && touch "$S/.githooks/pre-push.d/.keep"
chmod +x "$S/.githooks/pre-push" "$S/.githooks/pre-commit"
git -C "$S" config core.hooksPath .githooks
git -C "$S" add -A && git -C "$S" commit -qm hooks --no-verify
out="$(cd "$S" && "$MJ" init --personal 2>&1)" || { echo "    init --personal failed: $out"; exit 1; }
printf '%s\n' "$out" | grep -qF 'pre-push.d/majordomus-personal (covered already), holding these lines:' \
  || { echo "    the drop-in advice does not hold the block: $out"; exit 1; }
printf '%s\n' "$out" | grep -qF 'MAJORDOMUS_PUSHED="${mj_p# }"; export MAJORDOMUS_PUSHED' \
  || { echo "    the drop-in advice does not pass the pushed commits: $out"; exit 1; }
printf '%s\n' "$out" | grep -qxF "    $MJ finish --check || exit \$?" \
  || { echo "    the drop-in advice does not end with the finish --check line: $out"; exit 1; }
echo "ok: the adopter pre-push hook compares what the push publishes"

# majordomus-covers: doctor
# claims: dispatcher-wiring
. "$ROOT/test/lib.sh"
# A linked worktree's .git is a file that names the repository's common directory, and its
# hooks are the common directory's: git runs the same .git/hooks/pre-commit from every
# worktree. Doctor resolved the default hooks directory as "$MJ_ROOT/.git/hooks", a path
# that exists only in the primary checkout, so in a linked worktree every wired hook read
# as missing, doctor failed, and the pre-commit hook that runs doctor refused every commit.
"$MJ" init >/dev/null
"$MJ" update >/dev/null
git config --unset core.hooksPath 2>/dev/null || true
hooks="$(git rev-parse --git-path hooks)"
mkdir -p "$hooks"
printf '#!/bin/sh\n%s doctor || exit $?\n' "$MJ" > "$hooks/pre-commit"
printf '#!/bin/sh\n%s finish --check || exit $?\n' "$MJ" > "$hooks/pre-push"
chmod +x "$hooks/pre-commit" "$hooks/pre-push"
git add -A >/dev/null && git commit -qm "layer" >/dev/null

# the primary checkout: wired through the default directory
expect_exit 0 "$MJ" doctor
expect_grep 'OK   wiring +doctor-on-commit — wired via .git/hooks/pre-commit$'

# a linked worktree: the same hooks, found where git keeps them
git worktree add -q -b feature/linked "$T/linked" >/dev/null 2>&1
cd "$T/linked" || exit 1
expect_exit 0 "$MJ" doctor
expect_no_grep 'FAIL wiring'
expect_grep 'OK   wiring +doctor-on-commit — wired via '
expect_grep 'OK   wiring +finish-on-push — wired via '

# and git itself agrees that a commit from here runs that hook
echo x > linked.txt && git add linked.txt
git commit -qm "from the linked worktree" >/dev/null 2>&1
[ "$(git log -1 --format=%s)" = "from the linked worktree" ] || { echo "    the pre-commit hook refused a commit from a linked worktree"; exit 1; }

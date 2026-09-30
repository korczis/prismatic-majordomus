# The challenge: installed and wired, a second worktree whose worker already claims lib, and a
# test the task's verification runs that fails until the work is actually done — the parser's
# message has to name the column.
. "$FIXTURE_SETUP/two-worktrees.sh"
mkdir -p test
printf '#!/bin/sh\n# done when the message names the column\ngrep -q column lib/a\n' > test/parse.sh
git add test && git -c core.hooksPath=/dev/null commit -qm "test: the parser names the column"

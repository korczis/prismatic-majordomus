# Installed and wired, with a second worktree of the same repository on another branch
# holding an active task scoped to lib: what a second worker sees when it starts. The hooks
# are committed with hooks off, so that the wiring never blocks the setup itself.
. "$FIXTURE_SETUP/installed.sh"
mkdir -p .githooks
printf '#!/bin/sh\n%s doctor || exit $?\n' "$MJ" > .githooks/pre-commit
printf '#!/bin/sh\n%s finish --check || exit $?\n' "$MJ" > .githooks/pre-push
chmod +x .githooks/pre-commit .githooks/pre-push
git config core.hooksPath .githooks
git add .githooks && git -c core.hooksPath=/dev/null commit -qm hooks
git worktree add -q "$PWD/../$(basename "$PWD")-other" -b other >/dev/null 2>&1
( cd "../$(basename "$PWD")-other" && "$MJ" start "the other worker's task" --scope lib >/dev/null )

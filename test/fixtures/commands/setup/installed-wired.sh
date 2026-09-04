# Installed, and the two enforcements the policy declares are actually in place as hooks.
. "$FIXTURE_SETUP/installed.sh"
mkdir -p .githooks
printf '#!/bin/sh\n%s doctor || exit $?\n' "$MJ" > .githooks/pre-commit
printf '#!/bin/sh\n%s finish --check || exit $?\n' "$MJ" > .githooks/pre-push
chmod +x .githooks/pre-commit .githooks/pre-push
git config core.hooksPath .githooks

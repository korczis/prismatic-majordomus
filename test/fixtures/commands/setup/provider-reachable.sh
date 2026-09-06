# Installed, with the executable reachable from the repository itself and no provider hook
# wired yet. A hook shim finds Majordomus from its own location — `bin/majordomus` in the
# tool's own checkout, `.majordomus/bin/majordomus` in a repository that installed it —
# because the provider substitutes a project directory into the command string textually and
# exports nothing. So the scenario's repository is arranged the second way, which is what
# lets a payload actually reach the command from inside the sandbox.
. "$FIXTURE_SETUP/installed.sh"
mkdir -p .majordomus/bin
printf '#!/bin/sh\nexec "%s" "$@"\n' "$MJ" > .majordomus/bin/majordomus
chmod +x .majordomus/bin/majordomus

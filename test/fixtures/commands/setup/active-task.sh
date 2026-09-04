# ... and an active task scoped to lib, with work done inside that scope.
. "$FIXTURE_SETUP/installed.sh"
"$MJ" start "narrow the parser" --scope lib >/dev/null
echo work >> lib/a

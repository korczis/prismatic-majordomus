# An active task with an unresolved question open against it.
. "$FIXTURE_SETUP/active-task.sh"
"$MJ" question add "does the parser need to accept tabs?" >/dev/null

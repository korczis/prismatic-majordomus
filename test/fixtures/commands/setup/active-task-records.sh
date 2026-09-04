# An active task that has already produced a checkpoint, a decision and an open question.
. "$FIXTURE_SETUP/active-task.sh"
printf 'the parser now refuses tabs\n' | "$MJ" checkpoint >/dev/null
"$MJ" decision add "refuse tabs in the parser" --why "two encodings for one token" >/dev/null
"$MJ" question add "should tabs be an error or a warning?" >/dev/null

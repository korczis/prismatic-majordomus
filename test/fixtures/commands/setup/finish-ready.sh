# An active task with the note the outcome requires already written.
. "$FIXTURE_SETUP/active-task.sh"
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null

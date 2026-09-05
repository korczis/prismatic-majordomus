# Installed, with one execution episode open in this worktree.
. "$FIXTURE_SETUP/installed.sh"
"$MJ" session start --worker some-provider/some-model >/dev/null

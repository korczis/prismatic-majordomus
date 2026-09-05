# Installed, with the prompt-capture hook in place and majordomus on PATH, which is how a
# repository that is not the tool's own checkout reaches it.
. "$FIXTURE_SETUP/installed.sh"
PATH="$(dirname "$MJ"):$PATH"; export PATH
"$MJ" capture install >/dev/null

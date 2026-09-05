# Installed, with one decision proposed and committed. Discovery is over the tracked tree,
# so a decision the working copy holds and git does not is not yet part of the layer: the
# commit is what makes the record discoverable, exactly as it is for every other kind.
. "$FIXTURE_SETUP/installed.sh"
"$MJ" adr propose "The registry is the one canonical declaration" --from file:docs/d --tag architecture >/dev/null
git add . && git commit -qm "adr: the registry is the one canonical declaration"

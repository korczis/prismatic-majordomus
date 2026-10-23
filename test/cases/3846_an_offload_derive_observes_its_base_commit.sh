# majordomus-covers: none
# A derive observes the commit the branch carries, never a throwaway input (I3836). An
# offloaded derive runs on a host at a `git stash create` snapshot or a `commit-tree` merge
# input that never reaches origin; delivery.json named that commit, CI's --check could not
# find it, and every offloaded branch went red (#1759, run 39535247244). The offload now
# names the worktree's HEAD in MJ_DERIVE_OBSERVE, and the generator observes it when it is
# an ancestor of the tree it runs in. `--delivery-at` prints the commit a generation would
# observe, so this reads the choice without running one.
. "$ROOT/test/lib.sh"

fixture_repo "$T"
git -C "$T" add -A >/dev/null; git -C "$T" commit -qm base
base="$(git -C "$T" rev-parse HEAD)"
# the host's position: HEAD is a snapshot of the authored change, a child of base
echo "authored" > "$T/authored.txt"; git -C "$T" add authored.txt
snapshot="$(git -C "$T" stash create "derive-offload input")"
[ -n "$snapshot" ] || { echo "    the fixture made no snapshot"; exit 1; }
git -C "$T" checkout -q --detach "$snapshot"
gen="$T/scripts/generate-site-data"

# told nothing, a generation observes HEAD, as a local derive always has
[ "$("$gen" --delivery-at)" = "$snapshot" ] \
  || { echo "    with no MJ_DERIVE_OBSERVE the generation did not observe HEAD"; exit 1; }
# told the worktree's HEAD, it observes that commit, which the branch carries
[ "$(MJ_DERIVE_OBSERVE="$base" "$gen" --delivery-at)" = "$base" ] \
  || { echo "    the generation did not observe the base commit it was told"; exit 1; }
# a commit that is not an ancestor of the tree, or no commit at all, is not believed
git -C "$T" checkout -q -b side "$base"; echo side > "$T/side.txt"
git -C "$T" add side.txt; git -C "$T" commit -qm side
side="$(git -C "$T" rev-parse HEAD)"
git -C "$T" checkout -q --detach "$snapshot"
[ "$(MJ_DERIVE_OBSERVE="$side" "$gen" --delivery-at)" = "$snapshot" ] \
  || { echo "    a commit off the tree's history was observed"; exit 1; }
[ "$(MJ_DERIVE_OBSERVE=not-a-commit "$gen" --delivery-at)" = "$snapshot" ] \
  || { echo "    a name that is no commit was observed"; exit 1; }
echo "    a derive observes the base commit it is told, and only one its tree descends from"

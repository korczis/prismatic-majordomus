# majordomus-covers: none
# The merge driver that stops two branches conflicting over a fingerprint neither of them owns.
#
# Every derived artifact carries a hash of the tree it was generated from. After a merge that
# tree is neither side's, so both recorded hashes are wrong and no textual combination of them
# is right — the answer only exists after the generator runs. Left to git's default driver the
# merge stops with conflict markers a person resolves by picking a side and regenerating, once
# per merge, forever.
#
# What must hold: .gitattributes marks those paths, the driver resolves them instead of
# conflicting, and the file it leaves behind is *stale* — because the safety of resolving to
# ours is entirely the gate that refuses to commit stale derived data. A driver that quietly
# produced a plausible-looking file would be worse than the conflict.
. "$ROOT/test/lib.sh"

# ---------------------------------------------------------------- the declaration
expect_file "$ROOT/.gitattributes"
expect_file "$ROOT/scripts/merge-derived"
[ -x "$ROOT/scripts/merge-derived" ] || { echo "    scripts/merge-derived is not executable"; exit 1; }

# The paths the two generators write are the paths marked — asked of git rather than read out
# of the file, because what matters is the attribute that applies to the path, however the
# pattern that grants it is written. A derived file nobody marked conflicts exactly as before,
# which is the failure this loop exists to notice.
# Every path the generator writes, out of the generator's own inventory rather than a list
# kept by hand here. A driver that covers some of the derived files is this mechanism failing
# in the costume of a success: the merge runs, a few paths resolve, and everything nobody
# thought of still conflicts. A list written into this case would fall behind the generator
# the first time a target is added; asking `git check-attr` about the inventory cannot.
python3 -c '
import json, sys
d = json.load(open(sys.argv[1]))
sys.stdout.write("\n".join(a["path"] for a in d["artifacts"]))
' "$ROOT/docs/generated/artifacts.json" > "$T/artifacts.txt" \
  || { echo "    docs/generated/artifacts.json does not list artifacts"; exit 1; }
[ -s "$T/artifacts.txt" ] || { echo "    the artifacts inventory is empty; this would pass vacuously"; exit 1; }
uncovered="$(xargs git -C "$ROOT" check-attr merge -- < "$T/artifacts.txt" | grep -v 'merge: derived' | sed 's/: merge:.*//')"
[ -z "$uncovered" ] || {
  echo "    $(printf '%s\n' "$uncovered" | wc -l | tr -d ' ') generated artifact(s) carry no merge=derived rule, so they conflict on every merge:"
  printf '%s\n' "$uncovered" | head -10 | sed 's/^/    | /'
  echo "    add a pattern to .gitattributes that covers them"
  exit 1; }

# and a file that is not derived is left alone: marking the whole tree would be worse than
# marking none of it
attr="$(git -C "$ROOT" check-attr merge -- lib/common.sh | sed 's/.*: //')"
[ "$attr" != derived ] || { echo "    lib/common.sh is marked merge=derived"; exit 1; }

# `just derive-merge-driver` is how a clone declares it; the recipe has to name the script.
# The declaration is modular, so the whole of it is read rather than the root file alone.
JF="$(just_declaration)"
grep -q 'derive-merge-driver' "$JF" || {
  echo "    no workflow declares derive-merge-driver to wire the driver"; exit 1; }
grep -q 'merge-derived' "$JF" || {
  echo "    the derive-merge-driver recipe does not name scripts/merge-derived"; exit 1; }

# ---------------------------------------------------------------- it resolves, in a real merge
# A repository with one marked file, two branches that each moved it, and the driver wired.
R="$T/repo"; mkdir -p "$R/docs"
git -C "$R" init -q
git -C "$R" config user.email t@example.com
git -C "$R" config user.name Test
git -C "$R" config merge.derived.name "derived"
git -C "$R" config merge.derived.driver "$ROOT/scripts/merge-derived %O %A %B %P"
printf 'docs/PLAN_STATUS.md merge=derived\n' > "$R/.gitattributes"
printf 'Generated from canonical inputs `aaaaaaaaaaaa`.\n' > "$R/docs/PLAN_STATUS.md"
git -C "$R" add -A; git -C "$R" commit -qm base

git -C "$R" checkout -q -b theirs
printf 'Generated from canonical inputs `bbbbbbbbbbbb`.\n' > "$R/docs/PLAN_STATUS.md"
git -C "$R" commit -qam theirs

git -C "$R" checkout -q master 2>/dev/null || git -C "$R" checkout -q main
printf 'Generated from canonical inputs `cccccccccccc`.\n' > "$R/docs/PLAN_STATUS.md"
git -C "$R" commit -qam ours
before_merge="$(git -C "$R" rev-parse HEAD)"

out="$(git -C "$R" merge theirs 2>&1)" || {
  echo "    the marked file still conflicts with the driver wired:"; printf '    | %s\n' "$out"; exit 1; }
[ -z "$(git -C "$R" status --porcelain | grep -E '^(UU|AA)')" ] || {
  echo "    the merge left an unmerged path"; exit 1; }

# ours, unchanged — and therefore stale, which is the whole contract
grep -q 'cccccccccccc' "$R/docs/PLAN_STATUS.md" || {
  echo "    the driver did not resolve to ours"; exit 1; }
case "$out" in
  *"run \`just derive\`"*) ;;
  *) echo "    the driver resolved silently; it must name the repair:"; printf '    | %s\n' "$out"; exit 1 ;;
esac

# ---------------------------------------------------------------- and without the driver
# The declaration is per clone, so the same tree must degrade to today's behaviour rather than
# to a silent wrong answer: an unwired clone conflicts, exactly as it does now.
git -C "$R" config --unset merge.derived.driver
git -C "$R" reset -q --hard "$before_merge"
if git -C "$R" merge theirs >/dev/null 2>&1; then
  echo "    an unwired clone merged the marked file anyway; the resolution must not be silent"; exit 1
fi
git -C "$R" merge --abort 2>/dev/null || true

# ---------------------------------------------------------------- and something requires it
# The declaration is per clone and git falls back to the default text merge in silence when
# it is missing, so a clone looks configured, every file is committed, and the driver never
# runs. The policy entry is what lets doctor — and therefore the pre-commit hook — refuse it.
grep -q 'wired_by: git-config:merge.derived.driver' "$ROOT/.ai/repo/policy.yaml" || {
  echo "    the driver is not an enforcement entry, so doctor cannot require it of a clone"; exit 1; }

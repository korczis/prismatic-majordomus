# majordomus-covers: none
# The site generator reads every .claude/skills/<id>/SKILL.md its artifacts manifest names (ADR
# 0165), and those files are the editors projection's output, not canonical inputs, so a fixture
# built from `--inputs` alone lacked them and cases 10, 11 and 37 failed at the first skill with
# "No such file or directory" (I3999). fixture_repo copies the directory, and this proves it.
#
#   every skill file the manifest names is present in a fixture, byte for byte
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || skip "jq absent"
fixture_repo "$T"
want="$(jq -r '.artifacts[] | select(.document | startswith("editors/")) | select(.path | test("^\\.claude/skills/[^/]+/SKILL\\.md$")) | .path' "$ROOT/docs/generated/artifacts.json")"
[ -n "$want" ] || skip "the manifest names no skill"
for p in $want; do
  [ -f "$T/$p" ] || { echo "the fixture lacks $p" >&2; false; }
  cmp -s "$ROOT/$p" "$T/$p" || { echo "the fixture's $p differs from the checkout's" >&2; false; }
done
echo "ok: $(printf '%s\n' "$want" | wc -l | tr -d ' ') skill file(s) present in the fixture"

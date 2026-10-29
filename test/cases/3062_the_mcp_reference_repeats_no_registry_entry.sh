# majordomus-covers: none
# No hand-written document repeats the registry's tool list (I2732, ADR 0004).
#
# docs/MCP.md carried five hand-written tables of MCP tools — tool, capability, arguments,
# answers — copied from the registry. On 2026-10-15 a new capability, worktree.uncommitted,
# reached the registry and every generated page and was missing from those tables: a second
# definition falls behind the first the day a module is composed. The total reference is
# derived (docs/generated/capabilities.md, /registry/mcp/ — case 532 holds the latter to the
# registry), so a narrative page names a tool in prose and never lists tools in a table.
#
# What this holds: no Markdown document git tracks under docs/, outside docs/generated/, has
# a table row whose first cell is a backticked majordomus_ tool name. Before it reads the
# tree it proves its own pattern on a planted row and a planted non-row, so that a pattern
# that matches nothing can never read as a clean tree.
. "$ROOT/test/lib.sh"
cd "$ROOT" || exit 1

# a table row whose first cell is one backticked tool name: `| `majordomus_x` | ...`
row='^[[:space:]]*\|[[:space:]]*`majordomus_[a-z0-9_]+`[[:space:]]*\|'

S="$(mktemp -d "${TMPDIR:-/tmp}/mj3062.XXXXXX")"; trap 'rm -rf "$S"' EXIT
printf '%s\n' '| `majordomus_worktree_uncommitted` | `worktree.uncommitted` | none | x |' > "$S/row.md"
printf '%s\n' 'Two tools serve the board: `majordomus_peers` reads it.' '| `peers.list` | x |' > "$S/prose.md"
grep -qE -- "$row" "$S/row.md" || { echo "    the pattern does not match a planted tool row"; exit 1; }
if grep -qE -- "$row" "$S/prose.md"; then echo "    the pattern matches prose or a capability row"; exit 1; fi

fail=0
files="$(git ls-files -- 'docs/*.md' 'docs/**/*.md' | grep -v '^docs/generated/' || true)"
[ -n "$files" ] || { echo "    git tracks no document under docs/"; exit 1; }
for f in $files; do
  hits="$(grep -nE -- "$row" "$f" || true)"
  [ -n "$hits" ] || continue
  fail=1
  printf '%s\n' "$hits" | while IFS= read -r h; do
    echo "    $f:${h%%:*}: a hand-written tool row; name the tool in prose and link docs/generated/capabilities.md"
  done
done
exit "$fail"

# A worktree is created by one capability (I4002).
#
# Until this case every worktree.* capability read, and the only way a branch's worktree
# came into being was a person typing `majordomus worktree create`: an executor that needed
# an isolated checkout had no typed operation to ask for one, and the two places that ran
# `git worktree add` for themselves made detached scratch trees, not the canonical one.
# `worktree.create` is the service's `ensure`, declared once and projected on MCP, HTTP and
# the command line (`worktree ensure`).
#
# What is held here, through the built executable over the real MCP pipes:
#
#   1  create: a new branch gets its worktree at the canonical path, from the remote trunk,
#      and the answer names the branch, the path, the head read back, and `existed: false`;
#   2  the path is the canonical derivation: what `worktree path <branch>` prints;
#   3  an idempotent repeat answers the same worktree with `existed: true` and changes
#      nothing in the topology;
#   4  a conflicting existing worktree — the branch checked out by hand somewhere else — is
#      refused with the service's reason, and nothing is created or moved;
#   5  the command-line projection `worktree ensure --format json` answers the same shape;
#   6  the capability declares that it writes the repository, and MCP announces it so.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as every Rust case does.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }

# Nested one level down so the container (<repo>-wt) stays under $T; resolved, because git
# reports the resolved path and on macOS that is not the $TMPDIR spelling.
mkdir -p "$T/wt"; W="$(cd "$T/wt" && pwd -P)"; R="$W/repo"; O="$W/origin.git"; S="$W/scratch"
mkdir -p "$S"
fixture_repo "$R" >/dev/null
gitc() { git -c user.email=t@example.com -c user.name=t -c core.hooksPath=/dev/null "$@"; }
git -C "$R" init -q -b master .
git -C "$R" add -A >/dev/null
gitc -C "$R" commit -qm fixture >/dev/null
git init -q --bare -b master "$O"
git -C "$R" remote add origin "$O"
git -C "$R" push -q origin master 2>/dev/null
git -C "$R" fetch -q origin
git -C "$R" remote set-head origin master >/dev/null
trunk="$(git -C "$R" rev-parse master)"

fail=0
note() { printf '    %s\n' "$1"; fail=1; }

# One MCP tool call over the real pipes: the tool's own verdict is isError, not the
# process's status. The structured answer goes to stdout; isError and the text to files.
tool() { # <tool> <arguments-json>
  local name="$1" args="$2" out="$S/frames"
  {
    printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case4002","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/list"}\n'
    printf '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$name" "$args"
  } | ( cd "$R" && MAJORDOMUS_SHARE="$R/share" "$RB" mcp ) 2>"$S/mcp.err" > "$out" || {
    printf '    the server failed on %s:\n' "$name" >&2; sed 's/^/    | /' "$S/mcp.err" >&2; return 1; }
  jq -c 'select(.id == 2)' < "$out" > "$S/list.json"
  jq -r 'select(.id == 3) | .result.isError' < "$out" > "$S/is_error"
  jq -r 'select(.id == 3) | .result.content[0].text // ""' < "$out" > "$S/text"
  jq 'select(.id == 3) | .result.structuredContent' < "$out"
}
mj() { ( cd "$R" && MAJORDOMUS_SHARE="$R/share" "$RB" "$@" ); }
topology() { git -C "$R" worktree list --porcelain; git -C "$R" for-each-ref --format='%(refname) %(objectname)' refs/heads; }

# ---------------------------------------------------------------- 1. create
tool majordomus_worktree_create '{"branch":"feature/one/capability"}' > "$S/made.json" || exit 1
[ "$(cat "$S/is_error")" = false ] || { note "the create was refused: $(cat "$S/text")"; cat "$S/mcp.err"; exit 1; }
[ "$(jq -r .branch < "$S/made.json")" = feature/one/capability ] || note "the answer names $(jq -r .branch < "$S/made.json")"
[ "$(jq -r .existed < "$S/made.json")" = false ] || note "a new worktree was answered as existing"
[ "$(jq -r .branch_created < "$S/made.json")" = true ] || note "the new branch was not created"
[ "$(jq -r .base < "$S/made.json")" = origin/master ] || note "the branch started at $(jq -r .base < "$S/made.json"), not origin/master"
path="$(jq -r .path < "$S/made.json")"
head="$(jq -r .head < "$S/made.json")"
[ -d "$path" ] || note "no directory at $path"
[ "$(git -C "$path" symbolic-ref --short HEAD 2>/dev/null)" = feature/one/capability ] || note "the worktree is not on its branch"
[ "$head" = "$(git -C "$path" rev-parse HEAD 2>/dev/null)" ] || note "the head $head is not the worktree's"
[ "$head" = "$trunk" ] || note "the new branch starts at $head, not the remote trunk $trunk"

# ---------------------------------------------------------------- 2. the canonical derivation
[ "$path" = "$R-wt/feature/one/capability" ] || note "the path is $path, not $R-wt/feature/one/capability"
expect_exit 0 mj worktree path feature/one/capability || fail=1
[ "$LAST_OUT" = "$path" ] || note "worktree path says '$LAST_OUT', the capability made '$path'"

# ---------------------------------------------------------------- 3. idempotent repeat
before="$(topology)"
tool majordomus_worktree_create '{"branch":"feature/one/capability"}' > "$S/again.json" || exit 1
[ "$(cat "$S/is_error")" = false ] || note "the repeat was refused: $(cat "$S/text")"
[ "$(jq -r .existed < "$S/again.json")" = true ] || note "the repeat did not answer the existing worktree"
[ "$(jq -r .branch_created < "$S/again.json")" = false ] || note "the repeat created a branch"
[ "$(jq -r .path < "$S/again.json")" = "$path" ] || note "the repeat answered $(jq -r .path < "$S/again.json")"
[ "$(jq -r .head < "$S/again.json")" = "$head" ] || note "the repeat answered head $(jq -r .head < "$S/again.json")"
[ "$(topology)" = "$before" ] || note "the repeat changed the topology"

# ---------------------------------------------------------------- 4. a conflicting worktree
gitc -C "$R" worktree add -q -b fix/made-by-hand "$W/elsewhere" 2>/dev/null
before="$(topology)"
tool majordomus_worktree_create '{"branch":"fix/made-by-hand"}' > "$S/refused.json" || exit 1
[ "$(cat "$S/is_error")" = true ] || note "a branch checked out elsewhere was not refused"
grep -q "fix/made-by-hand" "$S/text" || note "the refusal does not name the branch: $(cat "$S/text")"
grep -q "$W/elsewhere" "$S/text" || note "the refusal does not name where it is: $(cat "$S/text")"
[ -e "$R-wt/fix/made-by-hand" ] && note "the refused create made $R-wt/fix/made-by-hand"
[ -d "$W/elsewhere" ] || note "the refused create moved the worktree it met"
[ "$(topology)" = "$before" ] || note "the refusal changed the topology"
# and a path occupied by something that is not this branch's worktree
mkdir -p "$R-wt/feature/occupied" && echo mine > "$R-wt/feature/occupied/file"
tool majordomus_worktree_create '{"branch":"feature/occupied"}' > /dev/null || exit 1
[ "$(cat "$S/is_error")" = true ] || note "an occupied canonical path was not refused"
[ "$(cat "$R-wt/feature/occupied/file")" = mine ] || note "the refused create touched what occupied the path"
git -C "$R" rev-parse -q --verify refs/heads/feature/occupied >/dev/null && note "the refused create made the branch"

# ---------------------------------------------------------------- 5. the command-line projection
mj worktree ensure feature/one/capability --format json > "$S/cli.json" 2>"$S/cli.err" \
  || note "worktree ensure failed: $(cat "$S/cli.err")"
[ "$(jq -r .existed < "$S/cli.json")" = true ] || note "worktree ensure did not answer the existing worktree"
[ "$(jq -r .path < "$S/cli.json")" = "$path" ] || note "worktree ensure answered $(jq -r .path < "$S/cli.json")"
[ "$(jq -r .head < "$S/cli.json")" = "$head" ] || note "worktree ensure answered head $(jq -r .head < "$S/cli.json")"

# ---------------------------------------------------------------- 6. declared as a write
jq -e '.result.tools[] | select(.name == "majordomus_worktree_create") | .annotations.readOnlyHint == false' \
  < "$S/list.json" >/dev/null || note "MCP does not announce majordomus_worktree_create as a write"
mj capabilities describe worktree.create --format json > "$S/describe.json" 2>/dev/null \
  || note "capabilities describe worktree.create failed"
grep -q '"repository_mutation"' "$S/describe.json" || note "worktree.create does not declare the repository-mutation effect"

exit "$fail"

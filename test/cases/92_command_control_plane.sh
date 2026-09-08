# majordomus-covers: none
# The canonical command graph and its projections.
#
# What this case exists to prove, in the order a reader will want it:
#
#   the graph is valid — no command that can be run without saying what running it
#   changes, and no two commands answering to one spelling on one surface;
#
#   the exposure policy is derived rather than configured — nothing classified as
#   changing the repository or destroying work is offered to a machine surface, and the
#   refusal names the reason;
#
#   the two surfaces cannot drift — `just <recipe> <TAB>` and `majordomus <words> <TAB>`
#   return the same candidates, because the recipe resolves to the node the words walk to;
#
#   the bridge is a projection — generated, atomic, idempotent, ignored by git, free of
#   any call back into `just`, and carrying every recipe name the justfile used to have;
#
#   arguments survive the bridge — spaces, quotes, `$`, a leading dash and a UTF-8 word
#   reach the executable exactly as they were typed.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }
RB="$(rust_bin)" || rust_bin_exit $?

# ---------------------------------------------------------------- the graph is valid
expect_exit 0 "$RB" commands graph
G="$T/graph.json"; printf '%s' "$LAST_OUT" > "$G"
[ "$(jq '.diagnostics | length' "$G")" = 0 ] || { echo "    the graph carries diagnostics: $(jq -c '.diagnostics' "$G")"; exit 1; }
[ "$(jq '.commands | length' "$G")" -gt 20 ] || { echo "    only $(jq '.commands | length' "$G") commands"; exit 1; }
jq -e '.schema == "majordomus/commands/v1"' "$G" >/dev/null || { echo "    the graph does not name its schema"; exit 1; }

# every command that can be run says what running it changes
missing="$(jq -r '.commands[] | select(.group == false) | select(.semantics == null) | .id' "$G")"
[ -z "$missing" ] || { echo "    no semantics declared for: $missing"; exit 1; }

# the fingerprint is of the graph, not of the moment
a="$(jq -r .fingerprint "$G")"; b="$("$RB" commands graph | jq -r .fingerprint)"
[ "$a" = "$b" ] || { echo "    the fingerprint moved between two runs: $a vs $b"; exit 1; }

# ---------------------------------------------------------------- exposure is derived
leaked="$(jq -r '.commands[] | select(.semantics.effect == "repository_mutation" or .semantics.effect == "destructive") | select(.projections.mcp != null or .projections.cockpit != null) | .id' "$G")"
[ -z "$leaked" ] || { echo "    offered to a machine despite its effect: $leaked"; exit 1; }
expect_exit 0 "$RB" commands explain worktree.remove
expect_grep 'destructive'
expect_grep 'none — it removes work'
expect_grep 'just worktree-remove'
# and a read is offered everywhere it can be. The tool name here is the registry's own
# declaration — `majordomus_capabilities` — not the one the naming algorithm would compute:
# a declared exposure is canonical, and the algorithm only fills in where nothing is.
expect_exit 0 "$RB" commands explain capabilities.list
expect_grep 'majordomus_capabilities'
expect_grep 'GET /api/v1/capabilities'

# ---------------------------------------------------------------- the surfaces agree
cli="$("$RB" completion query --surface cli --cursor 3 -- worktree status --format '')"
jst="$("$RB" completion query --surface just --cursor 2 -- worktree-status --format '')"
[ "$cli" = "$jst" ] || { printf '    the two surfaces answer differently\n    cli:  %s\n    just: %s\n' "$cli" "$jst"; exit 1; }
[ -n "$cli" ] || { echo "    neither surface offered anything"; exit 1; }
printf '%s' "$cli" | grep -q '^json' || { echo "    the accepted values of --format were not offered: $cli"; exit 1; }

# an unknown or half-typed command line answers rather than failing
expect_exit 0 "$RB" completion query --surface cli --cursor 1 -- nosuchcommand ''
expect_exit 0 "$RB" completion query --surface just --cursor 0 -- ''

# no credential can be offered: the engine reads no environment value
OPENAI_API_KEY=SENTINEL_ONE ANTHROPIC_API_KEY=SENTINEL_TWO "$RB" completion query --surface cli --cursor 0 -- '' > "$T/candidates" 2>&1
expect_no_grep 'SENTINEL_' "$T/candidates"
OPENAI_API_KEY=SENTINEL_ONE "$RB" commands graph > "$T/graph2.json"
expect_no_grep 'SENTINEL_' "$T/graph2.json"

# ---------------------------------------------------------------- the bridge is a projection
B="$T/repo"; mkdir -p "$B"
expect_exit 0 env -C "$B" "$RB" commands materialise
expect_grep 'wrote'
expect_exit 0 env -C "$B" "$RB" commands materialise
expect_grep 'current'                      # the second run writes nothing
BRIDGE="$B/.majordomus/runtime/just/bridge.just"
expect_file "$BRIDGE"
expect_grep 'GENERATED FILE' "$BRIDGE"
expect_grep 'set positional-arguments' "$BRIDGE"
expect_no_grep '^[[:space:]]*just ' "$BRIDGE"     # no recipe of the bridge calls just back
expect_grep '^\[confirm\(' "$BRIDGE"              # and destruction asks first

# every recipe name the bridge produces is one just can parse
bad="$(grep -oE '^[^[:space:]#]+ \*args:' "$BRIDGE" | sed 's/ \*args:$//' | grep -vE '^[A-Za-z][A-Za-z0-9_-]*$' || true)"
[ -z "$bad" ] || { echo "    unusable recipe name(s): $bad"; exit 1; }

# ---------------------------------------------------------------- the repository's own bridge
if command -v just >/dev/null 2>&1; then
  ( cd "$ROOT" && just bridge >/dev/null 2>&1 ) || { echo "    just bridge failed in the repository"; exit 1; }
  # entering the repository and listing its recipes leaves the tree as it was
  before="$(git -C "$ROOT" status --porcelain | wc -l | tr -d ' ')"
  ( cd "$ROOT" && just --list >/dev/null 2>&1 ) || { echo "    just --list failed"; exit 1; }
  after="$(git -C "$ROOT" status --porcelain | wc -l | tr -d ' ')"
  [ "$before" = "$after" ] || { echo "    listing the recipes changed the working tree ($before -> $after)"; exit 1; }
  git -C "$ROOT" check-ignore -q .majordomus/runtime/just/bridge.just || { echo "    the generated bridge is not ignored"; exit 1; }

  # every recipe name the justfile used to carry still resolves, through a generated alias
  names="$(cd "$ROOT" && just --dump --dump-format json 2>/dev/null | jq -r '(.recipes | keys[]), (.aliases // {} | keys[])' | sort -u)"
  for old in capabilities describe validate generate bench-run bench-baseline wt wt-create wt-doctor wt-migrate serve mcp bench-coverage; do
    printf '%s\n' "$names" | grep -qx "$old" || { echo "    the recipe name '$old' disappeared in the migration"; exit 1; }
  done

  # the justfile may not regrow a recipe for something the executable already declares:
  # that is the drift this rule exists to prevent, and it is checked against the tracked
  # file rather than the merged dump, which is where a hand-written duplicate would appear
  for recipe in $(jq -r '.commands[].projections.just | select(. != null)' "$G"); do
    grep -qE "^${recipe}( |:)" "$ROOT/justfile" && { echo "    the justfile declares '$recipe' by hand; it is already a projection of the command graph"; exit 1; }
  done

  # arguments reach the executable exactly as they were typed
  out="$(cd "$ROOT" && just commands-explain 'worktree.remove' 2>&1)" || { echo "    an argument did not survive the bridge: $out"; exit 1; }
  printf '%s' "$out" | grep -q 'worktree.remove' || { echo "    the argument was not forwarded: $out"; exit 1; }
  for arg in 'a b' 'a$b' "a'b" 'a"b' 'příliš žluťoučký' '  leading and trailing  '; do
    out="$(cd "$ROOT" && just commands-explain "$arg" 2>&1 || true)"
    printf '%s' "$out" | grep -qF "no command '$arg'" || { printf '    the argument %s did not arrive intact: %s\n' "$arg" "$out"; exit 1; }
  done
  # a value that looks like a flag reaches the executable whole; clap then refuses it, which
  # is the parser's contract and not a failure of the forwarding — the refusal quotes it back
  out="$(cd "$ROOT" && just commands-explain -- '--looks-like-a-flag' 2>&1 || true)"
  printf '%s' "$out" | grep -qF -- '--looks-like-a-flag' || { printf '    a flag-shaped argument was lost: %s\n' "$out"; exit 1; }
fi

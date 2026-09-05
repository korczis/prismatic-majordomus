# majordomus-covers: none
# The native command line documents itself, through the built binary: the registry's own
# validation names the command-line contract, the generated document holds every command with
# the route and the examples its projections render, the Markdown reference and the site
# dataset are that same tree, a hand edit to either projection makes `generate --check` stale,
# and two generations produce identical bytes. Nothing here lists a command: every expectation
# is read from docs/generated/cli.json, which the executable derived from its clap declaration.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    skip: jq not installed"; exit 0; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj98.XXXXXX")"; trap 'rm -rf "$S"' EXIT
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
CLI="$ROOT/docs/generated/cli.json"

# --- the contract is a gate of the registry's own validation
( cd "$ROOT" && expect_exit 0 "$RB" capabilities validate && expect_grep '^OK   cli ' && expect_grep 'validate: 0 failure\(s\)' ) || exit 1

# --- the document: a command line with commands, each with the route its page has
jq -e '.schema == "majordomus/cli/v1" and (.route_prefix == "/docs/cli") and (.cli.path == ["majordomus"])' "$CLI" >/dev/null \
  || { echo "    docs/generated/cli.json is not the command line as data"; exit 1; }
[ -f "$ROOT/$(jq -r '.source' "$CLI")" ] || { echo "    cli.json names a declaration that does not exist"; exit 1; }
# every command, flattened, with what each projection needs
jq -r '[.cli | recurse(.subcommands[]?) | {path, route, executable, about, usage, examples, args}]' "$CLI" > "$S/commands.json"
n="$(jq 'length' "$S/commands.json")"
[ "$n" -ge 2 ] || { echo "    cli.json holds $n command(s)"; exit 1; }

# every command's route is its path under the prefix, and no two share one
jq -e 'all(.[]; .route == ("/docs/cli/" + (.path[1:] | join("/")) + (if (.path | length) > 1 then "/" else "" end)))' "$S/commands.json" >/dev/null \
  || { echo "    a command's route is not its path under /docs/cli/"; exit 1; }
[ "$(jq '[.[].route] | unique | length' "$S/commands.json")" = "$n" ] || { echo "    two commands share a route"; exit 1; }

# every command says what it does; every argument says what it is for; every command a person
# can run carries at least one example, and every example says what it shows
jq -e 'all(.[]; (.about | length) > 0 and (.usage | length) > 0 and all(.args[]?; (.help | length) > 0))' "$S/commands.json" >/dev/null \
  || { echo "    a command or an argument is undocumented: $(jq -r '[.[] | select((.about|length) == 0 or any(.args[]?; (.help|length) == 0)) | .path | join(" ")] | join(", ")' "$S/commands.json")"; exit 1; }
jq -e 'all(.[] | select(.executable); (.examples | length) > 0)' "$S/commands.json" >/dev/null \
  || { echo "    a command that can be run carries no example: $(jq -r '[.[] | select(.executable) | select((.examples|length) == 0) | .path | join(" ")] | join(", ")' "$S/commands.json")"; exit 1; }
jq -e 'all(.[].examples[]?; (.id | length) > 0 and (.title | length) > 0 and (.description | length) > 0 and (.argv | length) > 0 and (.command | startswith("majordomus ")) and (.expectation | length) > 0)' "$S/commands.json" >/dev/null \
  || { echo "    an example is incomplete"; exit 1; }
[ "$(jq '[.[].examples[]?.id] | unique | length' "$S/commands.json")" = "$(jq '[.[].examples[]?.id] | length' "$S/commands.json")" ] \
  || { echo "    two examples share an id"; exit 1; }
# the command line a reader copies is the argument vector, rendered: every word survives it
jq -e 'all(.[].examples[]?; . as $e | all($e.argv[]; . as $w | $e.command | contains($w)))' "$S/commands.json" >/dev/null \
  || { echo "    an example's command line is not its argument vector"; exit 1; }

# --- the Markdown reference is that same tree: every command and every example
jq -r '.[] | .path | join(" ")' "$S/commands.json" | while read -r cmd; do
  grep -qF "## \`$cmd\`" "$ROOT/docs/generated/cli.md" || { echo "    $cmd is not in docs/generated/cli.md"; exit 1; }
done || exit 1
jq -r '.[].examples[]?.command' "$S/commands.json" | while read -r line; do
  grep -qF "$ $line" "$ROOT/docs/generated/cli.md" || { echo "    the example \`$line\` is not in docs/generated/cli.md"; exit 1; }
done || exit 1

# --- the site dataset carries the same command line, with the same routes
[ -f "$ROOT/site/data/registry/registry.json" ] && {
  a="$(jq -S '[.cli | recurse(.subcommands[]?) | .route]' "$ROOT/site/data/registry/registry.json")"
  b="$(jq -S '[.[].route]' "$S/commands.json")"
  [ "$a" = "$b" ] || { echo "    the site dataset's command routes differ from cli.json's"; exit 1; }
}

# --- drift: a hand edit to either projection makes the tree stale, and only regeneration clears it
expect_exit 0 "$RB" generate docs --repo "$ROOT" --out "$S/gen"
expect_exit 0 "$RB" generate docs --repo "$ROOT" --out "$S/gen" --check
printf '\nhand written\n' >> "$S/gen/docs/generated/cli.md"
expect_exit 10 "$RB" generate docs --repo "$ROOT" --out "$S/gen" --check
expect_grep 'docs/generated/cli.md'
expect_exit 0 "$RB" generate docs --repo "$ROOT" --out "$S/gen"
expect_exit 0 "$RB" generate docs --repo "$ROOT" --out "$S/gen" --check
jq '.cli.about = "a hand edit"' "$S/gen/docs/generated/cli.json" > "$S/gen/docs/generated/cli.json.tmp" \
  && mv "$S/gen/docs/generated/cli.json.tmp" "$S/gen/docs/generated/cli.json"
expect_exit 10 "$RB" generate docs --repo "$ROOT" --out "$S/gen" --check
expect_grep 'docs/generated/cli.json'

# --- determinism: two generations of the same tree are byte for byte the same
expect_exit 0 "$RB" generate docs --repo "$ROOT" --out "$S/a"
expect_exit 0 "$RB" generate docs --repo "$ROOT" --out "$S/b"
cmp -s "$S/a/docs/generated/cli.md" "$S/b/docs/generated/cli.md" || { echo "    two generations of cli.md differ"; exit 1; }
cmp -s "$S/a/docs/generated/cli.json" "$S/b/docs/generated/cli.json" || { echo "    two generations of cli.json differ"; exit 1; }
# and nothing in either is of this machine or this moment
grep -qE '/Users/|/home/|[0-9]{4}-[0-9]{2}-[0-9]{2}T' "$S/a/docs/generated/cli.md" "$S/a/docs/generated/cli.json" \
  && { echo "    a generated command-line projection carries a local path or a timestamp"; exit 1; }

echo "    ok: $n command(s), $(jq '[.[].examples[]?] | length' "$S/commands.json") example(s), routes derived, projections in sync and deterministic"

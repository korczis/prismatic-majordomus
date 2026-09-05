# majordomus-covers: init doctor start check watch update handover finish context checkpoint history decision question prompt search version
# majordomus-negative: version
. "$ROOT/test/lib.sh"
# Every subcommand the binary dispatches is reachable, self-documenting, and exercised
# here — so "tested in CI" is true of each one individually, not only of the suite.
#
# The list is read from the dispatch table rather than written here, so adding a command
# to bin/majordomus adds it to every check below instead of quietly escaping them.
COMMANDS="$(grep -oE '^  [a-z|]+\)$' "$ROOT/bin/majordomus" | tr -d ' )' | tr '|' '\n' | sort -u)"
[ "$(printf '%s\n' "$COMMANDS" | wc -w | tr -d ' ')" -ge 8 ] \
  || { echo "    the dispatch table in bin/majordomus changed shape; update this case"; exit 1; }
# the commands that must exist whatever else is added
for c in init doctor start check watch update handover finish; do
  printf '%s\n' "$COMMANDS" | grep -qx "$c" || { echo "    dispatch table lost $c"; exit 1; }
done

# usage lists every dispatched command, and version, under some heading
expect_exit 0 "$MJ" --help
for c in $COMMANDS version; do
  expect_grep "^  $c" || { echo "    usage does not list $c"; exit 1; }
done
expect_grep 'exit codes: 0 ok'

# every command answers --help and says what it is
"$MJ" init >/dev/null
for c in $COMMANDS; do
  expect_exit 0 "$MJ" "$c" --help
  expect_grep "usage: majordomus $c" || { echo "    $c --help does not print its usage"; exit 1; }
done

# version needs no installation and is the string the rest of the project derives from
expect_exit 0 "$MJ" version
expect_grep '^majordomus [0-9]+\.[0-9]+\.[0-9]+$'
expect_exit 0 "$MJ" --version
expect_grep '^majordomus [0-9]+\.[0-9]+\.[0-9]+$'
ver="$(sed -n 's/^MJ_VERSION="\([^"]*\)"$/\1/p' "$ROOT/bin/majordomus")"
expect_grep "^majordomus $ver$"
# version is dispatched ahead of the option parser, which is exactly how a command comes to
# ignore arguments the rest of the surface refuses. It has to refuse them for itself.
expect_exit 2 "$MJ" version --no-such-option
expect_grep 'version: unknown option --no-such-option'

# an unknown command is a usage error, not a silent no-op
expect_exit 2 "$MJ" nonsense
expect_grep 'unknown command: nonsense'

# every command refuses an argument it does not know rather than ignoring it. Commands with
# subcommands report an unknown subcommand; the rest report an unknown option. Either way
# the exit code is 2 and the reason is named.
for c in $COMMANDS; do
  case "$c" in init|start|search) continue ;; esac   # these take positional arguments
  expect_exit 2 "$MJ" "$c" --no-such-option
  expect_grep 'unknown (option|subcommand)' || { echo "    $c accepted --no-such-option"; exit 1; }
done

# the read-only commands stay read-only: nothing under state/ changes when they run
"$MJ" update >/dev/null
before="$(find .ai/local/state -type f -exec shasum -a 256 {} \; | sort)"
for c in doctor watch context history search; do
  "$MJ" "$c" x >/dev/null 2>&1 || true
done
after="$(find .ai/local/state -type f -exec shasum -a 256 {} \; | sort)"
[ "$before" = "$after" ] || { echo "    a read-only command wrote to .ai/local/state"; exit 1; }

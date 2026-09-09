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
expect_grep 'usage: majordomus <command>'

# Two programs answer to the name `majordomus`, and the repository's own instructions name
# commands of both, so an unknown command that belongs to the Rust executable must say which
# program has it and how to reach it — a worker who followed those instructions and got only
# "unknown command" has nowhere to go. The command probed with is read from the projection of
# the clap declaration rather than written here: a command name in a test is the catalogue
# project.commands-are-projections forbids.
CLI_DOC="$ROOT/docs/generated/cli.yaml"
if [ -f "$CLI_DOC" ]; then
  # the first item of the `subcommands:` list directly under `cli:`; its path is the program
  # and the command, and the deeper indentation of a nested list keeps that list out of this
  native="$(awk '
    /^  subcommands:$/ { top = 1; next }
    top && /^    - path:$/ { n = 0; inpath = 1; next }
    inpath && /^        - / { n += 1; if (n == 2) { s = $0; sub(/^ *- */, "", s); print s; exit } }
  ' "$CLI_DOC")"
  [ -n "$native" ] || { echo "    $CLI_DOC names no command of the Rust executable"; exit 1; }
  if printf '%s\n' "$COMMANDS" | grep -qx "$native"; then
    echo "    $native is dispatched by both programs; this check needs one that is not"; exit 1
  fi
  expect_exit 2 "$MJ" "$native"
  expect_grep "$native is a command of the Rust executable"
  # and it names the launcher that runs it, which builds the executable when it must
  expect_grep "bin/majordomus-cli $native"
  # the pointer is the answer, so the usage text of the wrong program is not also dumped
  expect_no_grep 'usage: majordomus <command>'
fi

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

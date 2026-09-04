. "$ROOT/test/lib.sh"
# Every subcommand the binary dispatches is reachable, self-documenting, and exercised
# here — so "tested in CI" is true of each one individually, not only of the suite.
COMMANDS="init doctor start check watch update handover finish doctrine version"

# the binary's own dispatch table is the list this case is allowed to check
grep -q 'init|doctor|start|check|watch|update|handover|finish|doctrine)' "$ROOT/bin/majordomus" \
  || { echo "    the dispatch table in bin/majordomus changed shape; update this case"; exit 1; }

# usage lists every command
expect_exit 0 "$MJ" --help
for c in $COMMANDS; do
  expect_grep "^  $c" || { echo "    usage does not list $c"; exit 1; }
done
expect_grep 'exit codes: 0 ok'

# every command answers --help without an installation, and says what it is
"$MJ" init >/dev/null
for c in doctor start check watch update handover finish doctrine; do
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

# an unknown command is a usage error, not a silent no-op
expect_exit 2 "$MJ" nonsense
expect_grep 'unknown command: nonsense'

# every command refuses an option it does not know rather than ignoring it
for c in doctor update check watch handover finish; do
  expect_exit 2 "$MJ" "$c" --no-such-option
  expect_grep "unknown option"
done

# the read-only commands stay read-only: nothing under state/ changes when they run
"$MJ" update >/dev/null
before="$(find .majordomus/state -type f -exec shasum -a 256 {} \; | sort)"
"$MJ" doctor   >/dev/null 2>&1 || true
"$MJ" watch    >/dev/null 2>&1 || true
"$MJ" doctrine >/dev/null 2>&1 || true
after="$(find .majordomus/state -type f -exec shasum -a 256 {} \; | sort)"
[ "$before" = "$after" ] || { echo "    a read-only command wrote to .majordomus/state"; exit 1; }

# majordomus-covers: none
# New shell is refused unless the automation inventory exempts it (ADR 0069).
#
# No `majordomus-covers:` command: `shell check` is the Rust executable's, not the shell
# tool's. The gate is driven against a repository this case builds, so what is asserted is
# the check's behaviour rather than this checkout's inventory (case 366 asks that):
#
#   a new `scripts/foo` with a bash interpreter line fails, naming the file and the remedy;
#   a record with an exemption makes it pass;
#   deleting the file makes the record stale, and the stale record fails;
#   an exemption without its removal condition fails, naming the field;
#   a unit outside the governed directories, and a file that is not shell, are not units.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
"$MJ" init >/dev/null

git init -q . 2>/dev/null || true
INV=.ai/repo/automation/inventory.jsonl
mkdir -p scripts test/cases .ai/repo/automation

echo "  a repository with no shell under a governed directory passes"
expect_exit 0 "$RB" shell check --repo .
expect_grep 'shell check: clean'

echo "  a new scripts/foo with a bash interpreter line is refused by name, with the remedy"
printf '#!/usr/bin/env bash\necho foo\n' > scripts/foo
expect_exit 10 "$RB" shell check --repo .
expect_grep 'shell\.undeclared +scripts/foo'
expect_grep 'scripted capability'

echo "  the same verdict is a document"
expect_exit 10 "$RB" shell check --repo . --format json
expect_grep '"passes": false'
expect_grep '"path": "scripts/foo"'

echo "  a file that is not shell, and shell outside the governed directories, are not units"
printf '#!/usr/bin/env node\n' > scripts/tool.mjs
printf '#!/usr/bin/env bash\n' > test/cases/1_x.sh
expect_exit 10 "$RB" shell check --repo .
expect_no_grep 'tool\.mjs'
expect_no_grep 'test/cases'

echo "  an exemption that carries why and removal makes it pass"
printf '%s\n' '{"unit":"scripts/foo","disposition":"B","reason":"orchestrates two commands","exemption":{"why":"no scripted capability runs it yet","removal":"a scripted capability replaces it"}}' > "$INV"
expect_exit 0 "$RB" shell check --repo .
expect_grep 'exemptions by disposition: B 1'

echo "  an exemption without its removal condition is refused, naming the field"
printf '%s\n' '{"unit":"scripts/foo","disposition":"B","reason":"orchestrates two commands","exemption":{"why":"no scripted capability runs it yet"}}' > "$INV"
expect_exit 10 "$RB" shell check --repo .
expect_grep 'shell\.missing_field'
expect_grep 'exemption\.removal'

echo "  deleting the file makes the exemption stale, and the stale exemption is refused"
printf '%s\n' '{"unit":"scripts/foo","disposition":"B","reason":"orchestrates two commands","exemption":{"why":"no scripted capability runs it yet","removal":"a scripted capability replaces it"}}' > "$INV"
rm scripts/foo
expect_exit 10 "$RB" shell check --repo .
expect_grep 'shell\.stale +scripts/foo'

echo "  deleting the record with the file passes again: the list only shrinks"
: > "$INV"
expect_exit 0 "$RB" shell check --repo .

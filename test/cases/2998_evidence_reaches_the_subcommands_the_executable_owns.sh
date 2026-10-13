# majordomus-covers: none
# `majordomus evidence show|claim|proves|record|stamp` answer through the launcher.
#
# Two programs answer to `majordomus evidence`. The shell tool records what a task owes
# (`--covers`, `--gate`); the Rust executable holds the ledger of what ran (`show`, `claim`,
# `proves`, `record`, `stamp`). docs/EVIDENCE.md, docs/PREFLIGHT.md and docs/SURFACES.md name
# `majordomus evidence record`, `majordomus evidence stamp` and `majordomus evidence show`,
# and a person with an installed tool has one program on PATH: `majordomus`. Its dispatch
# listed `evidence` among the shell tool's own commands, so the word went to an option
# parser that knows only options and answered `evidence: unknown option record` (seen
# adopting 0.21.0 and 0.22.0 in a second repository, docs/ADOPTION_SECOND_RUN.md G17).
#
# What is held here, in a checkout and in an installed layout:
#
#   1. every subcommand the executable declares under `evidence` is reached, and the list is
#      the executable's own declaration (docs/generated/cli.json), not one written here;
#   2. the launcher's global options may stand on either side of the command;
#   3. the shell half is exactly what it was: `--covers`, `--gate`, a bare `evidence`, and a
#      word neither program has;
#   4. `evidence --help` lists both halves, and names no word the dispatch does not reach.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq: the subcommands are read from docs/generated/cli.json"
RB="$(rust_bin)" || rust_bin_exit $?
# the launcher must run that executable and never build one of its own
export MAJORDOMUS_BIN="$RB"
MJ="$ROOT/bin/majordomus"

# ---------------------------------------------------------------- 1. the executable's words
WORDS="$(jq -r '[.. | objects | select(.path? == ["majordomus", "evidence"]) | .subcommands[]?.path[2]] | unique | .[]' \
  "$ROOT/docs/generated/cli.json")"
[ -n "$WORDS" ] || { echo "    docs/generated/cli.json declares no subcommand under majordomus evidence"; exit 1; }
for w in show record stamp; do
  grep -Fxq "$w" <<<"$WORDS" || { echo "    the executable no longer declares evidence $w, which the pages name"; exit 1; }
done
for w in $WORDS; do
  expect_exit 0 "$MJ" evidence "$w" --help || exit 1
  expect_grep "^Usage: majordomus evidence $w" || exit 1
  expect_no_grep 'unknown option' || exit 1
done

# Every `majordomus evidence <word>` a page prints, in a fence, a table or a sentence, is a
# word the launcher reaches. The gate of documented commands (scripts/ci/doc-command-check,
# case 638) could not see this: it reads `majordomus`, `bin/majordomus` and
# `bin/majordomus-cli` as one command, and takes an example the executable ran as proof for
# all three.
# A command, not a sentence: the words open a line of a fence, a code span, or a workflow's
# `run:`. An issue titled "majordomus evidence reaches ..." in a generated table is prose.
DOCUMENTED="$(cat "$ROOT"/docs/*.md "$ROOT/README.md" | grep -oE '(^|`|run: )majordomus evidence [a-z][a-z-]*' | sed 's/.* //' | LC_ALL=C sort -u)"
for w in show record stamp; do
  grep -Fxq "$w" <<<"$DOCUMENTED" || { echo "    no page prints majordomus evidence $w any more; this case no longer reads the pages"; exit 1; }
done
for w in $DOCUMENTED; do
  grep -Fxq "$w" <<<"$WORDS" || { echo "    a page prints majordomus evidence $w, which the executable does not declare"; exit 1; }
done
# and the registry's own syntax line names exactly those words
REG="$(sed -n '/^  - id: evidence$/,/^  - id: /p' "$ROOT/share/commands.yaml" | sed -n 's/^    syntax: .*majordomus evidence <\([a-z|-]*\)>.*/\1/p' | tr '|' '\n' | LC_ALL=C sort)"
[ "$REG" = "$(LC_ALL=C sort <<<"$WORDS")" ] || {
  printf '    share/commands.yaml gives evidence the subcommands\n%s\n    and the executable declares\n%s\n' "$REG" "$WORDS"; exit 1; }

# a subcommand that does something, not only one that describes itself: a repository with a
# claims matrix and no ledger is told that nothing was recorded
mkdir -p docs test/cases
cat > docs/CLAIMS.yaml <<'YAML'
version: 1
claims:
  - id: alpha-holds
    status: guaranteed
    area: Alpha
    claim: "Alpha holds."
    test: test/cases/01_alpha.sh
    page: docs/claims/alpha.md
    command: scripts/alpha-check
YAML
printf '# the alpha case\n' > test/cases/01_alpha.sh
printf '# Sample\n' > README.md
git add -A >/dev/null 2>&1 && git --no-pager commit -qm fixture >/dev/null 2>&1
expect_exit 0 "$MJ" init || exit 1
expect_exit 0 "$MJ" update || exit 1
git add -A >/dev/null 2>&1 && git --no-pager commit -qm layer >/dev/null 2>&1
expect_exit 0 "$MJ" evidence show || exit 1
expect_grep 'nothing has been recorded' || exit 1

# ---------------------------------------------------------------- 2. global options, either side
expect_exit 0 "$MJ" evidence --repo "$T" show || exit 1
expect_grep 'nothing has been recorded' || exit 1
expect_exit 0 "$MJ" --repo "$T" evidence show || exit 1
expect_grep 'nothing has been recorded' || exit 1
expect_exit 0 "$MJ" evidence "--repo=$T" show --format json || exit 1
printf '%s' "$LAST_OUT" | sed -n '/^{/,$p' | jq -e '.ledger.present == false' >/dev/null \
  || { echo "    evidence show --format json did not answer one JSON document: $LAST_OUT"; exit 1; }

# ---------------------------------------------------------------- 3. the shell half is unchanged
# a bare `evidence` is a question and exits 2 with the usage; a word neither program has is
# refused by the shell tool, and nothing is run for it
expect_exit 2 "$MJ" evidence || exit 1
expect_grep '^usage: majordomus evidence --covers' || exit 1
expect_exit 2 "$MJ" evidence nonsense || exit 1
expect_grep 'evidence: unknown option nonsense' || exit 1
# clap's own `help` word is not a subcommand anybody documented; it stays the shell tool's
expect_exit 2 "$MJ" evidence help || exit 1
# an option's value that happens to be one of the executable's words is still a value
expect_exit 0 "$MJ" start "an obligation" --scope README.md --requires tests || exit 1
expect_exit 0 "$MJ" evidence --covers tests --command show || exit 1
expect_grep '^evidence: tests recorded for ' || exit 1
grep -q '"event":"task.evidence".*"covers":"tests".*"command":"show"' .ai/local/state/ledger.jsonl \
  || { echo "    --covers tests --command show did not write the task's evidence line"; exit 1; }
# and the refusals it had are the refusals it has
expect_exit 2 "$MJ" evidence --covers || exit 1
expect_grep 'covers needs a token' || exit 1
expect_exit 2 "$MJ" evidence --covers no-such-token --command true || exit 1
expect_grep "no obligation 'no-such-token'" || exit 1
expect_exit 2 "$MJ" evidence --covers tests --gate lint || exit 1
expect_grep 'a gate or an obligation, not both' || exit 1

# ---------------------------------------------------------------- 4. the help names both halves
expect_exit 0 "$MJ" evidence --help || exit 1
expect_grep '^usage: majordomus evidence --covers <token>' || exit 1
expect_grep '^       majordomus evidence --gate <id> --exit <status>' || exit 1
HELP="$LAST_OUT"
for w in $WORDS; do
  grep -qE "^  $w +[A-Z]" <<<"$HELP" || { echo "    evidence --help does not list the subcommand $w"; exit 1; }
done
# exactly the executable's words: a line of that block naming anything else is a promise the
# dispatch does not keep
listed="$(sed -n '/^usage: majordomus evidence </,$p' <<<"$HELP" | sed -n 's/^  \([a-z][a-z-]*\)  .*/\1/p' | LC_ALL=C sort)"
[ "$listed" = "$(LC_ALL=C sort <<<"$WORDS")" ] || {
  printf '    evidence --help lists\n%s\n    and the executable declares\n%s\n' "$listed" "$WORDS"; exit 1; }

# ---------------------------------------------------------------- an installed layout
# bin/, lib/, libexec/ and share/ with no docs/ (scripts/release-package): the projection the
# checkout reads is not there, so the shipped executable is asked. A stand-in answers
# `help evidence [<word>]` the way clap does and says what it was run with.
unset MAJORDOMUS_BIN MAJORDOMUS_SHARE
I="$T/installed"
mkdir -p "$I/bin" "$I/lib" "$I/libexec" "$I/share"
cp "$ROOT/bin/majordomus" "$ROOT/bin/majordomus-cli" "$I/bin/"
cp "$ROOT"/lib/* "$I/lib/"
cp -R "$ROOT/share/." "$I/share/"
cat > "$I/libexec/majordomus-cli" <<'STUB'
#!/bin/sh
if [ "$1" = help ] && [ "$2" = evidence ]; then
  case "${3:-}" in
    '') cat <<'HELP'
What actually ran and what it proves

Usage: majordomus evidence [OPTIONS] <COMMAND>

Commands:
  show    Every claim against the evidence recorded for it
  record  Record a run that happened into the ledger
  stamp   Measure the checkout as a run left it
  help    Print this message or the help of the given subcommand(s)

Options:
      --repo <PATH>
HELP
      exit 0 ;;
    show|record|stamp) exit 0 ;;
    *) echo "error: unrecognized subcommand '$3'" >&2; exit 2 ;;
  esac
fi
[ "$1" = help ] && exit 2
echo "executable ran: $*"
STUB
chmod +x "$I/bin/majordomus" "$I/bin/majordomus-cli" "$I/libexec/majordomus-cli"
[ ! -e "$I/docs" ] || { echo "    the installed layout must carry no docs/"; exit 1; }
MJI() { env -u MAJORDOMUS_BIN -u MAJORDOMUS_SHARE -u MAJORDOMUS_LAUNCHER "$I/bin/majordomus" "$@"; }

# the lines the pages print, arguments unchanged
expect_exit 0 MJI evidence record --suite suite.tsv --provenance suite.provenance.json || exit 1
expect_grep '^executable ran: evidence record --suite suite.tsv --provenance suite.provenance.json$' || exit 1
expect_exit 0 MJI evidence stamp --producer suite --report suite.tsv --out suite.provenance.json || exit 1
expect_grep '^executable ran: evidence stamp --producer suite --report suite.tsv --out suite.provenance.json$' || exit 1
expect_exit 0 MJI evidence show || exit 1
expect_grep '^executable ran: evidence show$' || exit 1
expect_exit 0 MJI evidence --repo "$T" show || exit 1
expect_grep "^executable ran: evidence --repo $T show\$" || exit 1

# a word the shipped executable does not have is the shell tool's to refuse, and nothing ran
expect_exit 2 MJI evidence claim alpha-holds || exit 1
expect_grep 'evidence: unknown option claim' || exit 1
expect_no_grep 'executable ran' || exit 1

# the shell half does not reach the executable for its own options
expect_exit 0 MJI evidence --covers tests --command 'record' || exit 1
expect_grep '^evidence: tests recorded for ' || exit 1
expect_no_grep 'executable ran' || exit 1

# and the help lists what this installation's executable has, no more
expect_exit 0 MJI evidence --help || exit 1
expect_grep '^usage: majordomus evidence --covers <token>' || exit 1
expect_grep '^usage: majordomus evidence <show\|record\|stamp> \[options\]$' || exit 1
expect_grep '^  record +Record a run that happened into the ledger$' || exit 1
expect_no_grep '^  (claim|proves|help) ' || exit 1

echo "    ok: evidence $(printf '%s' "$WORDS" | tr '\n' ' ') reach the executable; --covers and --gate stay the shell tool's"

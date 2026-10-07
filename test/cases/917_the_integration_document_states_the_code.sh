# majordomus-covers: none
# docs/INTEGRATION.md states what the code does, and a gate holds it to that (audit gap G5).
#
# The document carried only `mermaid` and `text` fences, so doc-command-check extracted
# nothing from it: every command and every number in it was held by nobody, and a constant
# changed in the code left the sentence about it standing. Three things are held here.
#
#   1. Its commands are in `bash` fences the gate extracts, so each is one the project runs
#      (the reading commands), or carries the reason it cannot run here (the acting ones).
#   2. The numbers it states about the executor, each listed below, are the constants the
#      code declares: read from the Rust source by name, never copied here a third time.
#   3. Its Commands table and the command line are the same list: every `prs` subcommand has
#      a row, and every row names a subcommand that exists.
#
# A check nobody has watched fail is a check nobody knows works, so each is planted against a
# copy of the document with one fact changed.
. "$ROOT/test/lib.sh"

DOC="$ROOT/docs/INTEGRATION.md"
SRC="$ROOT/apps/majordomus-cli/src"

# the value a constant is declared with, as the source spells it
const_of() { # <file under src> <NAME>
  sed -n "s/^pub const $2: [^=]*= *\\(.*\\);\$/\\1/p" "$SRC/$1" | head -n 1
}

# <document> — every stated number against its constant; prints one line per disagreement
numbers() {
  local doc="$1" flat bad=0
  flat="$(tr '\n' ' ' < "$doc" | tr -s ' ')"
  says() { # <file> <NAME> <the source's value> <the document's words>
    local got; got="$(const_of "$1" "$2")"
    [ "$got" = "$3" ] || { echo "    $1 declares $2 = '$got', and this case expects '$3': update the case and the document together"; bad=1; }
    case "$flat" in *"$4"*) ;; *) echo "    the document does not say '$4' ($1: $2 = $3)"; bad=1 ;; esac
  }
  says integration/drain.rs ROLLOUT_MERGES_BEFORE_CONTINUOUS '5' 'five verified merges'
  says integration/drain.rs LEASE_STALE_AFTER 'Duration::from_secs(1800)' '30 minutes'
  says integration/drain.rs INTERVAL_SECONDS '30..=900' '30 to 900'
  says integration/drain.rs CONTINUOUS_TRANSIENT_LIMIT '3' 'at most three consecutive cycles'
  says integration/drain.rs DRAIN_STEPS_PER_MERGE '3' 'three steps for each merge asked for'
  says integration/drain.rs REFRESHED_HEAD_REPORTS_WITHIN 'Duration::from_secs(4 * 3600)' 'four hours'
  says integration/wait.rs STARVING_AFTER '3' 'passed over three times'
  says integration/mod.rs OBSERVATION_STALE_AFTER 'std::time::Duration::from_secs(3600)' 'than an hour'
  says integration/forge.rs OPEN_LIMIT '500' 'at most 500 open'
  says integration/forge.rs RESOLVED_LIMIT '200' 'newest 200'
  says integration/forge.rs MERGED_LIMIT '1000' 'newest 1000 merged'
  says integration/bounded.rs FORGE_READ 'Duration::from_secs(60)' '60 seconds'
  says integration/bounded.rs FORGE_WRITE 'Duration::from_secs(120)' '120 seconds'
  says integration/bounded.rs GIT_TRANSFER 'Duration::from_secs(300)' '300 seconds'
  # the retry policy is one value with fields, read as the source spells them
  grep -A3 '^pub const FORGE: Backoff' "$SRC/integration/retry.rs" | tr -d ' \n' | grep -q 'attempts:4,initial:Duration::from_secs(2),' \
    || { echo "    retry::FORGE is no longer four attempts starting at 2 s: update the case and the document together"; bad=1; }
  case "$flat" in *'at most four attempts with waits of 2, 4 and 8 seconds'*) ;; *) echo "    the document does not say 'at most four attempts with waits of 2, 4 and 8 seconds'"; bad=1 ;; esac
  # the interval's default is an argument's, not a constant's
  grep -q 'default_value_t = 300, value_parser = clap::value_parser!(u64).range(crate::integration::drain::INTERVAL_SECONDS)' "$SRC/cli.rs" \
    || { echo "    the interval's default in cli.rs is no longer 300"; bad=1; }
  case "$flat" in *'300 by default'*) ;; *) echo "    the document does not say '300 by default'"; bad=1 ;; esac
  return "$bad"
}

# the subcommands of `majordomus prs`, as the command line declares them: the variants of
# PrsCommand, in kebab case
subcommands() {
  awk '/^pub enum PrsCommand/{on=1;next} on&&/^}/{exit} on&&/^    [A-Z][A-Za-z]*( \{|,)/{print $1}' "$SRC/cli.rs" \
    | tr -d ',{' | sed -E 's/([a-z])([A-Z])/\1-\2/g' | tr '[:upper:]' '[:lower:]' | LC_ALL=C sort -u
}
# <document> — the subcommands its Commands table has a row for
rows() {
  awk '/^## Commands/{on=1;next} on&&/^## /{exit} on&&/^\| `majordomus prs/' "$1" \
    | grep -oE '`majordomus prs [a-z-]+|/ `prs [a-z-]+' | awk '{print $NF}' | LC_ALL=C sort -u
}
table() {
  local doc="$1"
  diff <(subcommands) <(rows "$doc") > "$T/table.diff" || {
    echo "    the Commands table and PrsCommand disagree (< the command line, > the document):"
    sed 's/^/      /' "$T/table.diff"; return 1; }
}

echo "    the document's commands are extracted by the gate"
listed="$("$ROOT/scripts/ci/doc-command-check" --list 2>/dev/null | grep -c '^docs/INTEGRATION.md' || true)"
[ "${listed:-0}" -ge 10 ] || { echo "    doc-command-check extracts $listed command(s) from docs/INTEGRATION.md; its fences are not the kind the gate reads"; exit 1; }

echo "    the numbers it states are the constants the code declares"
numbers "$DOC"

echo "    its Commands table is the command line's subcommands"
[ -n "$(subcommands)" ] || { echo "    no subcommand was read from cli.rs"; exit 1; }
table "$DOC"

echo "    planted: a number that is not the code's, and a row for a command that does not exist"
sed 's/five verified merges/three verified merges/' "$DOC" > "$T/wrong-number.md"
if numbers "$T/wrong-number.md" > "$T/planted.out" 2>&1; then
  echo "    a document that says three where the code says five passed"; exit 1; fi
grep -q "does not say 'five verified merges'" "$T/planted.out" || { cat "$T/planted.out"; exit 1; }
sed 's/^| `majordomus prs plan` /| `majordomus prs schedule` /' "$DOC" > "$T/wrong-row.md"
if table "$T/wrong-row.md" > "$T/planted.out" 2>&1; then
  echo "    a table naming a command that does not exist passed"; exit 1; fi
grep -q 'schedule' "$T/planted.out" || { cat "$T/planted.out"; exit 1; }

# majordomus-covers: none
# Every failure class in the published table names its proof (I2731, milestone
# failure-is-visible, evidence failure_class_table).
#
# The milestone's outcome is that "each failure class in a published table names its test or
# its stated limitation". On 2026-10-15 the table in docs/MCP.md ("Failure behaviour") said
# what each failure does and nothing about what shows it, and the runtime audit of 2026-10-09
# named classes no test drives (disk full, a read-only journal directory, a frozen worker, a
# kill between lease creation and its first write). A table of behaviours with no proof column
# reads as a list of guarantees. The table now carries a third column, and this case holds it:
#
#   1  the real table: header `| state | what happens | proof |`, at least one row, and every
#      row passes the rules below
#   2  a row whose proof cell is empty, or holds prose that is neither a citation nor a
#      `limitation:`, is refused and named
#   3  a row citing a test function or a case file this tree does not hold is refused, and so
#      is a citation shaped almost like one (a test file with no `::function`)
#   4  a `limitation:` with no reason after it is refused
#   5  the table missing, or its proof column missing, is refused rather than read as clean
#
# A citation is `apps/majordomus-cli/tests/<file>.rs::<function>` (the file exists and
# declares `fn <function>(`) or `test/cases/<n>_<name>.sh` (the file exists). A limitation is
# a cell that begins `limitation:` and gives its reason; it may cite tests too, and those must
# resolve as well. The case reads the table only: it does not run the tests it names, which
# the rust and suite lanes do.
. "$ROOT/test/lib.sh"

# failure_table_check DOC TREE: exit 0 when every row of DOC's "Failure behaviour" table names
# a resolvable proof under TREE or a stated limitation; exit 10 naming each row that does not.
failure_table_check() {
  local doc="$1" tree="$2" section header rows bad=0 n=0 row state proof refs ref file fn
  [ -f "$doc" ] || { echo "no table: $doc does not exist"; return 10; }
  section="$(awk '/^## Failure behaviour$/{on=1; next} on && /^## /{exit} on' "$doc")"
  [ -n "$section" ] || { echo "no table: $doc has no '## Failure behaviour' section"; return 10; }
  header="$(grep -E '^\| state \|' <<<"$section" | head -1)"
  [ "$header" = '| state | what happens | proof |' ] \
    || { echo "no proof column: the header reads '${header:-nothing}'"; return 10; }
  rows="$(awk '/^\|---/{on=1; next} on && /^\| /{print} on && !/^\|/{exit}' <<<"$section")"
  [ -n "$rows" ] || { echo "no rows: the table is empty"; return 10; }
  while IFS= read -r row; do
    n=$((n + 1))
    # cells hold no `|`, so the row is split on it and each cell trimmed
    cell() { awk -F'|' -v i="$1" '{c = $(i + 1); gsub(/^ +| +$/, "", c); print c}' <<<"$row"; }
    state="$(cell 1)"; proof="$(cell 3)"
    if [ "$(awk -F'|' '{print NF - 2}' <<<"$row")" != 3 ]; then
      echo "row $n ($state): not three cells"; bad=1; continue
    fi
    # anything backticked under the two test roots must be a whole, resolvable citation
    refs="$(grep -oE '`(apps/majordomus-cli/tests/|test/cases/)[^`]*`' <<<"$proof" | tr -d '`' || true)"
    while IFS= read -r ref; do
      [ -n "$ref" ] || continue
      case "$ref" in
        apps/majordomus-cli/tests/*.rs::*)
          file="${ref%%::*}"; fn="${ref#*::}"
          if [ ! -f "$tree/$file" ]; then
            echo "row $n ($state): cites $file, which this tree does not hold"; bad=1
          elif ! grep -qE "fn ${fn}\(" "$tree/$file"; then
            echo "row $n ($state): cites $ref, and $file declares no fn $fn"; bad=1
          fi ;;
        test/cases/*.sh)
          [ -f "$tree/$ref" ] \
            || { echo "row $n ($state): cites $ref, which this tree does not hold"; bad=1; } ;;
        *) echo "row $n ($state): '$ref' is not a citation (file.rs::function or a case file)"
           bad=1 ;;
      esac
    done <<<"$refs"
    case "$proof" in
      'limitation: '?????????*) ;;
      limitation:*) echo "row $n ($state): a limitation that gives no reason"; bad=1 ;;
      *) [ -n "$refs" ] \
           || { echo "row $n ($state): names neither a test nor a limitation"; bad=1; } ;;
    esac
  done <<<"$rows"
  [ "$bad" = 0 ] || return 10
  echo "failure table: $n row(s), each names its proof"
}

DOC="$ROOT/docs/MCP.md"

# ---------------------------------------------------------------- 1. the published table holds
expect_exit 0 failure_table_check "$DOC" "$ROOT" \
  || { echo "    a row of docs/MCP.md's failure table names no resolvable proof"; exit 1; }
expect_grep '^failure table: [0-9]+ row\(s\), each names its proof$' || exit 1
# the classes the audit found untested are in the table, each as a stated limitation
for class in 'the disk is full' 'refuses writes \(read-only\)' 'a frozen worker' \
  'killed between creating the lease and its first write'; do
  grep -E "^\| [^|]*${class}[^|]* \| [^|]* \| limitation: " "$DOC" >/dev/null \
    || { echo "    the class /$class/ is not in the table as a stated limitation"; exit 1; }
done

# fixtures: a tree holding one test function and one case, and a table built row by row
mkdir -p tree/apps/majordomus-cli/tests tree/test/cases
printf '#[test]\nfn a_thing_is_proven() {}\n' > tree/apps/majordomus-cli/tests/thing.rs
printf '# a case\n' > tree/test/cases/7_a_case.sh
table() {
  { printf '# Doc\n\n## Failure behaviour\n\nIntro.\n\n| state | what happens | proof |\n|---|---|---|\n'
    printf '| a held class | it holds | `apps/majordomus-cli/tests/thing.rs::a_thing_is_proven` |\n'
    printf '| a cased class | it holds | `test/cases/7_a_case.sh` |\n'
    printf '| a stated class | unmeasured | limitation: not tested, because nothing can fill it |\n'
    local extra; for extra in "$@"; do printf '%s\n' "$extra"; done
    printf '\n## Next\n\n| not | this | table |\n|---|---|---|\n| x | y | |\n'
  } > fixture.md
}
table
expect_exit 0 failure_table_check fixture.md tree \
  || { echo "    a fixture whose every row names its proof was refused"; exit 1; }
expect_grep 'failure table: 3 row' || { echo "    the next section's table was read as rows"; exit 1; }

# ---------------------------------------------------------------- 2. a row with neither
table '| an unproven class | it fails | |'
expect_exit 10 failure_table_check fixture.md tree \
  || { echo "    a row with an empty proof cell passed"; exit 1; }
expect_grep 'row 4 \(an unproven class\): names neither a test nor a limitation' || exit 1
table '| a hopeful class | it fails | covered elsewhere |'
expect_exit 10 failure_table_check fixture.md tree \
  || { echo "    a proof cell of bare prose passed"; exit 1; }
expect_grep 'row 4 \(a hopeful class\): names neither' || exit 1

# ---------------------------------------------------------------- 3. a citation that does not resolve
table '| a ghost | it fails | `apps/majordomus-cli/tests/thing.rs::a_test_nobody_wrote` |'
expect_exit 10 failure_table_check fixture.md tree \
  || { echo "    a citation of a function the file does not declare passed"; exit 1; }
expect_grep 'row 4 \(a ghost\): cites .*thing.rs::a_test_nobody_wrote, and .* declares no fn' || exit 1
table '| a lost file | it fails | `apps/majordomus-cli/tests/gone.rs::a_thing_is_proven` |'
expect_exit 10 failure_table_check fixture.md tree || exit 1
expect_grep 'row 4 \(a lost file\): cites apps/majordomus-cli/tests/gone.rs, which this tree does not hold' || exit 1
table '| a lost case | it fails | `test/cases/9999_never_written.sh` |'
expect_exit 10 failure_table_check fixture.md tree \
  || { echo "    a citation of a case file the tree does not hold passed"; exit 1; }
expect_grep 'row 4 \(a lost case\): cites test/cases/9999_never_written.sh' || exit 1
table '| a vague one | it fails | `apps/majordomus-cli/tests/thing.rs` |'
expect_exit 10 failure_table_check fixture.md tree \
  || { echo "    a test file cited with no function passed"; exit 1; }
expect_grep "row 4 \\(a vague one\\): 'apps/majordomus-cli/tests/thing.rs' is not a citation" || exit 1
# a limitation's own citations must resolve too
table '| a hedged one | unmeasured | limitation: not tested; see `test/cases/9999_never_written.sh` |'
expect_exit 10 failure_table_check fixture.md tree \
  || { echo "    a limitation citing a missing case passed"; exit 1; }

# ---------------------------------------------------------------- 4. a limitation with no reason
table '| a shrug | unmeasured | limitation: |'
expect_exit 10 failure_table_check fixture.md tree \
  || { echo "    a limitation with no reason passed"; exit 1; }
expect_grep 'row 4 \(a shrug\): a limitation that gives no reason' || exit 1

# ---------------------------------------------------------------- 5. no table is not a clean table
printf '# Doc\n\n## Failure behaviour\n\n| state | what happens |\n|---|---|\n| a | b |\n' > fixture.md
expect_exit 10 failure_table_check fixture.md tree \
  || { echo "    a table without its proof column passed"; exit 1; }
expect_grep 'no proof column' || exit 1
printf '# Doc\n\n## Something else\n' > fixture.md
expect_exit 10 failure_table_check fixture.md tree \
  || { echo "    a document with no failure table passed"; exit 1; }
expect_exit 10 failure_table_check missing.md tree || exit 1

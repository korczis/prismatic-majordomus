# majordomus-covers: none
# A gap the audit calls planned names a plan object that exists (I1510, gate audit-plan).
#
# docs/COCKPIT_IDE_AUDIT.md's gap matrix says that "Planned" requires a plan object, and
# nothing read it: a row could claim to be scheduled with nothing scheduling it, and the
# document's own search showed the IDE work was in no plan at all. scripts/ci/audit-plan-check
# reads the matrix and the plan together. This case plants each shape in a fixture tree —
# a planned row naming an issue that exists, one naming an id the plan does not hold, one
# naming none, and rows the check must leave alone — and holds this checkout to the gate,
# then mutates a copy of the real matrix to show the parser reads the real rows.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/audit-plan-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }
AUDIT="docs/COCKPIT_IDE_AUDIT.md"

# ---------------------------------------------------------------- this checkout
expect_exit 0 "$GATE"
expect_grep "planned row\(s\) of [0-9]+ in the gap matrix of docs/COCKPIT_IDE_AUDIT\.md, each naming an issue that exists"

# ---------------------------------------------------------------- a fixture tree
F="$T/tree"
mkdir -p "$F/docs" "$F/.ai/repo/project/issues"
issue() { printf 'id: %s\ntitle: Issue %s\n' "$1" "$1" > "$F/.ai/repo/project/issues/$1.yaml"; }
issue I0101; issue I0102
matrix() { # <rows of the Development table...>
  { printf '# Audit\n\n## 3. Measured\n\n| capability | status | evidence |\n|---|---|---|\n'
    printf '| outside the matrix | planned | as I0999, read by nobody |\n\n'
    printf '## 4. Gap matrix\n\nOne status per capability. "Planned" requires a plan object.\n\n'
    printf '```console\n| fenced | planned | I0998 |\n```\n\n### Development\n\n'
    printf '| capability | status | evidence |\n|---|---|---|\n'
    printf '| capability explorer | canonical+implemented+verified | `tests/cockpit.rs` |\n'
    for r in "$@"; do printf '%s\n' "$r"; done
    printf '\n## 5. Drift audit\n\n| class | status | owner |\n|---|---|---|\n'
    printf '| route arrays | planned | none |\n'; } > "$F/$AUDIT"
}
gate() { MJ_ROOT="$F" expect_exit "$1" "$GATE"; }

# a planned row naming an issue that exists passes; tables outside §4 and fences are not read
matrix '| diff viewing | planned | as `I0101` |'
gate 0
expect_grep '1 planned row\(s\) of 2 in the gap matrix'
expect_no_grep 'I0999|I0998|route arrays'

# bold status and two ids, both existing: still planned, still clean
matrix '| diff viewing | **planned** | I0101, then I0102 |'
gate 0
expect_grep '1 planned row\(s\) of 2'

# a planned row naming an id the plan does not hold: the row and the id are named
matrix '| diff viewing | planned | as I0101 |' '| CI status | planned | as I0404 |'
gate 10
expect_grep "MISSING    docs/COCKPIT_IDE_AUDIT\.md:[0-9]+ row 'CI status' \(Development\) is planned as I0404, which no \.ai/repo/project/issues/I0404\.yaml declares"
expect_no_grep "'diff viewing'"

# one existing id does not excuse a second that is missing
matrix '| diff viewing | planned | I0101 and I0405 |'
gate 10
expect_grep "row 'diff viewing' \(Development\) is planned as I0405"
expect_no_grep 'planned as I0101'

# a planned row naming no issue at all fails, and says so
matrix '| terminal / REPL | planned — soon | no PTY anywhere in `src/` |'
gate 10
expect_grep "UNPLANNED  docs/COCKPIT_IDE_AUDIT\.md:[0-9]+ row 'terminal / REPL' \(Development\) is planned and names no issue"

# an id inside a longer word is not an issue id
matrix '| terminal / REPL | planned | see XI0101 and I0101x |'
gate 10
expect_grep "row 'terminal / REPL' \(Development\) is planned and names no issue"

# rows in the other statuses are evidence claims, not plan claims: a missing id is not read
matrix '| issues | implemented but duplicated | no page — planned as `I0406` |' \
  '| diffs | absent — planned later as I0407 | — |' '| retries | **broken** | I0408 |'
gate 0
expect_grep '0 planned row\(s\) of 4'

# a matrix the gate cannot find is not a pass
printf '# Audit\n\n## 4. Drift audit\n\n| capability | status |\n|---|---|\n| a | planned |\n' > "$F/$AUDIT"
gate 12
printf '# Audit\n\n## 4. Gap matrix\n\nNo table.\n' > "$F/$AUDIT"
gate 12
rm "$F/$AUDIT"
gate 12
matrix '| diff viewing | planned | I0101 |'
rm -r "$F/.ai/repo/project/issues"
gate 12

# ---------------------------------------------------------------- the real matrix, mutated
# copy this checkout's matrix and plan; a row turned `planned` is held to the real issues
rm -rf "$F"; mkdir -p "$F/docs" "$F/.ai/repo/project"
cp -R "$ROOT/.ai/repo/project/issues" "$F/.ai/repo/project/issues"
[ ! -e "$F/.ai/repo/project/issues/I0000.yaml" ] || { echo "    I0000 is an issue; the mutation needs an id that is not"; exit 1; }
grep -qE '^\| diffs \| absent \| ' "$ROOT/$AUDIT" || { echo "    $AUDIT has no 'diffs | absent' row to mutate"; exit 1; }
grep -qE '^\| issues \| implemented but duplicated.*`I1507`' "$ROOT/$AUDIT" \
  || { echo "    $AUDIT has no issues row naming I1507 to mutate"; exit 1; }
[ -f "$ROOT/.ai/repo/project/issues/I1507.yaml" ] || { echo "    I1507 is not an issue of this plan"; exit 1; }

sed 's/^| diffs | absent | /| diffs | planned | /' "$ROOT/$AUDIT" > "$F/$AUDIT"
gate 10
expect_grep "UNPLANNED  docs/COCKPIT_IDE_AUDIT\.md:[0-9]+ row 'diffs' \(Planning and SCM\) is planned and names no issue"

sed 's/^| diffs | absent | — |/| diffs | planned | I0000 |/' "$ROOT/$AUDIT" > "$F/$AUDIT"
gate 10
expect_grep "row 'diffs' \(Planning and SCM\) is planned as I0000, which no "

sed 's/^| issues | implemented but duplicated/| issues | planned/' "$ROOT/$AUDIT" > "$F/$AUDIT"
cmp -s "$ROOT/$AUDIT" "$F/$AUDIT" && { echo "    the mutation did not change $AUDIT"; exit 1; }
gate 0
expect_grep '^audit-plan-check: 1 planned row\(s\) of '

echo "    a planned gap naming no issue, or an id the plan does not hold, is refused with the row and the id; one naming an issue passes; other statuses, other tables and fences are not read; and this checkout's matrix passes"

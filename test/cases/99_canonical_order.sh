# majordomus-covers: none
# The gate of project.canonical-order, driven against fixture trees rather than against this
# checkout: a case that asserted the repository's own counts would be a copy of the baseline
# and would fail the day a comparator is adopted. What is asserted here is the gate's
# behaviour — what it refuses, what it accepts, and what it says while doing it.
#
# The three absolute checks are exercised by writing the violation, because a check nobody
# has seen fail is a check nobody knows works. The ratchet is exercised in both directions:
# a count that rose is a new comparator beside a renderer, and a count that fell is a
# baseline nobody lowered, and the gate has to notice both or the debt drifts.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/order-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }

# A tree with the shape the gate reads: the crate's order module, one other source, the
# directories it counts shell sorts in, and a baseline that matches what is written.
fixture() {
  F="$T/tree$1"
  mkdir -p "$F/apps/majordomus-cli/src" "$F/lib" "$F/scripts" "$F/bin" "$F/test" \
           "$F/share" "$F/site" "$F/.ai/repo"
  cat > "$F/apps/majordomus-cli/src/order.rs" <<'RS'
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}
RS
  printf 'pub fn list() -> Vec<u8> { let mut v = vec![]; v.sort_by(|a, b| a.cmp(b)); v }\n' \
    > "$F/apps/majordomus-cli/src/thing.rs"
  printf 'ids() { LC_ALL=C sort; }\n' > "$F/lib/common.sh"
  ( cd "$F" && MJ_ROOT="$F" "$GATE" --update >/dev/null )
  printf '%s' "$F"
}

# --- a tree that satisfies the rule passes, and says what it measured
F="$(fixture 0)"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep 'no comparator outside order.rs folds case by hand'
expect_grep 'sort sites in the crate outside order.rs: 1, at the baseline'

# --- the baseline the gate writes is the counts, and nothing about this machine
grep -q '^crate_sorts=1$' "$F/.ai/repo/order-baseline.txt" \
  || { echo "    the baseline did not record the one sort site"; exit 1; }
grep -q '^shell_sorts=0$' "$F/.ai/repo/order-baseline.txt" \
  || { echo "    the baseline did not record the pinned shell sort as pinned"; exit 1; }

# --- 1. a case-folded comparator outside order.rs is the duplication the rule exists for
F="$(fixture 1)"
cat >> "$F/apps/majordomus-cli/src/thing.rs" <<'RS'
fn by_label(a: &str, b: &str) -> std::cmp::Ordering { a.to_lowercase().cmp(&b.to_lowercase()) }
RS
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'a case-folded comparator outside order.rs: apps/majordomus-cli/src/thing.rs'
expect_grep 'call crate::order::natural_cmp'

# --- 2. a sort key that renders markup makes a class name part of the sequence
F="$(fixture 2)"
printf 'fn t(rows: &mut Vec<El>) { rows.sort_by_key(|r| r.render()); }\n' \
  >> "$F/apps/majordomus-cli/src/thing.rs"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'a sort key that renders markup'
expect_grep 'order the values, then render them'

# --- 3. a locale-collated comparison orders the output by whoever produced it
F="$(fixture 3)"
printf 'export const by = (a, b) => a.id.localeCompare(b.id);\n' > "$F/scripts/report.mjs"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'localeCompare in a generated or served surface: scripts/report.mjs'

# --- the same word in prose is not a comparator: the rule documents the pattern it refuses
F="$(fixture 4)"
printf '# never write .localeCompare( in a generated surface\n' > "$F/scripts/notes.md"
expect_exit 0 env MJ_ROOT="$F" "$GATE"

# --- 4a. the ratchet refuses a rise: a new collection ordered beside its renderer
F="$(fixture 5)"
printf 'fn more(v: &mut Vec<u8>) { v.sort(); }\n' >> "$F/apps/majordomus-cli/src/thing.rs"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'sort sites in the crate rose from 1 to 2'
expect_grep 'implement crate::order::Ordered'

# --- 4b. and a fall: adopting the canonical order lowers the baseline in the same commit
F="$(fixture 6)"
printf 'pub fn list() -> Vec<u8> { vec![] }\n' > "$F/apps/majordomus-cli/src/thing.rs"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'sort sites in the crate fell from 1 to 0'
expect_grep 'order-check --update'

# --- 4c. an unpinned shell sort is a sequence that depends on the machine that ran it
F="$(fixture 7)"
# written with a placeholder so that this case does not itself count as an unpinned sort
printf 'ids() { %s -u; }\n' sort > "$F/scripts/list.sh"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'unpinned shell sorts rose from 0 to 1'
expect_grep 'LC_ALL=C sort'

# --- a tree with no baseline cannot be judged, and says so rather than passing
F="$T/tree8"; mkdir -p "$F/apps/majordomus-cli/src" "$F/.ai/repo"
expect_exit 12 env MJ_ROOT="$F" "$GATE"
expect_grep 'order-baseline.txt is missing'

# --- this repository is at its own baseline, whatever that baseline currently says
expect_exit 0 env MJ_ROOT="$ROOT" "$GATE"
echo "    the gate refuses a folded comparator, a rendered sort key, a locale comparison, and a debt that moved"

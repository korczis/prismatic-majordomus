# majordomus-covers: none
# A Pages deploy on a fresh master is not refused for claims newer than the ledger it reads
# (I2853). The deploy of 0.30.0 (run 39145730187) failed because master's own validation had
# not yet recorded its evidence: the publication read the previous master's ledger, and three
# claims added since then read `not run`, over a baseline of zero. No run of a test that did not
# exist when the ledger was recorded can be in it. This case holds `scripts/pages
# published-evidence` to that, through the real script and against this repository's own
# history, read-only:
#
#   1  a `not run` claim whose test exists at none of the ledger's commits is published as not
#      yet recorded and is not held against the baseline
#   2  a `not run` claim whose test existed when the ledger was recorded still counts, and is
#      refused over the baseline as before
#   3  a ledger none of whose commits is in this history decides nothing: every claim counts
#   4  a test named with a function (`path::name`) is judged by its file
. "$ROOT/test/lib.sh"

W="$T/w"
mkdir -p "$W"
# the oldest commit this checkout holds: a shallow clone's boundary is still a commit here
old="$(git -C "$ROOT" rev-list --max-parents=0 HEAD | tail -1)"
[ -n "$old" ] || { echo "    the checkout has no history to read"; exit 1; }
git -C "$ROOT" cat-file -e "$old:README.md" 2>/dev/null \
  || { echo "    README.md is not in $old; this case needs a file the oldest commit holds"; exit 1; }
new_test="test/cases/3242_a_claim_newer_than_the_ledger_is_not_yet_recorded.sh"
git -C "$ROOT" cat-file -e "$old:$new_test" 2>/dev/null \
  && { echo "    $new_test is already in $old; this case needs a test the ledger's commit lacks"; exit 1; }

report() {       # report <ledger commit> — an executable answering evidence.report
  local body
  body="$(jq -cn --arg c "$1" --arg new "$new_test" '{output: {
    totals: {"not run": 3, "proven": 10},
    ledger: {commits: [$c]},
    claims: [
      {id: "known-claim", state: "not_run", test_path: "README.md"},
      {id: "new-claim", state: "not_run", test_path: $new},
      {id: "new-function", state: "not_run", test_path: ($new + "::a_function")},
      {id: "proven-claim", state: "proven", test_path: "README.md"}
    ]}}')"
  printf '#!/usr/bin/env bash\nprintf %%s %q\n' "$body" > "$W/bin"
  chmod +x "$W/bin"
}
printf '1\n' > "$W/one.txt"
printf '0\n' > "$W/zero.txt"

# ---------------------------------------------------------------- 1 and 4. newer than the ledger
report "$old"
expect_exit 0 "$ROOT/scripts/pages" published-evidence --bin "$W/bin" --baseline "$W/one.txt"
expect_grep "2 claim\(s\) newer than the ledger, published as not yet recorded: new-claim new-function"
expect_grep "1 claim\(s\) not run, within the baseline of 1"

# ---------------------------------------------------------------- 2. a claim the ledger could have run
expect_exit 10 "$ROOT/scripts/pages" published-evidence --bin "$W/bin" --baseline "$W/zero.txt"
expect_grep "1 claim\(s\) would be published as not run, over the baseline of 0"

# ---------------------------------------------------------------- 3. a ledger outside this history
report "0000000000000000000000000000000000000001"
expect_exit 10 "$ROOT/scripts/pages" published-evidence --bin "$W/bin" --baseline "$W/one.txt"
expect_grep "3 claim\(s\) would be published as not run, over the baseline of 1"
case "$LAST_OUT" in *"newer than the ledger"*) echo "    a ledger outside this history excused a claim"; exit 1 ;; esac

echo "    a claim newer than the ledger is published as not yet recorded; one the ledger could have run still counts; a ledger outside the history excuses nothing"

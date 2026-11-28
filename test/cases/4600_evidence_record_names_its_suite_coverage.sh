# majordomus-covers: evidence
# claims: none
# A suite report is recorded as it is: a partial run is merged and keeps the evidence of the
# cases it did not run. What the recorder owes is to say how much of the suite the report
# covered (I4600), so that a report cut short is seen. On 2026-11-15 an adopter's nested runner
# inherited MJ_TEST_REPORT and truncated a ten-case report to two rows; `evidence record`
# answered "recorded 2 execution(s)" and every skill stayed stale with nothing saying why.
#
# What it must not become:
#
#   a refusal. A deliberate partial run is how one case is re-recorded after a fix.
#
#   a count of something else. The cases counted are the suite's own (test/cases/*.sh), and a
#   name the report holds that the repository lacks is not one of them.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p test/cases
for c in 01_alpha 02_beta 03_gamma; do printf 'true\n' > "test/cases/$c.sh"; done
printf 'not a case\n' > test/cases/README.md
git add -A >/dev/null; git commit -qm "three cases" >/dev/null
S="$(mktemp -d "${TMPDIR:-/tmp}/mj.case.XXXXXX")"   # outside the tree: a report here would be a touched path

# ---------------------------------------------------------------- a report cut short: recorded, and said
printf '01_alpha\tPASS\t1\tserial\n' > "$S/one.tsv"
expect_exit 0 "$MJ" evidence record --suite "$S/one.tsv"
expect_grep 'recorded +1 execution\(s\), 1 passing'
expect_grep 'suite +1 of 3 case\(s\) of the suite are in this report; the other 2 keep their earlier evidence'

# ---------------------------------------------------------------- the same as JSON
"$MJ" evidence record --suite "$S/one.tsv" --format json > "$S/one.json"
jq -e '.suite_coverage == {"in_report": 1, "on_disk": 3}' "$S/one.json" >/dev/null \
  || { echo "    suite_coverage is not {in_report: 1, on_disk: 3}"; cat "$S/one.json"; exit 1; }

# ---------------------------------------------------------------- a name the repository lacks is not a case
printf '01_alpha\tPASS\t1\tserial\nzz_gone\tPASS\t1\tserial\n' > "$S/ghost.tsv"
expect_exit 0 "$MJ" evidence record --suite "$S/ghost.tsv"
expect_grep 'suite +1 of 3 case\(s\) of the suite are in this report'

# ---------------------------------------------------------------- the whole suite: said as whole
printf '01_alpha\tPASS\t1\tserial\n02_beta\tPASS\t1\tserial\n03_gamma\tFAIL\t1\tserial\n' > "$S/all.tsv"
expect_exit 0 "$MJ" evidence record --suite "$S/all.tsv"
expect_grep 'suite +all 3 case\(s\) of the suite are in this report'
expect_no_grep 'keep their earlier evidence'

# ---------------------------------------------------------------- no suite report: no suite line
printf 'test result: ok. 0 passed; 0 failed\n' > "$S/crate.txt"
"$MJ" evidence record --crate-output "$S/crate.txt" > "$S/crate.out" 2>&1 || true
grep -q '^suite ' "$S/crate.out" && { echo "    a recording without a suite report printed a suite line"; cat "$S/crate.out"; exit 1; }
exit 0

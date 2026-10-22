# majordomus-negative
# claims: review-is-never-fabricated
# An accepted decision record is history: a claim the reasoning design refuses, standing
# in it, is reported and not a finding.
#
# An adopter's ADR-0023, accepted long before it adopted Majordomus, says its build
# "Requires Ollama binary". `reasoning check` failed it and doctor printed a FAIL asking, in
# effect, for the record of a decision to be rewritten. The record is right about what was
# decided then; a later design does not make it wrong. A proposed record is still a decision
# being made and stays a finding, or the check would stop saying anything about new ones.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
reasoning_fixture

expect_exit 0 rz reasoning check
expect_grep "no finding"

mkdir -p "$R/docs/adrs"
adr() { # file, status line
  printf '# ADR\n\n%s\n\n## Consequences\n\n- **Dependency:** Requires Gemini CLI on build systems\n' "$2" > "$R/docs/adrs/$1"
}

# accepted, in each form the record formats in use write it: reported, not failed
for status in '**Status:** Accepted  ' 'Status: accepted' $'---\nstatus: superseded\n---' $'## Status\n\nImplemented (2025-03-01)'; do
  adr adr-0023-history.md "$status"
  expect_exit 0 rz reasoning check
  expect_grep "no finding"
  expect_grep "INFO claims +docs/adrs/adr-0023-history.md: an accepted decision record says 'requires gemini cli'; it is history"
  # the document alone: the executable logs on stderr, and expect_exit keeps both
  json="$(rz reasoning check --format json 2>/dev/null)"
  [ "$(printf '%s' "$json" | jq -r '.ok')" = true ] || { echo "    ok is not true for an accepted record"; exit 1; }
  [ "$(printf '%s' "$json" | jq -r '.history | length')" = 1 ] || { echo "    history does not carry the accepted record"; exit 1; }
done
rm "$R/docs/adrs/adr-0023-history.md"

# proposed, or no status at all: a decision being made, and the check holds it
for status in 'Status: Proposed' '# no status'; do
  adr adr-0124-new.md "$status"
  expect_exit 10 rz reasoning check
  expect_grep "FAIL claims +docs/adrs/adr-0124-new.md: asserts 'requires gemini cli'"
done
rm "$R/docs/adrs/adr-0124-new.md"

# an accepted status outside a decision-record directory is prose, not a record
printf 'Status: Accepted\n\nThis workflow requires Gemini CLI.\n' > "$R/docs/GUIDE.md"
expect_exit 10 rz reasoning check
expect_grep "FAIL claims +docs/GUIDE.md"
rm "$R/docs/GUIDE.md"

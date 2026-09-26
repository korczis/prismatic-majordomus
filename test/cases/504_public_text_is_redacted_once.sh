# majordomus-covers: none
# Text a run printed is redacted by one rule set, whichever implementation applies it.
#
# The prompt archive has redacted credentials with `lib/capture.sh`'s table since it began;
# evidence published to the site needs the same guarantee from Rust, which has no regular
# expression engine and so carries a hand-written port (apps/majordomus-cli/src/redaction.rs).
# Two implementations of one table drift the day somebody edits one of them. What holds them
# together is one fixture that both are run against: this case runs every line of it through
# the shell's `mj_capture_redact`, and `tests/redaction.rs` runs the same lines through the
# port, each against the expectation the line states. Equal to one expectation on every line
# is equal to each other on every line.
#
# What is proved here:
#   - every fixture line, assembled from its prefix and body, comes out of the shell redactor
#     as its expectation says: the named marker for a credential, the line unchanged for a
#     near-miss, and the kinds the capture record would carry agree with it;
#   - every shape of the table and the assignment rule is exercised both ways, so a fixture
#     that lost a shape is refused rather than passed;
#   - the Rust test reads this very fixture, so the two halves cannot silently diverge onto
#     different data;
#   - the check can fail: an empty fixture, and a copy with one expectation flipped, are both
#     refused. A check nobody has watched fail is a check nobody knows works.
. "$ROOT/test/lib.sh"
FIXTURE="$ROOT/test/fixtures/redaction/shapes.tsv"
RUST_TEST="$ROOT/apps/majordomus-cli/tests/redaction.rs"
expect_file "$FIXTURE"
expect_file "$RUST_TEST"
# shellcheck source=/dev/null
. "$ROOT/lib/capture.sh"
TAB="$(printf '\t')"

# check_fixture <file>: every line through mj_capture_redact against its expectation. Prints
# each disagreement and returns 1 on any, or when a shape is not exercised both ways.
check_fixture() {
  local file="$1" bad=0 name prefix body expect line got want kinds want_kinds shape
  local seen=""
  while IFS="$TAB" read -r name prefix body expect; do
    [ "$name" = name ] && [ "$expect" = expect ] && continue
    if [ -z "$name" ] || [ -z "$prefix" ] || [ -z "$body" ] || [ -z "$expect" ]; then
      printf '    a fixture line has an empty column: %s|%s|%s|%s\n' \
        "$name" "$prefix" "$body" "$expect"
      bad=1; continue
    fi
    line="$prefix$body"
    got="$(printf '%s\n' "$line" | mj_capture_redact)"
    case "$expect" in
      redacted)
        # the assignment rule keeps the name and the separator, which is the whole prefix
        want="[redacted:$name]"
        [ "$name" != assignment ] || want="${prefix}[redacted:assignment]"
        want_kinds="$name" ;;
      kept) want="$line"; want_kinds="" ;;
      *) printf '    %s: the expectation is neither redacted nor kept: %s\n' "$name" "$expect"
         bad=1; continue ;;
    esac
    if [ "$got" != "$want" ]; then
      printf '    %s (%s, body of %s): expected %s, got %s\n' \
        "$name" "$expect" "${#body}" "$want" "$got"
      bad=1
    fi
    kinds="$(mj_capture_redacted_kinds "$got")"
    if [ "$kinds" != "$want_kinds" ]; then
      printf '    %s (%s): the record would name the kinds [%s], expected [%s]\n' \
        "$name" "$expect" "$kinds" "$want_kinds"
      bad=1
    fi
    seen="$seen $name:$expect"
  done < "$file"
  # every shape of the table, then the assignment rule, both ways
  for shape in $(printf '%s\n' "$MJ_CAPTURE_SECRETS" | cut -f1) assignment; do
    for expect in redacted kept; do
      case "$seen " in
        *" $shape:$expect "*) ;;
        *) printf '    the fixture has no %s line for %s\n' "$expect" "$shape"; bad=1 ;;
      esac
    done
  done
  return "$bad"
}

# --- the fixture holds on the shell's redactor
rc=0; check_fixture "$FIXTURE" > "$T/real.out" || rc=$?
[ "$rc" = 0 ] \
  || { echo "    the shell redactor disagrees with the fixture:"; cat "$T/real.out"; exit 1; }

# --- one fixture drives both halves: the Rust test reads this path, not a copy of its own
grep -qF 'test/fixtures/redaction/shapes.tsv' "$RUST_TEST" \
  || { echo "    tests/redaction.rs does not read test/fixtures/redaction/shapes.tsv"; exit 1; }

# --- an empty fixture is refused, not passed for having no line that disagrees
: > "$T/empty.tsv"
rc=0; check_fixture "$T/empty.tsv" > "$T/empty.out" || rc=$?
[ "$rc" != 0 ] || { echo "    an empty fixture passed the check"; exit 1; }
expect_grep 'no redacted line for anthropic-key' "$T/empty.out"

# --- one expectation flipped is caught, in either direction
awk -F'\t' -v OFS='\t' '!done && $4 == "redacted" { $4 = "kept"; done = 1 } { print }' \
  "$FIXTURE" > "$T/flipped.tsv"
rc=0; check_fixture "$T/flipped.tsv" > "$T/flipped.out" || rc=$?
[ "$rc" != 0 ] || { echo "    a redacted line marked kept passed the check"; exit 1; }
expect_grep 'anthropic-key \(kept' "$T/flipped.out"
awk -F'\t' -v OFS='\t' '!done && $4 == "kept" { $4 = "redacted"; done = 1 } { print }' \
  "$FIXTURE" > "$T/flipped2.tsv"
rc=0; check_fixture "$T/flipped2.tsv" > "$T/flipped2.out" || rc=$?
[ "$rc" != 0 ] || { echo "    a kept line marked redacted passed the check"; exit 1; }
expect_grep 'anthropic-key \(redacted' "$T/flipped2.out"

echo "    the shell redacts every fixture line as stated, and the Rust test reads the same fixture"

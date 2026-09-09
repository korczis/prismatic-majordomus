# majordomus-covers: none
# The public API quality gate, through the built binary, against repositories this case
# makes rather than against the one in the checkout.
#
# What is proved here: a repository that carries no Rust crate is answered rather than
# failed, because a rule that cannot apply is not a violation; a crate whose exported
# surface is documented, exampled and tested reports nothing; each way of getting it wrong
# reports its own stable code and no other; every finding names the rule that requires it
# and what to do about it; the JSON and the human rendering agree about the outcome and the
# exit code agrees with both; an unknown code is refused rather than answered with an empty
# list; and the ratchet works in both directions — a finding the baseline records does not
# fail the gate, and one it does not record does.
#
# Nothing here lists a module, an item or a command: every expectation is about the shape of
# the answer, so a crate that grows tomorrow is held to the same contract without this file
# being edited.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    skip: jq not installed"; exit 0; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj96q.XXXXXX")"; trap 'rm -rf "$S"' EXIT
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

# A repository of the layer, from the distribution's own skeleton, so this case carries no
# copy of what a repository is. Committed, because the layer's discovery contract is the
# version-control index.
make_repo() {
  r="$S/$1"; mkdir -p "$r"
  cp -R "$ROOT/share/skeleton/ai" "$r/.ai"
  ( cd "$r" && git init -q . \
    && git add -A >/dev/null 2>&1 \
    && git -c user.email=t@t -c user.name=t commit -qm init >/dev/null 2>&1 ) \
    || { echo "    could not make a fixture repository"; exit 1; }
  echo "$r"
}

# The crate under the path the executable measures, with `lib.rs` as given.
make_crate() {
  d="$1/apps/majordomus-cli/src"; mkdir -p "$d"
  printf '[package]\nname = "x"\n' > "$1/apps/majordomus-cli/Cargo.toml"
  cat > "$d/lib.rs"
}

# ---------------------------------------------------------------- no crate is not a failure
BARE="$(make_repo bare)"
expect_exit 0 sh -c "cd '$BARE' && '$RB' quality report --format json > '$S/bare.json' 2>/dev/null" || exit 1
[ "$(jq -r .measured "$S/bare.json")" = false ] || { echo "    a repository with no crate was measured"; exit 1; }
[ "$(jq -r .passes "$S/bare.json")" = true ] || { echo "    a repository with no crate was failed"; exit 1; }
jq -e '.reason | test("apps/majordomus-cli")' "$S/bare.json" >/dev/null \
  || { echo "    it does not say which crate it looked for"; exit 1; }

# ---------------------------------------------------------------- a clean crate reports nothing
CLEAN="$(make_repo clean)"
make_crate "$CLEAN" <<'RS'
//! A crate that does one thing, and this header says at some length what that one thing is,
//! which is what the module policy asks of every module a crate exports to anybody.
//!
//! ```
//! assert_eq!(majordomus_cli::two(), 2);
//! ```

/// Answers with two, every time, which is more than the signature promises on its own.
///
/// ```
/// assert_eq!(majordomus_cli::two(), 2);
/// ```
pub fn two() -> usize { 2 }

#[cfg(test)]
mod tests {
    #[test]
    fn it_is_two() { assert_eq!(super::two(), 2); }
}
RS
expect_exit 0 sh -c "cd '$CLEAN' && '$RB' quality report --format json > '$S/clean.json' 2>/dev/null" || exit 1
[ "$(jq -r .measured "$S/clean.json")" = true ] || { echo "    the crate was not measured"; exit 1; }
[ "$(jq -r '.report.violations | length' "$S/clean.json")" = 0 ] \
  || { echo "    a clean crate reported findings:"; jq -r '.report.violations[].code' "$S/clean.json"; exit 1; }
# the counts are derived, not asserted: the one item that owes an example has one
[ "$(jq -r .report.public_api.owe_example "$S/clean.json")" = "$(jq -r .report.public_api.exampled "$S/clean.json")" ] \
  || { echo "    owe_example and exampled disagree on a clean crate"; exit 1; }

# ---------------------------------------------------------------- one way of failing, one code
# body of lib.rs, the code it must report, and nothing else
check_one() {
  name="$1"; want="$2"
  r="$(make_repo "$name")"
  make_crate "$r"
  ( cd "$r" && "$RB" quality report --format json > "$S/$name.json" 2>/dev/null )
  got="$(jq -r '[.report.violations[].code] | unique | join(",")' "$S/$name.json")"
  [ "$got" = "$want" ] || { echo "    $name: expected $want, got '$got'"; return 1; }
  # and the finding explains itself
  jq -e '.report.violations[0] | (.rule | startswith("project.")) and (.why | length > 20)
         and (.remediation | length > 20) and (.symbol | length > 0)' "$S/$name.json" >/dev/null \
    || { echo "    $name: a finding does not name its rule, its reason or its remedy"; return 1; }
  # the JSON and the exit code agree
  code=0; ( cd "$r" && "$RB" quality report >/dev/null 2>&1 ) || code=$?
  [ "$code" = 10 ] || { echo "    $name: findings stand and the command exited $code"; return 1; }
  [ "$(jq -r .passes "$S/$name.json")" = false ] || { echo "    $name: findings stand and passes is true"; return 1; }
}

HEADER='//! A crate that does one thing, and this header says at some length what that one thing is,
//! which is what the module policy asks of every module a crate exports to anybody.
//!
//! ```
//! assert_eq!(majordomus_cli::two(), 2);
//! ```
'
TESTED='
#[cfg(test)]
mod tests { #[test] fn t() { assert_eq!(super::two(), 2); } }
'

check_one no_example RUST_PUBLIC_MISSING_EXAMPLE <<RS || exit 1
$HEADER
/// Answers with two, every time, which is more than the signature promises on its own.
pub fn two() -> usize { 2 }
$TESTED
RS

check_one ignored_example RUST_EXAMPLE_NOT_EXECUTABLE <<RS || exit 1
$HEADER
/// Answers with two, every time, which is more than the signature promises on its own.
///
/// \`\`\`ignore
/// two();
/// \`\`\`
pub fn two() -> usize { 2 }
$TESTED
RS

check_one placeholder_example RUST_EXAMPLE_PLACEHOLDER <<RS || exit 1
$HEADER
/// Answers with two, every time, which is more than the signature promises on its own.
///
/// \`\`\`
/// let _ = majordomus_cli::two;
/// assert!(true);
/// \`\`\`
pub fn two() -> usize { 2 }
$TESTED
RS

check_one untested_module RUST_MODULE_MISSING_BEHAVIOURAL_TEST <<RS || exit 1
$HEADER
/// Answers with two, every time, which is more than the signature promises on its own.
///
/// \`\`\`
/// assert_eq!(majordomus_cli::two(), 2);
/// \`\`\`
pub fn two() -> usize { 2 }
RS

# ---------------------------------------------------------------- an unknown code is refused
RATCHET="$(make_repo ratchet)"
make_crate "$RATCHET" <<RS
$HEADER
/// Answers with two, every time, which is more than the signature promises on its own.
pub fn two() -> usize { 2 }
$TESTED
RS
code=0; ( cd "$RATCHET" && "$RB" quality report --code RUST_NOPE >/dev/null 2>&1 ) || code=$?
[ "$code" = 2 ] || { echo "    an unknown code exited $code; the caller's mistake is a usage error"; exit 1; }

# ---------------------------------------------------------------- the ratchet, both directions
code=0; ( cd "$RATCHET" && "$RB" quality report >/dev/null 2>&1 ) || code=$?
[ "$code" = 10 ] || { echo "    the debt does not fail the gate before it is recorded"; exit 1; }

( cd "$RATCHET" && "$RB" quality report --write-baseline >/dev/null 2>&1 ) \
  || { echo "    the baseline could not be written"; exit 1; }
[ -f "$RATCHET/.ai/repo/rust-quality-baseline.txt" ] || { echo "    no baseline was written"; exit 1; }
grep -q '^RUST_PUBLIC_MISSING_EXAMPLE' "$RATCHET/.ai/repo/rust-quality-baseline.txt" \
  || { echo "    the baseline does not record the finding it accepted"; exit 1; }

expect_exit 0 sh -c "cd '$RATCHET' && '$RB' quality report >/dev/null 2>&1" \
  || { echo "    a recorded finding still fails the gate"; exit 1; }

# a second, unrecorded finding fails while the first is still accepted
cat >> "$RATCHET/apps/majordomus-cli/src/lib.rs" <<'RS'

/// Answers with three, every time, which is more than the signature promises on its own.
pub fn three() -> usize { 3 }
RS
code=0; ( cd "$RATCHET" && "$RB" quality report > "$S/ratchet.txt" 2>/dev/null ) || code=$?
[ "$code" = 10 ] || { echo "    new debt did not fail the gate"; exit 1; }
grep -q '::three' "$S/ratchet.txt" || { echo "    the new finding was not the one reported"; exit 1; }
if grep -q '::two' "$S/ratchet.txt"; then echo "    the recorded finding was reported as new"; exit 1; fi

echo "    quality: no crate answered, clean crate silent, four codes each reported alone, ratchet holds both ways"

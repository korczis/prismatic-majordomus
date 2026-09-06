# The doctrine that holds a command's evidence beside its declaration, exercised against a
# crate this case builds rather than against the one in the checkout.
#
# The subject is text, not Rust: the validator reads the composition and the module files
# and never compiles anything, so the fixture is a handful of files and the case runs in
# milliseconds. That is deliberate — a doctrine dispatched from `doctor` runs on every
# commit through the pre-commit hook, and one that needed cargo would be a doctrine nobody
# could afford to leave enabled.
#
# What is proved here: the denominator comes from `compose_modules!` and follows a change to
# it; either accepted form of assertion — an in-file #[test] or a doc example cargo runs —
# satisfies the rule on its own; a module with neither is reported exactly once, because
# demanding a particular form would buy a token test beside a real example; a module declared
# and composed by nobody is reported; a composed module with no file, and one that declares no
# command at all, are reported; the coverage floor must be declared and must be high; and a repository with no crate is skipped rather than failed,
# because the layer installs where there is no executable.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null

# The rule is this repository's, not the skeleton's, so `init` does not install it. Copying
# it into the fixture is what puts the whole chain under test — the front matter, the
# doctrine registry built from the effective set, the dispatch from doctor, and the
# validator — rather than the validator function alone.
mkdir -p .ai/repo/rules/project test/cases
# the whole project set, because the rule declares dependencies on other project rules and
# a set that does not resolve is not applied at all — which is the loader being fail-closed
cp "$ROOT"/.ai/repo/rules/project/*.v1.md .ai/repo/rules/project/
# the wiring check requires the case the rule names to exist in the repository it inspects
: > test/cases/88_rust_command_tested.sh

B=apps/majordomus-cli/src/capability/builtin
doctor_out() { "$MJ" doctor > doctor.txt 2>&1 || true; }

# a module that satisfies the rule: declared, composed, tested here, documented by example
good_module() {
  cat > "$B/$1.rs" <<'RS'
//! A command module.
//!
//! ```
//! assert_eq!(1 + 1, 2);
//! ```
module!(demo);
capability!(demo.thing);

#[cfg(test)]
mod tests {
    #[test]
    fn the_command_answers() {
        assert!(true);
    }
}
RS
}
# a module whose only assertion is a doc example — cargo runs it, so the rule is satisfied
# and no #[test] is owed. This is the form the rule must not quietly demand twice.
doc_only_module() {
  cat > "$B/$1.rs" <<'RS'
//! A command module documented by an example that runs.
//!
//! ```
//! assert_eq!("web.surfaces".split('.').next(), Some("web"));
//! ```
module!(docs_only);
capability!(docs_only.thing);
RS
}
# a module with no evidence of its own
bare_module() { printf 'module!(%s);\ncapability!(%s.thing);\n' "$1" "$1" > "$B/$1.rs"; }

compose() {   # compose <name>...
  { printf 'pub fn modules() -> Vec<ModuleDescriptor> {\n    compose_modules![\n'
    for m in "$@"; do printf '        %s,\n' "$m"; done
    printf '    ]\n}\n'
  } > "$B/mod.rs"
}

# --- a repository with no crate is skipped, and says which subject it could not find
doctor_out
grep -q 'rust-command.*carries no Rust capability modules' doctor.txt \
  || { echo "    a repository with no crate was not skipped"; grep -i rust-command doctor.txt; exit 1; }
grep -qE '^(FAIL|WARN) *rust-command' doctor.txt \
  && { echo "    a repository with no crate reported a finding"; grep -i rust-command doctor.txt; exit 1; }

# --- one good module, one bare, one declared but composed by nobody
mkdir -p "$B" scripts
printf '90\n' > scripts/rust-coverage-threshold
good_module tested
doc_only_module documented
bare_module bare
bare_module orphan
compose tested documented bare
doctor_out

grep -q "rust-command bare .*no assertion that runs" doctor.txt \
  || { echo "    a module asserting nothing was not reported"; grep -i rust-command doctor.txt; exit 1; }
# one finding, not one per missing form: demanding a particular form buys ceremony
[ "$(grep -c 'rust-command bare ' doctor.txt)" = 1 ] \
  || { echo "    a module asserting nothing produced more than one finding"; grep 'rust-command bare ' doctor.txt; exit 1; }
grep -q "rust-command orphan .*does not compose" doctor.txt \
  || { echo "    a module composed by nobody was not reported"; grep -i rust-command doctor.txt; exit 1; }
# the ones that satisfy the rule are silent: their evidence is where the rule wants it, and
# a doc example alone is enough — cargo runs it, so it is an assertion, not prose
grep -q 'rust-command tested' doctor.txt \
  && { echo "    a module with its own test was reported anyway"; grep -i rust-command doctor.txt; exit 1; }
grep -q 'rust-command documented' doctor.txt \
  && { echo "    a module asserting only through a doc example was reported; either form must count"; grep -i rust-command doctor.txt; exit 1; }

# --- the denominator is the composition, not a list in the validator
# composing the orphan makes it measured rather than orphaned, with no edit anywhere else
compose tested documented bare orphan
doctor_out
grep -q "rust-command orphan .*does not compose" doctor.txt \
  && { echo "    a module was still called uncomposed after being composed"; exit 1; }
grep -q "rust-command orphan .*no assertion that runs" doctor.txt \
  || { echo "    composing a module did not bring it into the measurement"; grep -i rust-command doctor.txt; exit 1; }
# and removing it from the composition takes it back out of the measurement
compose tested documented bare
doctor_out
grep -q "rust-command orphan .*no assertion that runs" doctor.txt \
  && { echo "    an uncomposed module was still measured as a command"; exit 1; }

# --- a module the composition names and the tree does not carry
compose tested documented bare missing
doctor_out
grep -q 'rust-command missing .*has no module file' doctor.txt \
  || { echo "    a composed module with no file was not reported"; grep -i rust-command doctor.txt; exit 1; }
compose tested documented bare

# --- a composed module that declares no command is not a command module
printf 'module!(empty);\n' > "$B/empty.rs"
compose tested documented bare empty
doctor_out
grep -q 'rust-command empty .*declares no capability' doctor.txt \
  || { echo "    a composed module declaring nothing was not reported"; grep -i rust-command doctor.txt; exit 1; }
rm -f "$B/empty.rs"; compose tested documented bare

# --- the coverage floor is declared, not assumed
rm -f scripts/rust-coverage-threshold
doctor_out
grep -q 'rust-command coverage .*no coverage floor is declared' doctor.txt \
  || { echo "    a missing coverage floor was not reported"; grep -i rust-command doctor.txt; exit 1; }

# --- and it is high: lowering the bar is a visible act, not a quiet edit
printf '60\n' > scripts/rust-coverage-threshold
doctor_out
grep -q 'rust-command coverage .*is 60%, below the 90%' doctor.txt \
  || { echo "    a floor below the rule's minimum was not reported"; grep -i rust-command doctor.txt; exit 1; }
printf '90\n' > scripts/rust-coverage-threshold
doctor_out
grep -q 'rust-command coverage' doctor.txt \
  && { echo "    a floor at the rule's minimum was still reported"; grep -i rust-command doctor.txt; exit 1; }

# --- a tree where every composed module satisfies the rule reports it once, and passes
good_module bare
rm -f "$B/orphan.rs"          # the uncomposed module was the subject above, not of this one
doctor_out
grep -qE '^OK *rust-command' doctor.txt \
  || { echo "    a compliant tree did not report OK"; grep -i rust-command doctor.txt; exit 1; }
grep -qE '^(FAIL|WARN) *rust-command' doctor.txt \
  && { echo "    a compliant tree still reported a finding"; grep -i rust-command doctor.txt; exit 1; }

# --- the rule is advisory at v1: a violation is a warning, never a blocking failure.
# The claim is about this doctrine's own level, not about doctor's exit code: a fixture this
# small fails other doctrines for reasons of its own, and asserting the exit code here would
# be asserting their state rather than this rule's.
bare_module bare
doctor_out
grep -qE '^WARN *rust-command' doctor.txt \
  || { echo "    an advisory violation was not reported as a warning"; grep -i rust-command doctor.txt; exit 1; }
grep -qE '^FAIL *rust-command' doctor.txt \
  && { echo "    an advisory violation was reported as a blocking failure; v1 must not block"; exit 1; }

# --- the rule resolves in this repository and names this case
R="$ROOT/.ai/repo/rules/project/rust-command-tested-in-file.v1.md"
[ -f "$R" ] || { echo "    the rule is missing: $R"; exit 1; }
grep -q '^  validator: rust_command_tested$' "$R" || { echo "    the rule does not name its validator"; exit 1; }
grep -q 'test/cases/88_rust_command_tested.sh' "$R" || { echo "    the rule does not name this case"; exit 1; }
"$MJ" --repo "$ROOT" rules list | grep -q 'project.rust-command-tested-in-file' \
  || { echo "    the rule does not resolve in this repository's effective set"; exit 1; }

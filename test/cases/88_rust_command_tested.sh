# The gate that holds a command's evidence beside its declaration, exercised against a crate
# this case builds rather than against the one in the checkout.
#
# The subject is text, not Rust: the check reads the composition and the module files and
# never compiles anything, so the fixture is a handful of files and the case runs fast.
#
# What is proved here: the denominator comes from `compose_modules!` and follows a change to
# it; either accepted form of assertion — an in-file #[test] or a doc example cargo runs —
# satisfies the rule on its own; a module with neither is reported once; a module composed by
# nobody, a composed module with no file, and a composed module declaring no command are each
# reported; the coverage floor must be declared, one integer, and not under the bound the
# check declares (it may rise and never fall); and the module ratchet works in
# both directions — debt already recorded in the baseline does not fail the gate, debt that is
# not recorded does, and clearing debt is noticed.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/ci/rust-command-check"
[ -x "$CHECK" ] || { echo "    scripts/ci/rust-command-check is missing or not executable"; exit 1; }

F="$PWD/fixture"; B="$F/apps/majordomus-cli/src/capability/builtin"
mkdir -p "$B" "$F/scripts" "$F/.ai/repo" "$PWD/empty"
# the floor ratchets from a bound the check itself declares, once; the case reads it there
BOUND="$(sed -n 's/^FLOOR_BOUND=\([0-9][0-9]*\)$/\1/p' "$CHECK")"
case "$BOUND" in ''|*[!0-9]*) echo "    the check declares no integer FLOOR_BOUND: '$BOUND'"; exit 1 ;; esac
printf '%s\n' "$BOUND" > "$F/scripts/rust-coverage-threshold"
run() { MJ_ROOT="$F" "$CHECK" > out.txt 2>&1; echo $?; }

good_module() {
  cat > "$B/$1.rs" <<'RS'
module!(demo);
capability!(demo.thing);

#[cfg(test)]
mod tests {
    #[test]
    fn the_command_answers() { assert!(true); }
}
RS
}
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
bare_module() { printf 'module!(%s);\ncapability!(%s.thing);\n' "$1" "$1" > "$B/$1.rs"; }
compose() { { printf 'pub fn modules() -> Vec<ModuleDescriptor> {\n    compose_modules![\n'
    for m in "$@"; do printf '        %s,\n' "$m"; done
    printf '    ]\n}\n'; } > "$B/mod.rs"; }

# --- a tree with no crate is passed over rather than failed
rc="$(MJ_ROOT="$PWD/empty" "$CHECK" > out.txt 2>&1; echo $?)"
[ "$rc" = 0 ] || { echo "    a tree with no crate did not pass ($rc)"; cat out.txt; exit 1; }
grep -q 'nothing to measure' out.txt || { echo "    a tree with no crate did not say so"; cat out.txt; exit 1; }

# --- one module per accepted form, one with neither, one composed by nobody
good_module tested
doc_only_module documented
bare_module bare
bare_module orphan
compose tested documented bare
rc="$(run)"
grep -q 'rust-command bare .*no assertion that runs' out.txt \
  || { echo "    a module asserting nothing was not reported"; cat out.txt; exit 1; }
[ "$(grep -c 'rust-command bare ' out.txt)" = 1 ] \
  || { echo "    a module asserting nothing produced more than one finding"; cat out.txt; exit 1; }
grep -q 'rust-command orphan .*composes nowhere' out.txt \
  || { echo "    a module composed by nobody was not reported"; cat out.txt; exit 1; }
grep -q 'rust-command tested ' out.txt \
  && { echo "    a module with its own test was reported anyway"; cat out.txt; exit 1; }
grep -q 'rust-command documented ' out.txt \
  && { echo "    a module asserting only through a doc example was reported; either form must count"; cat out.txt; exit 1; }
[ "$rc" = 10 ] || { echo "    unrecorded debt did not fail the gate ($rc)"; cat out.txt; exit 1; }

# --- the ratchet: recorded debt does not fail, unrecorded debt does
MJ_ROOT="$F" "$CHECK" --write-baseline > out.txt 2>&1 || { echo "    --write-baseline failed"; cat out.txt; exit 1; }
grep -q '^bare$' "$F/.ai/repo/rust-command-baseline.txt" \
  || { echo "    the baseline does not record today's debt"; cat "$F/.ai/repo/rust-command-baseline.txt"; exit 1; }
rc="$(run)"
[ "$rc" = 0 ] || { echo "    recorded debt failed the gate ($rc); the ratchet must not block on what it recorded"; cat out.txt; exit 1; }

bare_module newcmd
compose tested documented bare newcmd
rc="$(run)"
[ "$rc" = 10 ] || { echo "    a new command with no assertion did not fail the gate ($rc)"; cat out.txt; exit 1; }
grep -q 'new debt' out.txt || { echo "    the failure does not say the debt is new"; cat out.txt; exit 1; }
grep -q 'newcmd' out.txt || { echo "    the failure does not name the new module"; cat out.txt; exit 1; }

# --- clearing debt is noticed rather than silently accepted
good_module newcmd
good_module bare
rc="$(run)"
[ "$rc" = 0 ] || { echo "    a tree with no outstanding debt failed the gate ($rc)"; cat out.txt; exit 1; }
grep -q 'debt cleared' out.txt || { echo "    clearing debt was not reported"; cat out.txt; exit 1; }

# --- the denominator is the composition, not a list in the script
bare_module orphan
compose tested documented bare newcmd orphan
rc="$(run)"
grep -q 'rust-command orphan .*composes nowhere' out.txt \
  && { echo "    a composed module was still called uncomposed"; cat out.txt; exit 1; }
grep -q 'rust-command orphan .*no assertion that runs' out.txt \
  || { echo "    composing a module did not bring it into the measurement"; cat out.txt; exit 1; }
compose tested documented bare newcmd

# --- a composed module the tree does not carry, and one that declares no command
compose tested documented bare newcmd missing
rc="$(run)"
grep -q 'rust-command missing .*no module file' out.txt \
  || { echo "    a composed module with no file was not reported"; cat out.txt; exit 1; }
printf 'module!(empty);\n' > "$B/empty.rs"
compose tested documented bare newcmd empty
rc="$(run)"
grep -q 'rust-command empty .*declares no capability' out.txt \
  || { echo "    a composed module declaring no command was not reported"; cat out.txt; exit 1; }
rm -f "$B/empty.rs"; compose tested documented bare newcmd

# --- the coverage floor is declared, one integer, and never under the bound it ratchets from
rm -f "$F/scripts/rust-coverage-threshold"
rc="$(run)"
grep -q 'no coverage floor is declared' out.txt \
  || { echo "    a missing coverage floor was not reported"; cat out.txt; exit 1; }
printf '%s.5\n' "$BOUND" > "$F/scripts/rust-coverage-threshold"
rc="$(run)"
grep -q 'no coverage floor is declared' out.txt \
  || { echo "    a floor that is not one integer was not reported"; cat out.txt; exit 1; }
below=$((BOUND - 1))
printf '%s\n' "$below" > "$F/scripts/rust-coverage-threshold"
rc="$(run)"
grep -q "floor is ${below}%, below the ${BOUND}% bound" out.txt \
  || { echo "    a floor one under the ratchet's bound was not reported"; cat out.txt; exit 1; }
[ "$rc" = 10 ] || { echo "    a floor under the bound did not fail the gate ($rc)"; cat out.txt; exit 1; }
printf '%s\n' "$BOUND" > "$F/scripts/rust-coverage-threshold"
rc="$(run)"
grep -q 'coverage floor' out.txt \
  && { echo "    a floor at the bound was still reported"; cat out.txt; exit 1; }
printf '%s\n' "$((BOUND + 1))" > "$F/scripts/rust-coverage-threshold"
rc="$(run)"
grep -q 'coverage floor' out.txt \
  && { echo "    a floor raised above the bound was reported; a ratchet may rise"; cat out.txt; exit 1; }

# --- the rule exists, is blocking, and names the gate that holds it
R="$ROOT/.ai/repo/rules/project/rust-command-tested-in-file.v1.md"
[ -f "$R" ] || { echo "    the rule is missing: $R"; exit 1; }
grep -q '^class: blocking$' "$R" || { echo "    the rule is not blocking"; exit 1; }
grep -q 'scripts/ci/rust-command-check' "$R" || { echo "    the rule does not name the check that holds it"; exit 1; }
grep -q 'rust-command' "$ROOT/.ai/repo/ci/gates.yaml" || { echo "    no gate runs the check"; exit 1; }
"$MJ" --repo "$ROOT" rules list | grep -q 'project.rust-command-tested-in-file' \
  || { echo "    the rule does not resolve in this repository's effective set"; exit 1; }

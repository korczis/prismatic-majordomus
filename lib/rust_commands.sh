# rust_commands.sh — the validator for project.rust-command-tested-in-file.
# shellcheck disable=SC2034  # MJ_DOCTRINE_SKIPPED is read by the dispatcher in doctrine.sh
#
# The subject is the executable's command modules: the capability modules that
# `compose_modules!` composes into the application. The denominator is read from that
# composition rather than kept here, so a module added tomorrow is measured tomorrow and
# this file never carries a list of what exists. That is the same property the rule asks of
# the commands themselves.
#
# What it measures, per composed module:
#   - at least one assertion that runs, in the module that declares the command rather than
#     in a suite somewhere else: an in-file `#[test]`, or a doc example, which
#     `cargo test --doc` executes. Either form counts; the form is the author's choice and
#     demanding a particular one only buys a token test beside a real example.
#   - the module file exists at all.
# And across the tree: a module that declares itself with `module!` and is composed by
# nobody, which is a command that exists and is served to no one; and the coverage floor the
# crate is held to, which must be declared rather than assumed.
#
# Skipped, not failed, where there is no Rust crate: the layer installs into repositories
# that carry no executable, and a doctrine that cannot apply is not a violation.

# What this rule calls high coverage. Stated once, here, because the rule's prose and the
# check must not be able to disagree about it.
MJ_RUSTCMD_MIN_FLOOR=90

# The composed command modules, one per line, from the composition itself.
mj_rustcmd_modules() {
  awk '
    /compose_modules!\[/ { f = 1; next }
    f && /\]/            { exit }
    f                    { gsub(/[ \t,]/, ""); if ($0 != "" && $0 !~ /^\/\//) print }
  ' "$1"
}

# Every module file that declares itself a module, composed or not.
mj_rustcmd_declared() {
  grep -lE '^[[:space:]]*module!' "$1"/*.rs 2>/dev/null | while read -r f; do
    basename "$f" .rs
  done
}

mj_validate_rust_command_tested() {
  local dir="$MJ_ROOT/apps/majordomus-cli/src/capability/builtin"
  local mod_rs="$dir/mod.rs" bad=0 m f tests docs composed declared
  if [ ! -f "$mod_rs" ]; then
    MJ_DOCTRINE_SKIPPED=1
    mj_doctrine_skip rust-command modules \
      "this repository carries no Rust capability modules" \
      "ls apps/majordomus-cli/src/capability/builtin"
    return 0
  fi
  composed="$(mj_rustcmd_modules "$mod_rs")"
  if [ -z "$composed" ]; then
    MJ_DOCTRINE_SKIPPED=1
    mj_doctrine_skip rust-command modules \
      "the composition names no module; nothing to measure" \
      "grep -n -A12 'compose_modules!' apps/majordomus-cli/src/capability/builtin/mod.rs"
    return 0
  fi

  for m in $composed; do
    f="$dir/$m.rs"
    if [ ! -f "$f" ]; then
      bad=$((bad + 1))
      mj_doctrine_fail rust-command "$m" \
        "is composed into the application but has no module file" \
        "ls apps/majordomus-cli/src/capability/builtin/$m.rs"
      continue
    fi
    # grep -c prints its count and exits 1 when the count is zero, so the count is taken
    # from the output and the status ignored; `|| echo 0` would append a second line and
    # make every comparison an error that silently passes.
    tests="$(grep -c '#\[test\]' "$f" 2>/dev/null || true)"
    # both doc forms are executed by `cargo test --doc`: `//!` on the module and `///`
    # on an item, so both count as the module documenting itself by example
    docs="$(grep -cE '^[[:space:]]*//[/!] *```' "$f" 2>/dev/null || true)"
    # A composed module that declares no capability is not a command module. The registry
    # enforces that a capability sits in its own module's namespace (project rule
    # capability-modules, case 91); what it cannot see is a module composed into the
    # application that declares nothing at all.
    if ! grep -qE '^[[:space:]]*capability!' "$f" 2>/dev/null; then
      bad=$((bad + 1))
      mj_doctrine_fail rust-command "$m" \
        "is composed into the application and declares no capability; it is not a command module" \
        "grep -n 'capability!' apps/majordomus-cli/src/capability/builtin/$m.rs"
    fi
    # One finding, not two. What the rule wants is evidence beside the declaration that
    # the declaration is what it claims; requiring a particular *form* of it only invites a
    # token #[test] beside a doc example that already asserts the same thing. A doc example
    # is executed by `cargo test --doc`, so either form is an assertion that runs.
    if [ "$tests" -eq 0 ] && [ "$docs" -eq 0 ]; then
      bad=$((bad + 1))
      mj_doctrine_fail rust-command "$m" \
        "declares a command and carries no assertion that runs: no in-file #[test] and no doc example" \
        "grep -n 'capability!' apps/majordomus-cli/src/capability/builtin/$m.rs"
    fi
  done

  # a command nobody composed: it exists, it is declared, and it is served to no one
  declared="$(mj_rustcmd_declared "$dir")"
  for m in $declared; do
    case " $(echo "$composed" | tr '\n' ' ') " in
      *" $m "*) : ;;
      *)
        bad=$((bad + 1))
        mj_doctrine_fail rust-command "$m" \
          "declares a module that the root composition does not compose; it is registered nowhere and reachable by nothing" \
          "grep -n -A12 'compose_modules!' apps/majordomus-cli/src/capability/builtin/mod.rs" ;;
    esac
  done

  # the floor the crate is held to is declared, not assumed
  # "High coverage" is a number or it is an opinion. The floor must exist, be a number, and
  # be at least MJ_RUSTCMD_MIN_FLOOR — so that lowering the bar is a visible act rather than
  # a quiet edit that leaves every other check still passing.
  local floor="$MJ_ROOT/scripts/rust-coverage-threshold" value=""
  if [ ! -f "$floor" ] || ! grep -qE '^[0-9]+$' "$floor" 2>/dev/null; then
    bad=$((bad + 1))
    mj_doctrine_fail rust-command coverage \
      "no coverage floor is declared, so 'high coverage' is an opinion rather than a gate" \
      "cat scripts/rust-coverage-threshold"
  else
    value="$(grep -m1 -oE '^[0-9]+$' "$floor")"
    if [ "$value" -lt "$MJ_RUSTCMD_MIN_FLOOR" ]; then
      bad=$((bad + 1))
      mj_doctrine_fail rust-command coverage \
        "the coverage floor is ${value}%, below the ${MJ_RUSTCMD_MIN_FLOOR}% this rule calls high" \
        "cat scripts/rust-coverage-threshold"
    fi
  fi

  [ "$bad" = 0 ] && mj_ok rust-command "-" \
    "$(echo "$composed" | wc -w | tr -d ' ') command module(s) — each tested and documented in the file that declares it, each composed, floor $(cat "$floor" 2>/dev/null)%"
  return 0
}

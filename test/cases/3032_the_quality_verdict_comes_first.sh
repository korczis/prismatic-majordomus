# majordomus-covers: none
# The local Rust check gives the rust-quality verdict first, with the structure job's finding
# (I2715).
#
# On 2026-10-14 two implementations passed their crate tests, clippy, rustdoc and cases, and
# then failed the structure job's rust-quality gate on exported items with no example. The
# only local run of the report stood at the end of scripts/rust-check, after `cargo test`.
# Now a person's run (the default mode) asks it before every other gate and stops on its
# verdict, `--quality` asks it alone, and CI's modes still ask it after the registry checks.
#
# How it is proved, and why not with a built fixture crate: scripts/rust-check reaches the
# report through `cargo run`, and building a crate inside a case costs minutes. So `cargo` is
# a stand-in on PATH that logs every call and answers `cargo run -- <args>` by running the
# already-built executable with those arguments (or with a canned answer). The judge is the
# real executable, over a fixture repository whose crate this case writes, so the finding is
# the one the structure job prints; what the stand-in replaces is only the build.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MODEL="$ROOT/.ai/repo/ci/gates.yaml"

# ---------------------------------------------------------------- the fixture
F="$T/repo"; mkdir -p "$F/scripts" "$F/lib" "$F/apps/majordomus-cli/src"
cp "$ROOT/scripts/rust-check" "$F/scripts/rust-check"
cp "$ROOT/lib/rust_bin.sh" "$ROOT/lib/sha256.sh" "$F/lib/"
cp -R "$ROOT/share/skeleton/ai" "$F/.ai"
printf '[package]\nname = "x"\n' > "$F/apps/majordomus-cli/Cargo.toml"
cat > "$F/apps/majordomus-cli/src/lib.rs" <<'RS'
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
( cd "$F" && git init -q . && git add -A >/dev/null 2>&1 \
  && git -c user.email=t@t -c user.name=t commit -qm init >/dev/null 2>&1 ) \
  || { echo "    could not make the fixture repository"; exit 1; }

STUB="$T/stub"; mkdir -p "$STUB"
cat > "$STUB/cargo" <<'SH'
#!/bin/sh
printf '%s\n' "$*" >> "$STUB_LOG"
case " $* " in *" -- quality report"*) ;; *) exit 0 ;; esac
while [ "$1" != -- ]; do shift; done; shift
[ -z "${STUB_EXE:-}" ] || exec "$STUB_EXE" "$@"
echo "quality: canned answer"
exit "${STUB_QUALITY_EXIT:-0}"
SH
chmod +x "$STUB/cargo"
STUB_LOG="$T/cargo.log"; export STUB_LOG
check() { # <exit> <rust-check args...>: one run, its cargo calls in $STUB_LOG
  local want="$1"; shift
  : > "$STUB_LOG"
  PATH="$STUB:$PATH" MAJORDOMUS_BUILD_COMMIT=fixture expect_exit "$want" "$F/scripts/rust-check" "$@"
}

# ---------------------------------------------------------------- the gate's own arguments
# what rust-check hands the executable is what the rust-quality gate hands it, read from the
# model, so the finding text is the gate's
gate_args="$(awk '/^  - id: rust-quality$/{f=1} f && /^    runs: /{sub(/.* -- /, ""); print; exit}' "$MODEL")"
[ -n "$gate_args" ] || { echo "    the model declares no rust-quality gate with a 'cargo run ... --' command"; exit 1; }
STUB_EXE="" STUB_QUALITY_EXIT=0 check 0 --quality
[ "$(cat "$STUB_LOG")" = "run -- $gate_args" ] \
  || { echo "    --quality asked cargo for '$(tr '\n' '|' < "$STUB_LOG")', the gate runs '$gate_args'"; exit 1; }
expect_grep 'rust-check: the quality report accepts every exported item'

# ---------------------------------------------------------------- the orders
# a person's run asks the report first, and a red report stops it before format, lints, tests
STUB_EXE="" STUB_QUALITY_EXIT=10 check 10
[ "$(cat "$STUB_LOG")" = "run -- $gate_args" ] \
  || { echo "    the default mode asked cargo, in order: $(tr '\n' '|' < "$STUB_LOG")"; exit 1; }
STUB_EXE="" STUB_QUALITY_EXIT=10 check 10 --quality
# CI's --integration asks it once, after the registry checks, so a red report hides none of them
STUB_EXE="" STUB_QUALITY_EXIT=0 check 0 --integration
want_order="build
run -- capabilities validate
run -- generate --check
run -- bench coverage --check
run -- web validate
run -- $gate_args"
[ "$(cat "$STUB_LOG")" = "$want_order" ] \
  || { printf '    --integration asked cargo, in order:\n%s\n    want:\n%s\n' "$(cat "$STUB_LOG")" "$want_order"; exit 1; }
# --quality hands over nothing
check 2 --quality --artifact dist
expect_grep '--artifact belongs to a run of every gate'

# ---------------------------------------------------------------- the real verdict
# the executable judges the fixture crate: clean, then an exported function without an example
STUB_EXE="$RB"; export STUB_EXE
check 0 --quality
expect_grep 'quality: no findings'
printf '\n/// Answers with three, every time, which is more than the signature promises on its own.\npub fn three() -> usize { 3 }\n' \
  >> "$F/apps/majordomus-cli/src/lib.rs"
check 10 --quality
expect_grep 'RUST_PUBLIC_MISSING_EXAMPLE majordomus_cli::three'
expect_grep 'remedy +add a ``` block that calls the item'
# the default mode refuses it with the same finding, before anything else runs
check 10
expect_grep 'RUST_PUBLIC_MISSING_EXAMPLE majordomus_cli::three'
[ "$(cat "$STUB_LOG")" = "run -- $gate_args" ] \
  || { echo "    the default mode went on past a red quality report: $(tr '\n' '|' < "$STUB_LOG")"; exit 1; }
# the same tree under the rust-quality gate's own command line refuses it with the same line
( cd "$F" && "$RB" quality report > "$T/gate.out" 2>/dev/null ) && { echo "    the executable passed the item the check refused"; exit 1; }
grep -q 'RUST_PUBLIC_MISSING_EXAMPLE majordomus_cli::three' "$T/gate.out" \
  || { echo "    the executable's own report does not name the finding the check printed"; exit 1; }

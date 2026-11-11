# A push that changes the crate owes the formatter and version-surface (I4570).
#
# On 2026-11-09 #2983 and #2995 passed the before-push plan and were then refused by CI's rust
# job: both on `cargo fmt --check`, and #2995 again on version-surface. This case runs the real
# scripts/ci/run-plan --before-push in a fixture whose plan selects the crate's gates, and
# proves:
#
#   a plan that selects rust-check, an unformatted file   FAILED rust-fmt, the fix is named,
#                                                         exit 1
#   the same file formatted                               ok rust-fmt, exit 0
#   a plan that selects version-surface, the gate fails   FAILED version-surface, the fix is
#                                                         named `majordomus release bump`
#   a plan that selects neither (no crate change)         neither runs
#
# The formatter is real (`cargo fmt --check` on a one-file fixture crate, which builds
# nothing); version-surface is a stub with a chosen exit code. Skips without cargo/rustfmt.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"
command -v cargo >/dev/null 2>&1 || skip "no cargo to run the formatter"
cargo fmt --version >/dev/null 2>&1 || skip "no rustfmt to run the formatter"

fail() { printf '    %s\n' "$*"; exit 1; }

mkdir -p fx/scripts/ci fx/lib fx/test/cases fx/apps/majordomus-cli/src
cp "$ROOT/scripts/ci/run-plan" fx/scripts/ci/run-plan
stub() { printf '#!/bin/sh\n%s\n' "$2" > "fx/$1"; chmod +x "fx/$1"; }
# the plan: rust-check selected when RUST is set, version-surface when SURFACE is set
stub scripts/ci-plan 'g=""
[ -n "${RUST:-}" ] && g="{\"id\":\"rust-check\",\"job\":\"rust\",\"runs\":\"scripts/rust-check --ci\",\"selected\":true,\"reason\":\"t\"}"
[ -n "${SURFACE:-}" ] && g="${g:+$g,}{\"id\":\"version-surface\",\"job\":\"rust\",\"runs\":\"scripts/ci/version-matches-surface\",\"selected\":true,\"reason\":\"t\"}"
printf "{\"schema\":1,\"mode\":\"test\",\"reason\":\"a fixture\",\"selected\":[],\"gates\":[%s]}\n" "$g"'
stub scripts/ci/version-matches-surface 'echo "the contract requires a minor release and the version declares none"; exit "${SURFACE_RC:-0}"'
cat > fx/lib/rust_bin.sh <<'LIB'
mj_rust_same_generation() { return 1; }
mj_rust_resolve() { echo "${2}no executable" >&2; return 12; }
LIB
: > "$T/durations.tsv"; : > "$T/cases.tsv"
export MJ_PUSH_DURATIONS="$T/durations.tsv" MJ_PUSH_CASES="$T/cases.tsv"
unset MAJORDOMUS_BIN
cat > fx/apps/majordomus-cli/Cargo.toml <<'TOML'
[package]
name = "fixture"
version = "0.1.0"
edition = "2021"

[lib]
path = "src/lib.rs"
TOML
printf 'pub fn f( )->u8{1}\n' > fx/apps/majordomus-cli/src/lib.rs
(cd fx && git init -q . && git add -A && git -c user.name=t -c user.email=t@t commit -qm fixture)

plan() { (cd fx && bash scripts/ci/run-plan --before-push) > "$T/out" 2>&1; }

# an unformatted crate file: the formatter runs and refuses, naming the fix
rc=0; RUST=1 plan || rc=$?
[ "$rc" = 1 ] || { cat "$T/out"; fail "an unformatted crate change exited $rc, not 1"; }
grep -q '^  FAILED rust-fmt' "$T/out" || { cat "$T/out"; fail "rust-fmt did not fail"; }
grep -q 'fix: cargo fmt --manifest-path apps/majordomus-cli/Cargo.toml' "$T/out" \
  || { cat "$T/out"; fail "the formatter's fix is not named"; }

# formatted: it holds
(cd fx/apps/majordomus-cli && cargo fmt)
rc=0; RUST=1 plan || rc=$?
[ "$rc" = 0 ] || { cat "$T/out"; fail "a formatted crate exited $rc"; }
grep -q '^  ok     rust-fmt' "$T/out" || { cat "$T/out"; fail "rust-fmt did not run on a crate change"; }

# version-surface fails: refused, naming the bump
rc=0; SURFACE=1 SURFACE_RC=1 plan || rc=$?
[ "$rc" = 1 ] || { cat "$T/out"; fail "a failing version-surface exited $rc, not 1"; }
grep -q '^  FAILED version-surface' "$T/out" || { cat "$T/out"; fail "version-surface did not fail"; }
grep -q 'fix: majordomus release bump' "$T/out" || { cat "$T/out"; fail "the bump is not named"; }
rc=0; SURFACE=1 plan || rc=$?
[ "$rc" = 0 ] && grep -q '^  ok     version-surface' "$T/out" || { cat "$T/out"; fail "a holding version-surface did not pass"; }

# no crate change: neither runs
rc=0; plan || rc=$?
[ "$rc" = 0 ] || { cat "$T/out"; fail "a plan without the crate exited $rc"; }
if grep -qE 'rust-fmt|version-surface' "$T/out"; then cat "$T/out"; fail "a crate gate ran for a change outside the crate"; fi
exit 0

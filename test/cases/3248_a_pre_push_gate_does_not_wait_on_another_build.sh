# claim: derived-data-current
# A pre-push gate that runs the executable does not wait on another worktree's build (I2848),
# and no gate builds at all (ADR 0224, I3957).
#
# The structure gates that ran the executable did it through `cargo run`, which waits for the
# shared build directory's lock while other worktrees compile: on 2026-10-21 economics-claims
# took 2158 s of #1753's 2872 s pre-push, against the 6 s CI records for it. They now ask
# bin/majordomus-cli, which never builds, and `scripts/ci/run-plan` lets them inherit
# MAJORDOMUS_BIN only when the commit that executable was built from is known and this tree's
# crate is byte-for-byte that commit's (lib/rust_bin.sh, mj_rust_same_generation). A first
# version asked `generate --check` instead, which passes an executable of another crate whenever
# its registry matches, so a branch's changed module would have been judged by master's code.
# Amended for ADR 0224: what used to fall back to `cargo run` now falls back to the tree's own
# executable, and with none current the gate fails with exit 12 naming `just build`.
# A copy of run-plan, the launcher and lib/rust_bin.sh runs in a small repository whose crate
# is one file; a stand-in executable and a stand-in cargo on PATH record which one each gate ran:
#
#   1  built from HEAD (MAJORDOMUS_BIN_COMMIT), crate unchanged: the gate runs the executable
#   2  the same commit read from a shared ~/.cache/majordomus-master-bin/<sha>/ path
#   3  a crate file edited, uncommitted: MAJORDOMUS_BIN is not used, naming why, and with no
#      executable of the tree the gate fails at exit 12 naming `just build`
#   4  an untracked file in the crate: the same;  5  no build commit known: the same
#   6  no MAJORDOMUS_BIN at all: the tree's own executable when it is current, a refusal when
#      it is missing — and cargo is never called, in any of the six
. "$ROOT/test/lib.sh"

fail() { printf '    %s\n' "$*"; exit 1; }
mkdir -p repo/scripts/ci repo/bin repo/lib repo/apps/majordomus-cli/src stand-in
cp "$ROOT/scripts/ci/run-plan" repo/scripts/ci/run-plan
cp "$ROOT/bin/majordomus-cli" repo/bin/majordomus-cli
cp "$ROOT/lib/rust_bin.sh" repo/lib/rust_bin.sh
printf '[package]\nname = "fixture"\n' > repo/apps/majordomus-cli/Cargo.toml
printf 'fn main() {}\n' > repo/apps/majordomus-cli/src/main.rs
printf 'target/\n' > repo/.gitignore
git -C repo init -q && git -C repo add -A \
  && git -C repo -c user.email=case@example.invalid -c user.name=case commit -qm crate \
  || fail "could not build the fixture repository"
built="$(git -C repo rev-parse HEAD)"
R="$T/repo/scripts/ci/run-plan"

cat > stand-in/cargo <<'SH'
#!/usr/bin/env bash
printf 'cargo %s\n' "$*" >> "$CALLS"
SH
cat > stand-in/majordomus <<'SH'
#!/usr/bin/env bash
printf 'bin %s\n' "$*" >> "$CALLS"
SH
chmod +x stand-in/cargo stand-in/majordomus
mkdir -p "cache/majordomus-master-bin/$built" && cp stand-in/majordomus "cache/majordomus-master-bin/$built/majordomus"
# the tree's own executable lives where cargo would put it; no installed release stands in
export CALLS="$T/calls" PATH="$T/stand-in:$PATH" CARGO_TARGET_DIR="$T/target" \
  MAJORDOMUS_INSTALLED_BIN="$T/no-installed-release"

printf '{"schema":1,"mode":"test","reason":"a fixture","selected":[],"gates":[{"id":"economics-claims","job":"structure","runs":"bin/majordomus-cli economics check","selected":true,"network":false,"reason":"t"}]}' > "$T/plan.json"
printf 'economics-claims\t6\n' > "$T/durations.tsv"; : > "$T/cases.tsv"
export MJ_PUSH_DURATIONS="$T/durations.tsv" MJ_PUSH_CASES="$T/cases.tsv"

# push [expect-refusal]: run the pre-push dispatcher; a refusal is expected only when asked for
push() {
  : > "$CALLS"; local rc=0
  "$R" --before-push --plan "$T/plan.json" > "$T/out" 2>&1 || rc=$?
  if [ "${1:-}" = refused ]; then [ "$rc" = 1 ] || fail "the push was not refused (exit $rc): $(cat "$T/out")"
  else [ "$rc" = 0 ] || fail "the push failed: $(cat "$T/out")"; fi
  ! grep -q '^cargo ' "$CALLS" || fail "a gate called cargo: $(cat "$CALLS")"
}
borrowed() { grep -q '^bin economics check$' "$CALLS"; }
refused_naming_the_build() {
  ! grep -q '^bin ' "$CALLS" || fail "an executable ran where none of this tree's generation exists: $(cat "$CALLS")"
  grep -q 'FAILED economics-claims' "$T/out" || fail "the gate did not fail: $(cat "$T/out")"
  grep -q 'run: just build' "$T/out" || fail "the refusal does not name the explicit build: $(cat "$T/out")"
}

# ---------------------------------------------------------------- 1. built from HEAD, unchanged
MAJORDOMUS_BIN="$T/stand-in/majordomus" MAJORDOMUS_BIN_COMMIT="$built" push
borrowed || fail "an executable of this exact crate was not used: $(cat "$CALLS")"
grep -q "whose crate this tree carries unchanged; the gates run it" "$T/out" || fail "no line says the executable is used: $(cat "$T/out")"

# ---------------------------------------------------------------- 2. the commit from a shared cache path
MAJORDOMUS_BIN="$T/cache/majordomus-master-bin/$built/majordomus" push
borrowed || fail "the build commit was not read from the cache path: $(cat "$CALLS")"

# ---------------------------------------------------------------- 3. the crate edited
printf 'fn main() { println!("changed"); }\n' > repo/apps/majordomus-cli/src/main.rs
MAJORDOMUS_BIN="$T/stand-in/majordomus" MAJORDOMUS_BIN_COMMIT="$built" push refused
refused_naming_the_build
grep -q "the crate differs from ${built:0:11}" "$T/out" || fail "the reason is not named: $(cat "$T/out")"
git -C repo checkout -q -- apps/majordomus-cli/src/main.rs

# ---------------------------------------------------------------- 4. an untracked file in the crate
printf 'pub fn new() {}\n' > repo/apps/majordomus-cli/src/new.rs
MAJORDOMUS_BIN="$T/stand-in/majordomus" MAJORDOMUS_BIN_COMMIT="$built" push refused
refused_naming_the_build
grep -q 'the crate has untracked files' "$T/out" || fail "the reason is not named: $(cat "$T/out")"
rm repo/apps/majordomus-cli/src/new.rs

# ---------------------------------------------------------------- 5. no build commit known
( unset MAJORDOMUS_BIN_COMMIT; MAJORDOMUS_BIN="$T/stand-in/majordomus" push refused ) || exit 1
refused_naming_the_build
grep -q 'the commit it was built from is unknown' "$T/out" || fail "the reason is not named: $(cat "$T/out")"

# ---------------------------------------------------------------- 6. none named: the tree's own, or a refusal
( unset MAJORDOMUS_BIN MAJORDOMUS_BIN_COMMIT; push refused ) || exit 1
refused_naming_the_build
grep -q 'is not used' "$T/out" && fail "an executable was judged with none named: $(cat "$T/out")"
mkdir -p "$T/target/debug" && cp stand-in/majordomus "$T/target/debug/majordomus"
( unset MAJORDOMUS_BIN MAJORDOMUS_BIN_COMMIT; push ) || exit 1
borrowed || fail "the tree's own current executable did not run the gate: $(cat "$CALLS")"

echo "    a gate runs an executable only of this exact crate or the tree's own, never cargo"

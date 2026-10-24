# claim: derived-data-current
# A pre-push gate that runs the executable does not wait on another worktree's build (I2848).
#
# The structure gates that run the executable do it through `cargo run`, which waits for the
# shared build directory's lock while other worktrees compile: on 2026-10-21 economics-claims
# took 2158 s of #1753's 2872 s pre-push, against the 6 s CI records for it. `scripts/ci/run-plan
# --before-push` now runs MAJORDOMUS_BIN in its place, but only when the commit that executable
# was built from is known and this tree's crate is byte-for-byte that commit's. A first version
# asked `generate --check` instead, which passes an executable of another crate whenever its
# registry matches, so a branch's changed module would have been judged by master's code.
# A copy of run-plan runs in a small repository whose crate is one file; a stand-in executable
# and a stand-in cargo on PATH record which one each gate ran:
#
#   1  built from HEAD (MAJORDOMUS_BIN_COMMIT), crate unchanged: the gate runs the executable
#   2  the same commit read from a shared ~/.cache/majordomus-master-bin/<sha>/ path
#   3  a crate file edited, uncommitted: cargo run, naming why
#   4  an untracked file in the crate: cargo run
#   5  no build commit known: cargo run;  6  no MAJORDOMUS_BIN at all: cargo run, as before
. "$ROOT/test/lib.sh"

fail() { printf '    %s\n' "$*"; exit 1; }
mkdir -p repo/scripts/ci repo/apps/majordomus-cli/src stand-in
cp "$ROOT/scripts/ci/run-plan" repo/scripts/ci/run-plan
printf '[package]\nname = "fixture"\n' > repo/apps/majordomus-cli/Cargo.toml
printf 'fn main() {}\n' > repo/apps/majordomus-cli/src/main.rs
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
export CALLS="$T/calls" PATH="$T/stand-in:$PATH"

printf '{"schema":1,"mode":"test","reason":"a fixture","selected":[],"gates":[{"id":"economics-claims","job":"structure","runs":"cargo run --quiet --manifest-path apps/majordomus-cli/Cargo.toml -- economics check","selected":true,"network":false,"reason":"t"}]}' > "$T/plan.json"
printf 'economics-claims\t6\n' > "$T/durations.tsv"; : > "$T/cases.tsv"
export MJ_PUSH_DURATIONS="$T/durations.tsv" MJ_PUSH_CASES="$T/cases.tsv"

push() { : > "$CALLS"; "$R" --before-push --plan "$T/plan.json" > "$T/out" 2>&1 || fail "the push failed: $(cat "$T/out")"; }
borrowed() { grep -q '^bin economics check$' "$CALLS" && ! grep -q '^cargo ' "$CALLS"; }
built_by_cargo() { grep -q '^cargo run --quiet --manifest-path apps/majordomus-cli/Cargo.toml -- economics check$' "$CALLS" && ! grep -q '^bin ' "$CALLS"; }

# ---------------------------------------------------------------- 1. built from HEAD, unchanged
MAJORDOMUS_BIN="$T/stand-in/majordomus" MAJORDOMUS_BIN_COMMIT="$built" push
borrowed || fail "an executable of this exact crate was not used: $(cat "$CALLS")"
grep -q "whose crate this tree carries unchanged; the gates run it" "$T/out" || fail "no line says the executable is used: $(cat "$T/out")"

# ---------------------------------------------------------------- 2. the commit from a shared cache path
# the commit is read from the path alone: a MAJORDOMUS_BIN_COMMIT the pusher exported is not this
# fixture's, and would name another commit
(unset MAJORDOMUS_BIN_COMMIT; MAJORDOMUS_BIN="$T/cache/majordomus-master-bin/$built/majordomus" push)
borrowed || fail "the build commit was not read from the cache path: $(cat "$CALLS")"

# ---------------------------------------------------------------- 3. the crate edited
printf 'fn main() { println!("changed"); }\n' > repo/apps/majordomus-cli/src/main.rs
MAJORDOMUS_BIN="$T/stand-in/majordomus" MAJORDOMUS_BIN_COMMIT="$built" push
built_by_cargo || fail "a changed crate was judged by an executable built before the change: $(cat "$CALLS")"
grep -q "the crate differs from ${built:0:11}" "$T/out" || fail "the reason is not named: $(cat "$T/out")"
git -C repo checkout -q -- apps/majordomus-cli/src/main.rs

# ---------------------------------------------------------------- 4. an untracked file in the crate
printf 'pub fn new() {}\n' > repo/apps/majordomus-cli/src/new.rs
MAJORDOMUS_BIN="$T/stand-in/majordomus" MAJORDOMUS_BIN_COMMIT="$built" push
built_by_cargo || fail "an untracked crate file was ignored: $(cat "$CALLS")"
grep -q 'the crate has untracked files' "$T/out" || fail "the reason is not named: $(cat "$T/out")"
rm repo/apps/majordomus-cli/src/new.rs

# ---------------------------------------------------------------- 5. no build commit known
env -u MAJORDOMUS_BIN_COMMIT MAJORDOMUS_BIN="$T/stand-in/majordomus" bash -c 'push() { : > "$CALLS"; "$1" --before-push --plan "$2" > "$3" 2>&1; }; push "$@"' _ "$R" "$T/plan.json" "$T/out" \
  || fail "the push failed: $(cat "$T/out")"
built_by_cargo || fail "an executable of unknown provenance was used: $(cat "$CALLS")"
grep -q 'the commit it was built from is unknown' "$T/out" || fail "the reason is not named: $(cat "$T/out")"

# ---------------------------------------------------------------- 6. none named
env -u MAJORDOMUS_BIN -u MAJORDOMUS_BIN_COMMIT bash -c ': > "$CALLS"; "$1" --before-push --plan "$2" > "$3" 2>&1' _ "$R" "$T/plan.json" "$T/out" \
  || fail "the push failed: $(cat "$T/out")"
grep -q 'executable:' "$T/out" && fail "a line about an executable was printed with none named"
built_by_cargo || fail "with no MAJORDOMUS_BIN the gate did not run cargo run: $(cat "$CALLS")"

echo "    a gate runs an executable only of this exact crate, and cargo run otherwise"

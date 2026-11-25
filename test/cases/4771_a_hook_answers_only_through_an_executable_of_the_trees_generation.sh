# claims: none
# A hook or a gate answers only through an executable of the tree's generation, and the
# executable says which one it is (I4771).
#
# On 2026-11-23 a pre-push of #3111 ran with MAJORDOMUS_BIN naming a build from the evening
# before, older than the clippy fix and the master merge the push carried: the launcher trusts
# MAJORDOMUS_BIN as named, so every gate judged other code than the code that travelled. The
# shell side could tell an executable's commit only from what a caller declared about it, and
# asking the executable cost a command and an index load.
#
# The executable now answers `--build-identity [--tree <root>]` from its compiled-in constants
# before it parses anything; mj_rust_same_generation judges an executable that answers by its
# own digest, and the launcher sets an executable of another generation aside under a git hook.
#
#   1  the real executable answers its commit and generation at once, says `same yes` of the
#      tree it was built from and `same no` of a copy whose crate moved
#   2  `majordomus version --commit` names the commit compiled into the executable
#   3  an answering stand-in is judged by its digest: same, other, other compiled-in files
#   4  its own answer outranks a MAJORDOMUS_BIN_COMMIT that names another commit
#   5  one that predates the answer is judged by the declared commit, as before
#   6  under a hook, MAJORDOMUS_BIN of another generation is set aside, both sides are named,
#      and the tree's own current executable answers; with none, the refusal names the build
#   7  outside a hook, or in a tree that carries no crate, MAJORDOMUS_BIN runs as named
. "$ROOT/test/lib.sh"
unset MAJORDOMUS_BIN_COMMIT CARGO_TARGET_DIR CARGO_BUILD_TARGET_DIR MAJORDOMUS_HOOK
export MAJORDOMUS_INSTALLED_BIN="$T/no-installed-release"

fail() { printf '    %s\n' "$*"; exit 1; }
G1=1111111111111111111111111111111111111111111111111111111111111111
G2=2222222222222222222222222222222222222222222222222222222222222222

# ---------------------------------------------------------------- 1. the real executable
RB="$(rust_bin)" || rust_bin_exit $?
# the suite's own MAJORDOMUS_BIN would show through once a hook sets the fixture's aside
unset MAJORDOMUS_BIN
started="$(date +%s)"
expect_exit 0 "$RB" --build-identity || exit 1
expect_grep '^commit [0-9a-f]{40}$|^commit unknown$' || exit 1
expect_grep '^generation [0-9a-f]{64}$' || exit 1
expect_grep '^dirty (true|false|unknown)$' || exit 1
self_commit="$(sed -n 's/^commit //p' <<<"$LAST_OUT")"
[ $(( $(date +%s) - started )) -le 5 ] || fail "--build-identity took over 5 s: it must not load an index"
expect_exit 0 "$RB" --build-identity --tree "$ROOT" || exit 1
expect_grep '^same yes$' || exit 1
mkdir -p copy/apps
cp -R "$ROOT/apps/majordomus-cli" copy/apps/ && rm -rf copy/apps/majordomus-cli/target
[ -d "$ROOT/apps/majordomus-core" ] && cp -R "$ROOT/apps/majordomus-core" copy/apps/
printf '\n// a change the executable was not built from\n' >> copy/apps/majordomus-cli/src/lib.rs
expect_exit 1 "$RB" --build-identity --tree "$T/copy" || exit 1
expect_grep '^same no$' || exit 1
expect_exit 1 "$RB" --build-identity --tree "$T/nowhere" || exit 1
expect_grep '^tree none$' || exit 1
# anything else on the command line is the command it always was
expect_exit 2 "$RB" --build-identity --nonsense || exit 1

# ---------------------------------------------------------------- 2. version --commit
expect_exit 0 env MAJORDOMUS_BIN="$RB" "$ROOT/bin/majordomus" version --commit || exit 1
[ "$LAST_OUT" = "$self_commit" ] || fail "version --commit printed '$LAST_OUT', the executable says '$self_commit'"
expect_exit 2 "$ROOT/bin/majordomus" version --nonsense || exit 1

# ---------------------------------------------------------------- the fixture
mkdir -p repo/lib repo/apps/majordomus-cli/src stand
cp "$ROOT/lib/rust_bin.sh" repo/lib/rust_bin.sh
printf '[package]\nname = "fixture"\n' > repo/apps/majordomus-cli/Cargo.toml
printf 'fn main() {}\n' > repo/apps/majordomus-cli/src/main.rs
printf '[toolchain]\nchannel = "1.0.0"\n' > repo/rust-toolchain.toml
printf 'target/\n' > repo/.gitignore
gitc() { git -C repo -c user.email=case@example.invalid -c user.name=case "$@"; }
git -C repo init -q && git -C repo add -A && gitc commit -qm crate || fail "could not build the fixture"
built="$(git -C repo rev-parse HEAD)"
# a stand-in that answers as the executable does, from three files the case writes
cat > stand/answering <<SH
#!/usr/bin/env bash
if [ "\${1:-}" = --build-identity ]; then
  printf 'commit %s\ndirty false\ngeneration %s\n' "\$(cat "$T/stand/commit")" "\$(cat "$T/stand/gen")"
  [ "\${2:-}" = --tree ] || exit 0
  t="\$(cat "$T/stand/tree")"; printf 'tree %s\n' "\$t"
  [ "\$t" = "\$(cat "$T/stand/gen")" ] && { echo 'same yes'; exit 0; }
  echo 'same no'; exit 1
fi
echo answering "\$*"
SH
# one built before the answer existed: it refuses the flag, as clap refuses an unknown option
printf '#!/usr/bin/env bash\n[ "${1:-}" = --build-identity ] && exit 2\necho old "$*"\n' > stand/old
chmod +x stand/answering stand/old
answers() { printf '%s\n' "$1" > stand/commit; printf '%s\n' "$2" > stand/gen; printf '%s\n' "$3" > stand/tree; }
same() { ( cd "$T/repo" && . lib/rust_bin.sh && mj_rust_same_generation "$T/repo" "$1" \
  && echo "yes ${MJ_RUST_BUILT}" || echo "no ${MJ_RUST_BUILT}: ${MJ_RUST_GENERATION_WHY}" ); }

# ---------------------------------------------------------------- 3. judged by its digest
answers "$built" "$G1" "$G1"
[ "$(same "$T/stand/answering")" = "yes $built" ] || fail "an executable of this tree's digest was not taken: $(same "$T/stand/answering")"
answers "$built" "$G1" "$G2"
out="$(same "$T/stand/answering")"
case "$out" in "no $built: its generation 111111111111, built from ${built:0:11}, is not this tree's 222222222222") ;;
  *) fail "an executable of another digest was not refused with both sides named: $out" ;; esac
printf '[toolchain]\nchannel = "2.0.0"\n' > repo/rust-toolchain.toml
answers "$built" "$G1" "$G1"
out="$(same "$T/stand/answering")"
case "$out" in no*"what it compiles in from outside the crate"*"differs from ${built:0:11}") ;;
  *) fail "a changed file the build compiles in from outside the crate passed: $out" ;; esac
git -C repo checkout -q -- rust-toolchain.toml

# ---------------------------------------------------------------- 4. its own answer outranks a declaration
answers "$built" "$G1" "$G2"
out="$( cd repo && MAJORDOMUS_BIN="$T/stand/answering" MAJORDOMUS_BIN_COMMIT=0000000000000000000000000000000000000000 \
  bash -c '. lib/rust_bin.sh && mj_rust_built_commit "$MAJORDOMUS_BIN"' )"
[ "$out" = "$built" ] || fail "a declared MAJORDOMUS_BIN_COMMIT outranked the executable's own answer: $out"

# ---------------------------------------------------------------- 5. one that predates the answer
out="$( cd repo && MAJORDOMUS_BIN="$T/stand/old" MAJORDOMUS_BIN_COMMIT="$built" \
  bash -c '. lib/rust_bin.sh && mj_rust_same_generation "$PWD" "$MAJORDOMUS_BIN" && echo yes || echo "no: $MJ_RUST_GENERATION_WHY"' )"
[ "$out" = yes ] || fail "an executable that predates the answer is no longer judged by its declared commit: $out"
out="$( cd repo && MAJORDOMUS_BIN="$T/stand/old" \
  bash -c '. lib/rust_bin.sh && mj_rust_same_generation "$PWD" "$MAJORDOMUS_BIN" && echo yes || echo "no: $MJ_RUST_GENERATION_WHY"' )"
case "$out" in "no: the commit it was built from is unknown"*) ;; *) fail "an old executable of unknown commit was taken: $out" ;; esac

# ---------------------------------------------------------------- 6. under a hook
resolve() { ( cd "$T/repo" && . lib/rust_bin.sh && mj_rust_resolve "$T/repo" 'x: ' ) > "$T/out" 2> "$T/err"; }
OWN="$T/repo/apps/majordomus-cli/target/debug/majordomus"
answers "$built" "$G1" "$G2"
MAJORDOMUS_BIN="$T/stand/answering" MAJORDOMUS_HOOK=1 resolve && fail "a hook ran with no executable of this tree's generation: $(cat "$T/out")"
grep -q "MAJORDOMUS_BIN names $T/stand/answering, which is not of this tree's generation: its generation 111111111111" "$T/err" \
  || fail "the hook does not name the executable it set aside and why: $(cat "$T/err")"
grep -q "this tree is at $(git -C repo rev-parse --short=11 HEAD), so a hook does not answer through it" "$T/err" \
  || fail "the hook does not name the commit it is judging: $(cat "$T/err")"
grep -q 'run: just build' "$T/err" || fail "with no executable of its own the refusal does not name the build: $(cat "$T/err")"
mkdir -p "$(dirname "$OWN")" && printf '#!/usr/bin/env bash\necho own\n' > "$OWN" && chmod +x "$OWN"
touch -t 203001010000 "$OWN"   # newer than every source of the tree: current
MAJORDOMUS_BIN="$T/stand/answering" MAJORDOMUS_HOOK=1 resolve || fail "the tree's own current executable was not asked: $(cat "$T/err")"
[ "$(cat "$T/out")" = "$OWN" ] || fail "the hook ran $(cat "$T/out"), not the tree's own $OWN"
answers "$built" "$G1" "$G1"
MAJORDOMUS_BIN="$T/stand/answering" MAJORDOMUS_HOOK=1 resolve || fail "an executable of this tree's generation was set aside: $(cat "$T/err")"
[ "$(cat "$T/out")" = "$T/stand/answering" ] || fail "a hook did not run MAJORDOMUS_BIN of this tree's generation: $(cat "$T/out")"
[ -s "$T/err" ] && fail "a hook said something about an executable of this tree's generation: $(cat "$T/err")"

# ---------------------------------------------------------------- 7. outside a hook, and without a crate
answers "$built" "$G1" "$G2"
MAJORDOMUS_BIN="$T/stand/answering" resolve || fail "outside a hook MAJORDOMUS_BIN was not run as named: $(cat "$T/err")"
[ "$(cat "$T/out")" = "$T/stand/answering" ] || fail "outside a hook the launcher ran $(cat "$T/out")"
mv repo/apps/majordomus-cli/Cargo.toml repo/no-crate.toml
MAJORDOMUS_BIN="$T/stand/answering" MAJORDOMUS_HOOK=1 resolve || fail "a hook in a tree with no crate refused MAJORDOMUS_BIN: $(cat "$T/err")"
[ "$(cat "$T/out")" = "$T/stand/answering" ] || fail "a hook in a tree with no crate ran $(cat "$T/out")"
exit 0

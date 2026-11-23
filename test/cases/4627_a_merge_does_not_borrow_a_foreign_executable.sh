# claim: derived-data-current
# Where the crate came through a merge, scripts/derive borrows MAJORDOMUS_BIN only when it is of
# the tree's generation, and otherwise builds its own and says why (I4627).
#
# `majordomus generate` lets an executable of another generation through when it projects the
# registry the tree has committed. After a merge that registry is the side merge-derived kept,
# which is what the previous derive's executable wrote, so the check compared that executable
# with itself. On 2026-11-17 a combined top of four pull requests (#3065) derived every member
# with the first member's executable; every check passed and openapi.json lost #3062's
# capabilities until CI built a fresh one.
#
# The fixture is a disposable repository with a copy of scripts/derive and its libraries, a
# crate at apps/majordomus-cli, stub stages, a stub executable that records whether it ran
# borrowed or built, and a stub cargo that "builds" a copy of it:
#
#   1  HEAD merges two crate changes; the executable is the first side's: not borrowed, the
#      derive names the merge and the generation, and builds once
#   2  the same with the executable's commit unknown: not borrowed either
#   3  the same with the executable built from HEAD: borrowed, nothing built
#   4  a merge that leaves the crate alone: borrowed as before, even of an older generation
#   5  a merge in progress that changes the crate: built once, and the scratch derive of the
#      merge borrows that build instead of building a second one
. "$ROOT/test/lib.sh"

fail() { printf '    %s\n' "$*"; exit 1; }
STUB="$(mktemp -d "${TMPDIR:-/tmp}/mj-4627-stub.XXXXXX")"
trap 'rm -rf "$STUB"' EXIT

mkdir -p scripts lib docs apps/majordomus-cli/src "$STUB/bin"
cp "$ROOT/scripts/derive" scripts/derive
cp "$ROOT/lib/machine_lock.sh" "$ROOT/lib/rust_bin.sh" lib/
printf '#!/bin/sh\nexit 0\n' > scripts/rules-package
printf '#!/bin/sh\nexit 0\n' > scripts/generate-site-data
printf '#!/bin/sh\nexit 0\n' > scripts/gitattributes
printf '[package]\nname = "fixture"\nversion = "0.1.0"\n' > apps/majordomus-cli/Cargo.toml
printf 'pub fn base() {}\n' > apps/majordomus-cli/src/lib.rs
echo seed > docs/seed.md
# the executable: says whether it is the borrowed one or a build, and writes nothing
cat > "$STUB/majordomus" <<EOF
#!/bin/sh
[ "\$0" = "$STUB/majordomus" ] && echo borrowed >> "$STUB/exe.log" || echo built >> "$STUB/exe.log"
exit 0
EOF
# cargo: a build is counted and leaves a copy of the executable where cargo would
cat > "$STUB/bin/cargo" <<EOF
#!/bin/sh
case " \$* " in *" build "*)
  echo build >> "$STUB/cargo.log"
  mkdir -p "\$CARGO_TARGET_DIR/debug" && cp "$STUB/majordomus" "\$CARGO_TARGET_DIR/debug/majordomus" ;;
esac
exit 0
EOF
chmod +x "$STUB/majordomus" "$STUB/bin/cargo" scripts/*
PATH="$STUB/bin:$PATH"; export PATH
export CARGO_TARGET_DIR="$STUB/target"
export MAJORDOMUS_DERIVE_LOCK="$STUB/machine/.derive.lock"
export MAJORDOMUS_DERIVE_PRIORITY="$STUB/machine/.derive.priority"
export MAJORDOMUS_DERIVE_TIMINGS=""
# the disk floor, the builder count and the sweeps are other cases' subjects
export MAJORDOMUS_MIN_FREE_MB=0 MAJORDOMUS_MAX_BUILDERS=0 MAJORDOMUS_RECLAIM_BELOW_MB=0
export MAJORDOMUS_INCREMENTAL_IDLE_MIN=0 MAJORDOMUS_PRIVATE_BUILD_IDLE_MIN=0
unset MAJORDOMUS_BIN_COMMIT
git add . && git commit -qm "a crate and a derive"
base="$(git rev-parse --abbrev-ref HEAD)"
D="$T/scripts/derive"

derive() { rm -f "$STUB/cargo.log" "$STUB/exe.log"; MAJORDOMUS_BIN="$STUB/majordomus" "$D"; }
builds() { [ -f "$STUB/cargo.log" ] && grep -c . "$STUB/cargo.log" || echo 0; }
ran() { [ -f "$STUB/exe.log" ] && LC_ALL=C sort -u "$STUB/exe.log" | tr '\n' ' ' || true; }

# two crate changes on two branches, merged
git checkout -q -b two
printf 'pub fn two() {}\n' > apps/majordomus-cli/src/two.rs && git add . && git commit -qm "two"
git checkout -q "$base"
printf 'pub fn base() {}\npub fn one() {}\n' > apps/majordomus-cli/src/lib.rs
git add . && git commit -qm "one"
one="$(git rev-parse HEAD)"
git merge -q --no-ff -m "merge two" two
merge="$(git rev-parse HEAD)"

# ---------------------------------------------------------------- 1. the first side's executable
MAJORDOMUS_BIN_COMMIT="$one" expect_exit 0 derive || exit 1
expect_grep "not borrowing MAJORDOMUS_BIN=$STUB/majordomus: HEAD $(git rev-parse --short=11 HEAD) is a merge that changes the crate" || exit 1
expect_grep "the crate differs from $(printf '%.11s' "$one")" || exit 1
expect_grep 'building this tree.s own' || exit 1
[ "$(builds)" = 1 ] || fail "the first side's executable was not replaced by one build (builds: $(builds))"
[ "$(ran)" = "built " ] || fail "the derive ran the borrowed executable after refusing it (ran: $(ran))"

# ---------------------------------------------------------------- 2. an executable of no known commit
expect_exit 0 derive || exit 1
expect_grep 'not borrowing .*\(the commit it was built from is unknown' || exit 1
[ "$(builds)" = 1 ] || fail "an executable of unknown generation was borrowed over a merged crate"

# ---------------------------------------------------------------- 3. an executable of this tree's generation
MAJORDOMUS_BIN_COMMIT="$merge" expect_exit 0 derive || exit 1
expect_no_grep 'not borrowing' || exit 1
expect_grep "MAJORDOMUS_BIN=$STUB/majordomus \\(not building" || exit 1
[ "$(builds)" = 0 ] || fail "an executable of this tree's generation was rebuilt"
[ "$(ran)" = "borrowed " ] || fail "the borrowed executable did not run (ran: $(ran))"

# ---------------------------------------------------------------- 4. a merge that leaves the crate alone
git checkout -q -b prose
echo prose > docs/prose.md && git add . && git commit -qm "prose"
git checkout -q "$base"
echo more > docs/more.md && git add . && git commit -qm "more"
git merge -q --no-ff -m "merge prose" prose
MAJORDOMUS_BIN_COMMIT="$one" expect_exit 0 derive || exit 1
expect_no_grep 'not borrowing' || exit 1
[ "$(builds)" = 0 ] || fail "a merge that left the crate alone made the derive build"
expect_exit 0 derive || exit 1
[ "$(builds)" = 0 ] || fail "a merge that left the crate alone refused an executable of unknown commit"

# ---------------------------------------------------------------- 5. a merge in progress
git checkout -q -b four "$one"
printf 'pub fn four() {}\n' > apps/majordomus-cli/src/four.rs && git add . && git commit -qm "four"
git checkout -q "$base"
git merge -q --no-ff --no-commit four >/dev/null 2>&1
[ -e "$(git rev-parse --git-dir)/MERGE_HEAD" ] || fail "the fixture did not leave a merge uncommitted"
MAJORDOMUS_BIN_COMMIT="$(git rev-parse HEAD)" expect_exit 0 derive || exit 1
expect_grep 'not borrowing .* the merge in progress \(MERGE_HEAD [0-9a-f]{11}\) changes the crate' || exit 1
[ "$(grep -c 'not borrowing' <<<"$LAST_OUT")" = 1 ] || fail "the scratch derive of the merge refused the build made for it"
[ "$(builds)" = 1 ] || fail "a merge in progress was not built exactly once (builds: $(builds))"
expect_grep 'the merge is derived and staged' || exit 1
[ "$(ran)" = "built " ] || fail "the merge was derived with something other than its build (ran: $(ran))"
exit 0

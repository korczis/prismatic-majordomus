# majordomus-covers: none
# claims: mcp-client-always-served
# A client that spawns `majordomus mcp` gets a server whenever one can be given, and is told
# which one — or is told plainly that there is none.
#
# On 2026-10-16 a session opened in a checkout whose executable had never been built, on a
# volume with 4.5GB free against a floor of 5120MB. bin/majordomus-cli refused the build, as
# it must, and exited 12; the client saw `CONNECTION_CLOSED`, went on without the server,
# and the session worked as the only worker of a repository that has many (ADR 0226). A
# release that answers the same protocol was installed beside it the whole time.
#
# Amended for ADR 0224 (I3957), which supersedes ADR 0172: no command builds, `mcp` included,
# and the installed release stands in for a missing or stale executable only when it is of
# this tree's generation — the commit its RELEASE.json names carries this crate unchanged. A
# version string is not a generation: a release that cannot say so is refused with exit 12
# naming `just build`, because a server of another crate answers for a tree it was not built
# from. With a release of the same generation every command may use it, not `mcp` alone. The
# fixture is a copy of the launcher over stubs: a cargo that records it ran, no network, no
# real server.
. "$ROOT/test/lib.sh"

S="$(mktemp -d "${TMPDIR:-/tmp}/mj3177.XXXXXX")"; trap 'rm -rf "$S"' EXIT
R="$S/repo"; TARGET="$S/target"
mkdir -p "$R/bin" "$R/lib" "$R/apps/majordomus-cli/src" "$TARGET/debug" "$S/none" "$S/withcargo"
cp "$ROOT/bin/majordomus-cli" "$ROOT/bin/majordomus-mcp" "$R/bin/"
cp "$ROOT/lib/rust_bin.sh" "$R/lib/"
printf '[package]\nname = "majordomus-cli"\n' > "$R/apps/majordomus-cli/Cargo.toml"
: > "$R/apps/majordomus-cli/src/lib.rs"
printf 'target/\n' > "$R/.gitignore"
git -C "$R" init -q && git -C "$R" add -A \
  && git -C "$R" -c user.email=case@example.invalid -c user.name=case commit -qm crate \
  || { echo "    could not build the fixture repository"; exit 1; }
HEAD_SHA="$(git -C "$R" rev-parse HEAD)"

# a PATH that holds the tools the launcher uses and no cargo
for t in bash env dirname basename df awk find head sed tr cat mkdir uname date ls rm grep touch mktemp mv cp chmod sleep id stat wc sort readlink ln cut tail xargs git; do
  p="$(command -v "$t" 2>/dev/null)" && ln -sf "$p" "$S/none/$t"
done
cp -R "$S/none/." "$S/withcargo/"
# a cargo that records that it ran and fails: nothing may call it
printf '#!/bin/sh\necho "$@" >> "%s/cargo.log"\nexit 1\n' "$S" > "$S/withcargo/cargo"; chmod +x "$S/withcargo/cargo"

# the installed release, laid out as a version is installed: bin/majordomus beside RELEASE.json
INSTALLED="$S/installed/bin/majordomus"; mkdir -p "$S/installed/bin"
printf '#!/bin/sh\n[ "$1" = --version ] && { echo "majordomus 9.9.9"; exit; }\necho "INSTALLED $*"\n' > "$INSTALLED"; chmod +x "$INSTALLED"
stamp() { printf '{\n  "schema": "majordomus/build/v1",\n  "version": "9.9.9",\n  "commit": "%s"\n}\n' "$1" > "$S/installed/RELEASE.json"; }
called_cargo() { [ -s "$S/cargo.log" ]; }

# run <path-dir> <args...>: the launcher with nothing inherited that would change its mind
run() {
  local pdir="$1"; shift
  env -i HOME="$S/home" PATH="$pdir" CARGO_TARGET_DIR="$TARGET" \
    MAJORDOMUS_INSTALLED_BIN="${INST:-$INSTALLED}" \
    "$R/bin/majordomus-cli" "$@" >"$S/out" 2>"$S/err"
}
fail() { printf '    %s\n    stdout: %s\n    stderr: %s\n' "$1" "$(cat "$S/out")" "$(cat "$S/err")"; exit 1; }

# 1. no executable, a release of this generation: mcp is served by it, and says so
stamp "$HEAD_SHA"
rc=0; run "$S/none" mcp --standalone || rc=$?
[ "$rc" = 0 ] || fail "mcp with no executable and a release of this generation exited $rc, not 0"
grep -qx 'INSTALLED mcp --standalone' "$S/out" || fail "the installed release did not get the arguments"
grep -q 'is missing; answering through the installed release .*, built from .*, whose crate this tree carries unchanged' "$S/err" \
  || fail "stderr does not say why the installed release ran"

# 2. ... and so is any other command, which asks the same question
rc=0; run "$S/none" worktree status || rc=$?
[ "$rc" = 0 ] && grep -qx 'INSTALLED worktree status' "$S/out" || fail "a command other than mcp was not served by a release of this generation"

# 3. a release of another generation is refused, mcp or not, and the refusal says why
stamp 0000000000000000000000000000000000000000
for cmd in mcp "worktree status"; do
  rc=0; run "$S/none" $cmd || rc=$?
  [ "$rc" = 12 ] || fail "'$cmd' with a release of another generation exited $rc, not 12"
  ! grep -q INSTALLED "$S/out" || fail "'$cmd' ran a release of another generation"
  grep -q "is not of this tree's generation" "$S/err" || fail "'$cmd': the refusal does not say why the release was not used"
  grep -q 'run: just build' "$S/err" || fail "'$cmd': the refusal does not name the explicit build"
done
# a release whose stamp names no commit is no more of this generation than one naming another
rm -f "$S/installed/RELEASE.json"
rc=0; run "$S/none" mcp || rc=$?
[ "$rc" = 12 ] || fail "mcp with a release that names no build commit exited $rc, not 12"

# 4. an executable that is stale, with cargo on PATH: nothing is built; a release of this
#    generation answers, and without one it is refused
printf '#!/bin/sh\necho SOURCE\n' > "$TARGET/debug/majordomus"; chmod +x "$TARGET/debug/majordomus"
touch -t 202001010000 "$TARGET/debug/majordomus"
rm -f "$S/cargo.log"; stamp "$HEAD_SHA"
rc=0; run "$S/withcargo" mcp || rc=$?
[ "$rc" = 0 ] && grep -qx 'INSTALLED mcp' "$S/out" || fail "mcp with a stale executable was not served by a release of this generation"
grep -q 'is stale (older than its sources); answering through the installed release' "$S/err" || fail "stderr does not say the executable is stale"
stamp 0000000000000000000000000000000000000000
rc=0; run "$S/withcargo" mcp || rc=$?
[ "$rc" = 12 ] || fail "mcp with a stale executable and no release of this generation exited $rc, not 12"
! called_cargo || fail "a stale executable started cargo: $(cat "$S/cargo.log")"

# 5. a fresh executable is the checkout's own, never the installed one
touch "$TARGET/debug/majordomus"; stamp "$HEAD_SHA"
rc=0; run "$S/none" mcp || rc=$?
[ "$rc" = 0 ] && grep -qx SOURCE "$S/out" || fail "a fresh executable did not run as itself"

# 6. the crate moved after the release was cut: the release is no longer of this generation
rm -f "$TARGET/debug/majordomus"
printf 'pub fn moved() {}\n' > "$R/apps/majordomus-cli/src/lib.rs"
rc=0; run "$S/withcargo" mcp || rc=$?
[ "$rc" = 12 ] || fail "a release built before the crate moved was used (exit $rc)"
grep -q 'the crate differs from' "$S/err" || fail "the refusal does not say the crate differs"
git -C "$R" checkout -q -- apps/majordomus-cli/src/lib.rs
! called_cargo || fail "cargo was called: $(cat "$S/cargo.log")"

# 7. nothing installed: the failure is reported as it always was
rc=0; INST="$S/not-there" run "$S/none" mcp || rc=$?
[ "$rc" = 12 ] || fail "mcp with nothing installed exited $rc, not 12"

# 8. an "installed release" inside the repository is the launcher finding itself: refused
cp "$INSTALLED" "$R/bin/itself"
rc=0; INST="$R/bin/itself" run "$S/none" mcp || rc=$?
[ "$rc" = 12 ] || fail "a file inside the repository was taken for the installed release (exit $rc)"

# 9. one that is executable but does not answer --version is not a release
printf '#!/bin/sh\nexit 3\n' > "$S/installed/bin/broken"; chmod +x "$S/installed/bin/broken"
rc=0; INST="$S/installed/bin/broken" run "$S/none" mcp || rc=$?
[ "$rc" = 12 ] || fail "an executable that does not answer --version was taken for a release (exit $rc)"

# 10. the client configurations still name the one launcher; nothing here moved them
grep -q '"command": "bin/majordomus-mcp"' "$ROOT/.mcp.json" || { echo "    .mcp.json no longer starts bin/majordomus-mcp"; exit 1; }

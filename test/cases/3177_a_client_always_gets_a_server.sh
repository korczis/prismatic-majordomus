# majordomus-covers: none
# claims: mcp-client-always-served
# A client that spawns `majordomus mcp` always gets a server, and is told which one.
#
# On 2026-10-16 a session opened in a checkout whose executable had never been built, on a
# volume with 4.5GB free against a floor of 5120MB. bin/majordomus-cli refused the build, as
# it must, and exited 12; the client saw `CONNECTION_CLOSED`, went on without the server,
# and the session worked as the only worker of a repository that has many (ADR 0156). A
# release that answers the same protocol was installed beside it the whole time.
#
# So `mcp`, and only `mcp`, falls back to the installed release when this checkout has no
# executable to run, or cannot make one, and says on stderr which and why. Every other
# command still exits 12, and a stale executable that can be rebuilt is still rebuilt. The
# fixture is a copy of the launcher over stubs: no cargo, no network, no real server.
. "$ROOT/test/lib.sh"

S="$(mktemp -d "${TMPDIR:-/tmp}/mj3177.XXXXXX")"; trap 'rm -rf "$S"' EXIT
R="$S/repo"; TARGET="$S/target"
mkdir -p "$R/bin" "$R/lib" "$R/apps/majordomus-cli/src" "$TARGET/debug" "$S/none" "$S/withcargo"
cp "$ROOT/bin/majordomus-cli" "$ROOT/bin/majordomus-mcp" "$R/bin/"
cp "$ROOT/lib/rust_bin.sh" "$R/lib/"
printf '[package]\nname = "majordomus-cli"\n' > "$R/apps/majordomus-cli/Cargo.toml"
: > "$R/apps/majordomus-cli/src/lib.rs"

# a PATH that holds the tools the launcher uses and no cargo
for t in bash env dirname basename df awk find head sed tr cat mkdir uname date ls rm grep touch mktemp mv cp chmod sleep id stat wc sort readlink ln cut tail xargs; do
  p="$(command -v "$t" 2>/dev/null)" && ln -sf "$p" "$S/none/$t"
done
cp -R "$S/none/." "$S/withcargo/"
# a cargo that records that it ran and fails: a build that cannot succeed here
printf '#!/bin/sh\necho "$@" >> "%s/cargo.log"\nexit 1\n' "$S" > "$S/withcargo/cargo"; chmod +x "$S/withcargo/cargo"

# the installed release: answers --version and says it ran
INSTALLED="$S/installed/majordomus"; mkdir -p "$S/installed"
printf '#!/bin/sh\n[ "$1" = --version ] && { echo "majordomus 9.9.9"; exit 0; }\necho "INSTALLED $*"\n' > "$INSTALLED"; chmod +x "$INSTALLED"

# run <path-dir> <args...>: the launcher with nothing inherited that would change its mind
run() {
  local pdir="$1"; shift
  env -i HOME="$S/home" PATH="$pdir" CARGO_TARGET_DIR="$TARGET" \
    MAJORDOMUS_INSTALLED_BIN="${INST:-$INSTALLED}" MAJORDOMUS_MIN_FREE_MB="${FREE:-0}" \
    ${EXTRA:+$EXTRA} "$R/bin/majordomus-cli" "$@" >"$S/out" 2>"$S/err"
}
fail() { printf '    %s\n    stdout: %s\n    stderr: %s\n' "$1" "$(cat "$S/out")" "$(cat "$S/err")"; exit 1; }

# 1. no executable, no cargo: mcp is served by the installed release, and says so
rc=0; run "$S/none" mcp --standalone || rc=$?
[ "$rc" = 0 ] || fail "mcp with no executable and no cargo exited $rc, not 0"
grep -qx 'INSTALLED mcp --standalone' "$S/out" || fail "the installed release did not get the arguments"
grep -q 'is missing; serving this client from the installed release' "$S/err" || fail "stderr does not say why the installed release ran"
grep -q 'majordomus 9.9.9' "$S/err" || fail "stderr does not name the installed release's version"

# 2. no executable, no cargo, any other command: still the documented refusal
rc=0; run "$S/none" worktree status || rc=$?
[ "$rc" = 12 ] || fail "a command other than mcp exited $rc, not 12"
! grep -q INSTALLED "$S/out" || fail "a command other than mcp ran the installed release"

# 3. an executable that is stale and cannot be rebuilt: the build is tried, fails, mcp falls back
printf '#!/bin/sh\necho SOURCE\n' > "$TARGET/debug/majordomus"; chmod +x "$TARGET/debug/majordomus"
touch -t 202001010000 "$TARGET/debug/majordomus"
rm -f "$S/cargo.log"
rc=0; run "$S/withcargo" mcp || rc=$?
[ "$rc" = 0 ] || fail "mcp with a failing build exited $rc, not 0"
[ -s "$S/cargo.log" ] || fail "a stale executable that can be rebuilt was not rebuilt first"
grep -q 'the build failed; serving this client from the installed release' "$S/err" || fail "stderr does not say the build failed"

# 4. too little room: no build is started at all, mcp falls back
rm -f "$S/cargo.log"
rc=0; FREE=999999999 run "$S/withcargo" mcp || rc=$?
[ "$rc" = 0 ] || fail "mcp below the free-space floor exited $rc, not 0"
[ ! -e "$S/cargo.log" ] || fail "a build was started below the free-space floor"
grep -q 'there is not room to build; serving this client from the installed release' "$S/err" || fail "stderr does not say there was no room"
# ... and any other command is still refused, not served by the wrong release
rc=0; FREE=999999999 run "$S/withcargo" worktree status || rc=$?
[ "$rc" = 12 ] || fail "a command other than mcp below the floor exited $rc, not 12"

# 5. a fresh executable is the checkout's own, never the installed one
touch "$TARGET/debug/majordomus"
rc=0; run "$S/none" mcp || rc=$?
[ "$rc" = 0 ] && grep -qx SOURCE "$S/out" || fail "a fresh executable did not run as itself"

# 6. a contributor who wants the build asks for it
rm -f "$TARGET/debug/majordomus"
rc=0; EXTRA=MAJORDOMUS_MCP_BUILD=1 run "$S/none" mcp || rc=$?
[ "$rc" = 12 ] || fail "MAJORDOMUS_MCP_BUILD=1 exited $rc, not the build's own 12"
! grep -q INSTALLED "$S/out" || fail "MAJORDOMUS_MCP_BUILD=1 ran the installed release"

# 7. nothing installed: the failure is reported as it always was
rc=0; INST="$S/not-there" run "$S/none" mcp || rc=$?
[ "$rc" = 12 ] || fail "mcp with nothing installed exited $rc, not 12"

# 8. an "installed release" inside the repository is the launcher finding itself: refused
cp "$INSTALLED" "$R/bin/itself"
rc=0; INST="$R/bin/itself" run "$S/none" mcp || rc=$?
[ "$rc" = 12 ] || fail "a file inside the repository was taken for the installed release (exit $rc)"

# 9. one that is executable but does not answer --version is not a release
printf '#!/bin/sh\nexit 3\n' > "$S/installed/broken"; chmod +x "$S/installed/broken"
rc=0; INST="$S/installed/broken" run "$S/none" mcp || rc=$?
[ "$rc" = 12 ] || fail "an executable that does not answer --version was taken for a release (exit $rc)"

# 10. the client configurations still name the one launcher; nothing here moved them
grep -q '"command": "bin/majordomus-mcp"' "$ROOT/.mcp.json" || { echo "    .mcp.json no longer starts bin/majordomus-mcp"; exit 1; }

# majordomus-covers: none
# A git hook never starts a cold build of the executable (I3831). On 2026-10-21 a commit hook
# found its worktree's target removed by a disk sweep, waited on the shared build directory's
# lock and compiled every dependency cold, at a load of 77, for a check that takes a second.
# The hooks now say MAJORDOMUS_HOOK=1, and bin/majordomus-cli answers a hook whose checkout
# has no executable from the installed release, saying so; a stale executable that exists is
# still rebuilt, and outside a hook nothing changes.
#
# Driven in a fixture with a stand-in cargo that only records that it was called, and a
# stand-in installed release, so nothing compiles.
. "$ROOT/test/lib.sh"

F="$T/fixture"; FAKE="$T/fake"; CALLS="$T/cargo.calls"
mkdir -p "$F/bin" "$F/lib" "$F/apps/majordomus-cli/src" "$FAKE" "$T/installed"
cp "$ROOT/bin/majordomus-cli" "$F/bin/"; cp "$ROOT/lib/rust_bin.sh" "$F/lib/"
cp "$ROOT/apps/majordomus-cli/Cargo.toml" "$ROOT/apps/majordomus-cli/Cargo.lock" "$F/apps/majordomus-cli/"
: > "$F/apps/majordomus-cli/src/main.rs"
cat > "$FAKE/cargo" <<'SH'
#!/bin/sh
[ "$1" = metadata ] && exit 1
printf '%s\n' "$*" >> "$FAKE_CALLS"
exit 101
SH
cat > "$T/installed/majordomus" <<'SH'
#!/bin/sh
[ "$1" = --version ] && { echo "majordomus 0.0.0-installed"; exit 0; }
echo "installed answered: $*"
SH
chmod +x "$FAKE/cargo" "$T/installed/majordomus"

# ask [VAR=value ...] -- the launcher in the fixture asked for `worktree guard`; sets RC, OUT, ERR
ask() {
  : > "$CALLS"; RC=0
  env -u MAJORDOMUS_BIN -u MAJORDOMUS_HOOK PATH="$FAKE:$PATH" FAKE_CALLS="$CALLS" \
      CARGO_HOME="$T/cargo-home" CARGO_TARGET_DIR="$T/target" \
      MAJORDOMUS_MIN_FREE_MB=0 MAJORDOMUS_INCREMENTAL_IDLE_MIN=0 "$@" \
      bash "$F/bin/majordomus-cli" worktree guard --quiet > "$T/out" 2> "$T/err" || RC=$?
  OUT="$(cat "$T/out")"; ERR="$(cat "$T/err")"
}

# ---------------------------------------------------------------- a hook, no executable: the installed release
ask MAJORDOMUS_HOOK=1 MAJORDOMUS_INSTALLED_BIN="$T/installed/majordomus"
[ "$RC" = 0 ] || { echo "    the hook's ask failed: rc $RC: $ERR"; exit 1; }
[ "$OUT" = "installed answered: worktree guard --quiet" ] \
  || { echo "    the installed release did not answer the hook: $OUT / $ERR"; exit 1; }
[ ! -s "$CALLS" ] || { echo "    a hook started cargo: $(cat "$CALLS")"; exit 1; }
printf '%s\n' "$ERR" | grep -q 'the hook asks the installed release .* instead of a cold build' \
  || { echo "    the hook was not told why: $ERR"; exit 1; }

# ---------------------------------------------------------------- outside a hook: it builds as before
ask MAJORDOMUS_INSTALLED_BIN="$T/installed/majordomus"
grep -q '^build' "$CALLS" || { echo "    outside a hook the missing executable was not built: $ERR"; exit 1; }

# ---------------------------------------------------------------- a hook with no installed release: it builds as before
ask MAJORDOMUS_HOOK=1 MAJORDOMUS_INSTALLED_BIN="$T/installed/none"
grep -q '^build' "$CALLS" || { echo "    a hook with no installed release did not build: $ERR"; exit 1; }

# ---------------------------------------------------------------- every hook says it is one
for h in pre-commit commit-msg pre-push; do
  grep -qx 'MAJORDOMUS_HOOK=1; export MAJORDOMUS_HOOK' "$ROOT/.githooks/$h" \
    || { echo "    .githooks/$h does not export MAJORDOMUS_HOOK"; exit 1; }
done
echo "    a hook with no executable asks the installed release; outside a hook, or with none, it builds"

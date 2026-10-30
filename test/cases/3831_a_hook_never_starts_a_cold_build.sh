# majordomus-covers: none
# A git hook never starts a cold build of the executable (I3831). On 2026-10-21 a commit hook
# found its worktree's target removed by a disk sweep, waited on the shared build directory's
# lock and compiled every dependency cold, at a load of 77, for a check that takes a second.
# The hooks say MAJORDOMUS_HOOK=1, and bin/majordomus-cli answers a hook whose checkout has no
# executable from the installed release, saying so.
#
# Amended for ADR 0224 (I3957), which supersedes the build this case used to expect: no
# caller builds, a hook or not, and the installed release answers only when it is of this
# tree's generation — the commit its RELEASE.json names carries this crate unchanged. A
# release that cannot say so is refused, and so is the hook: exit 12 naming `just build`.
#
# Driven in a fixture with a stand-in cargo that only records that it was called, and a
# stand-in installed release, so nothing compiles.
. "$ROOT/test/lib.sh"

F="$T/fixture"; FAKE="$T/fake"; CALLS="$T/cargo.calls"
mkdir -p "$F/bin" "$F/lib" "$F/apps/majordomus-cli/src" "$FAKE" "$T/release/bin"
cp "$ROOT/bin/majordomus-cli" "$F/bin/"; cp "$ROOT/lib/rust_bin.sh" "$F/lib/"
cp "$ROOT/apps/majordomus-cli/Cargo.toml" "$ROOT/apps/majordomus-cli/Cargo.lock" "$F/apps/majordomus-cli/"
: > "$F/apps/majordomus-cli/src/main.rs"
git -C "$F" init -q && git -C "$F" add -A \
  && git -C "$F" -c user.email=case@example.invalid -c user.name=case commit -qm crate \
  || { echo "    could not build the fixture repository"; exit 1; }
cat > "$FAKE/cargo" <<'SH'
#!/bin/sh
[ "$1" = metadata ] && exit 1
printf '%s\n' "$*" >> "$FAKE_CALLS"
exit 101
SH
# a release laid out as an installed version is: bin/majordomus beside a RELEASE.json stamp
cat > "$T/release/bin/majordomus" <<'SH'
#!/bin/sh
[ "$1" = --version ] && { echo "majordomus 0.0.0-installed"; exit 0; }
echo "installed answered: $*"
SH
chmod +x "$FAKE/cargo" "$T/release/bin/majordomus"
stamp() { printf '{\n  "schema": "majordomus/build/v1",\n  "commit": "%s"\n}\n' "$1" > "$T/release/RELEASE.json"; }

# ask [VAR=value ...] -- the launcher in the fixture asked for `worktree guard`; sets RC, OUT, ERR
ask() {
  : > "$CALLS"; RC=0
  env -u MAJORDOMUS_BIN -u MAJORDOMUS_HOOK PATH="$FAKE:$PATH" FAKE_CALLS="$CALLS" \
      CARGO_HOME="$T/cargo-home" CARGO_TARGET_DIR="$T/target" \
      MAJORDOMUS_MIN_FREE_MB=0 MAJORDOMUS_INCREMENTAL_IDLE_MIN=0 \
      MAJORDOMUS_INSTALLED_BIN="$T/release/bin/majordomus" "$@" \
      bash "$F/bin/majordomus-cli" worktree guard --quiet > "$T/out" 2> "$T/err" || RC=$?
  OUT="$(cat "$T/out")"; ERR="$(cat "$T/err")"
  [ ! -s "$CALLS" ] || { echo "    the launcher started cargo: $(cat "$CALLS")"; exit 1; }
}

# ---------------------------------------------------------------- a hook, no executable, a release of this generation
stamp "$(git -C "$F" rev-parse HEAD)"
ask MAJORDOMUS_HOOK=1
[ "$RC" = 0 ] || { echo "    the hook's ask failed: rc $RC: $ERR"; exit 1; }
[ "$OUT" = "installed answered: worktree guard --quiet" ] \
  || { echo "    the installed release did not answer the hook: $OUT / $ERR"; exit 1; }
printf '%s\n' "$ERR" | grep -q 'answering through the installed release .* whose crate this tree carries unchanged' \
  || { echo "    the hook was not told why: $ERR"; exit 1; }

# ---------------------------------------------------------------- a hook, a release of another generation: refused
stamp 0000000000000000000000000000000000000000
ask MAJORDOMUS_HOOK=1
[ "$RC" = 12 ] || { echo "    a release of another generation was not refused: rc $RC: $OUT / $ERR"; exit 1; }
[ -z "$OUT" ] || { echo "    a refused release answered: $OUT"; exit 1; }
printf '%s\n' "$ERR" | grep -q 'run: just build' || { echo "    the refusal does not name the build: $ERR"; exit 1; }

# ---------------------------------------------------------------- outside a hook, nothing installed: refused, never built
ask MAJORDOMUS_INSTALLED_BIN="$T/none"
[ "$RC" = 12 ] || { echo "    a missing executable outside a hook exited $RC, not 12: $ERR"; exit 1; }

# ---------------------------------------------------------------- every hook says it is one, and what a refusal means
for h in pre-commit commit-msg pre-push; do
  grep -qx 'MAJORDOMUS_HOOK=1; export MAJORDOMUS_HOOK' "$ROOT/.githooks/$h" \
    || { echo "    .githooks/$h does not export MAJORDOMUS_HOOK"; exit 1; }
done
echo "    a hook is answered by a release of this generation or refused; nothing is built"

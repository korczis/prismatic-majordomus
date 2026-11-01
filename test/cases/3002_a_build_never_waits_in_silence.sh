# majordomus-covers: none
# A build never waits in silence behind another build's lock
# (project.a-build-never-waits-in-silence-on-anothers-lock).
#
# The worktrees of this repository share one build directory (ADR 0139), cargo takes one
# exclusive lock on it, and on 2026-10-13 a commit hook's `cargo build --quiet` stood behind
# that lock for fourteen minutes: no CPU, no child, no line of output, because `--quiet`
# removes the one line cargo says about it. lib/rust_bin.sh's mj_cargo_bounded is what every
# build of the executable goes through now. This case drives it with a stand-in `cargo`, so
# it proves the behaviour in seconds and compiles nothing.
#
# What is proved: a build nobody is in the way of runs once, in the shared directory, and
# says nothing; a build behind a lock waits to the bound, says what it waited behind, and is
# run again in this tree's own directory; a build that got the lock before the bound is not
# moved; a build already in its own directory that is still blocked stops with 75 rather
# than wait; the disk floor is asked before a private build starts; a failing build shows
# its error although the caller asked for quiet; the derive, one of the two explicit builds,
# builds through this function; and the launcher the commit hook runs calls cargo in no way at
# all, because since ADR 0224 it never builds (case 3958 walks it with cargo on PATH).
. "$ROOT/test/lib.sh"

LIB="$ROOT/lib/rust_bin.sh"
grep -q '^mj_cargo_bounded()' "$LIB" || { echo "    lib/rust_bin.sh defines no mj_cargo_bounded"; exit 1; }

FAKE="$T/fake"; mkdir -p "$FAKE"
CALLS="$T/calls.log"
# The stand-in. It records the build directory each call was given, and behaves as
# $FAKE_MODE says. `exec sleep` so that the process the function stops is the one waiting.
cat > "$FAKE/cargo" <<'SH'
#!/bin/sh
# no build: what the floor's refusal asks to name the largest build directories (I2713)
[ "$1" = metadata ] && exit 1
printf 'dir=%s args=%s\n' "${CARGO_BUILD_BUILD_DIR:-shared}" "$*" >> "$FAKE_CALLS"
blocked() { printf '    Blocking waiting for file lock on build directory\n' >&2; }
case "$FAKE_MODE" in
  free)   printf '   Compiling x v0.0.0\n' >&2; exit 0 ;;
  held)   if [ -z "${CARGO_BUILD_BUILD_DIR:-}" ]; then blocked; exec sleep 300; fi
          printf '   Compiling x v0.0.0\n' >&2; exit 0 ;;
  always) blocked; exec sleep 300 ;;
  late)   blocked; sleep 2; printf '   Compiling x v0.0.0\n' >&2; exit 0 ;;
  fail)   printf 'error: could not compile x\n' >&2; exit 101 ;;
esac
SH
chmod +x "$FAKE/cargo"

# run <mode> <bound> [VAR=value ...] -- the function under a stand-in cargo; sets RC, ERR
# The sweep of idle incremental sessions a build makes first (I2713) is switched off: it asks
# cargo where the build directory is, which the stand-in would count as a build. It is
# proved in test/cases/3080_idle_incremental_sessions_are_pruned.sh.
run() {
  mode="$1"; bound="$2"; shift 2
  : > "$CALLS"
  RC=0
  ERR="$(env PATH="$FAKE:$PATH" FAKE_MODE="$mode" FAKE_CALLS="$CALLS" CARGO_HOME="$T/cargo-home" \
        MAJORDOMUS_BUILD_LOCK_WAIT="$bound" MAJORDOMUS_MIN_FREE_MB=0 MAJORDOMUS_INCREMENTAL_IDLE_MIN=0 \
        MAJORDOMUS_PRIVATE_BUILD_ABOVE_MB=0 "$@" \
        sh -c '. "$1"; mj_cargo_bounded "$2" build --quiet --locked' sh "$LIB" "$T" 2>&1 >/dev/null)" || RC=$?
}
calls() { wc -l < "$CALLS" | tr -d ' '; }

# ---------------------------------------------------------------- nobody in the way
run free 3 env -u CARGO_BUILD_BUILD_DIR
[ "$RC" = 0 ] || { echo "    a free build did not succeed: rc $RC: $ERR"; exit 1; }
[ "$(calls)" = 1 ] || { echo "    a free build ran cargo $(calls) time(s), expected 1"; exit 1; }
grep -q '^dir=shared ' "$CALLS" || { echo "    a free build left the shared directory: $(cat "$CALLS")"; exit 1; }
[ -z "$ERR" ] || { echo "    a quiet build that succeeded printed: $ERR"; exit 1; }
grep -q -- '--quiet' "$CALLS" && { echo "    --quiet reached cargo, which removes the line the bound reads: $(cat "$CALLS")"; exit 1; }
grep -q -- '--locked' "$CALLS" || { echo "    an argument of the caller was lost: $(cat "$CALLS")"; exit 1; }

# ---------------------------------------------------------------- behind a lock: bounded, said, moved
start="$(date +%s)"
run held 2 env -u CARGO_BUILD_BUILD_DIR
took=$(( $(date +%s) - start ))
[ "$RC" = 0 ] || { echo "    a build behind a lock did not finish in its own directory: rc $RC: $ERR"; exit 1; }
[ "$took" -le 30 ] || { echo "    a bound of 2s took ${took}s"; exit 1; }
[ "$(calls)" = 2 ] || { echo "    expected the shared attempt and one private one, got $(calls): $(cat "$CALLS")"; exit 1; }
sed -n 2p "$CALLS" | grep -q "^dir=$T/cargo-home/build-private/" \
  || { echo "    the second attempt was not in this tree's own directory: $(cat "$CALLS")"; exit 1; }
printf '%s\n' "$ERR" | grep -q "waited .*behind another build's lock" \
  || { echo "    the wait was not said: $ERR"; exit 1; }
printf '%s\n' "$ERR" | grep -q "building in this tree's own build directory instead" \
  || { echo "    the way out was not said: $ERR"; exit 1; }

# ---------------------------------------------------------------- the bound is zero: no wait at all
run held 0 env -u CARGO_BUILD_BUILD_DIR
[ "$RC" = 0 ] && [ "$(calls)" = 2 ] || { echo "    a bound of 0 did not move at the first sign: rc $RC, $(calls) call(s)"; exit 1; }

# ---------------------------------------------------------------- the lock came free in time
run late 30 env -u CARGO_BUILD_BUILD_DIR
[ "$RC" = 0 ] || { echo "    a build that got the lock before the bound failed: rc $RC: $ERR"; exit 1; }
[ "$(calls)" = 1 ] || { echo "    a build that got the lock before the bound was moved anyway: $(cat "$CALLS")"; exit 1; }
[ -z "$ERR" ] || { echo "    a build that only waited a moment printed: $ERR"; exit 1; }

# ---------------------------------------------------------------- nowhere else to go: stop, do not wait
run always 2 CARGO_BUILD_BUILD_DIR="$T/mine"
[ "$RC" = 75 ] || { echo "    a blocked build in its own directory returned $RC, expected 75: $ERR"; exit 1; }
[ "$(calls)" = 1 ] || { echo "    it tried again with nowhere to go: $(cat "$CALLS")"; exit 1; }
printf '%s\n' "$ERR" | grep -q 'nowhere else to build' || { echo "    the stop was not explained: $ERR"; exit 1; }

# ---------------------------------------------------------------- the disk floor is asked first
: > "$CALLS"; RC=0
ERR="$(env -u CARGO_BUILD_BUILD_DIR PATH="$FAKE:$PATH" FAKE_MODE=held FAKE_CALLS="$CALLS" CARGO_HOME="$T/cargo-home" \
      MAJORDOMUS_BUILD_LOCK_WAIT=1 MAJORDOMUS_MIN_FREE_MB=999999999 MAJORDOMUS_INCREMENTAL_IDLE_MIN=0 \
      sh -c '. "$1"; mj_cargo_bounded "$2" build --quiet' sh "$LIB" "$T" 2>&1 >/dev/null)" || RC=$?
[ "$RC" = 12 ] || { echo "    a private build below the disk floor returned $RC, expected 12: $ERR"; exit 1; }
[ "$(calls)" = 1 ] || { echo "    a private build started below the disk floor: $(cat "$CALLS")"; exit 1; }

# ---------------------------------------------------------------- a failure is shown, quiet or not
run fail 3 env -u CARGO_BUILD_BUILD_DIR
[ "$RC" = 101 ] || { echo "    cargo's own exit status was not returned: $RC"; exit 1; }
printf '%s\n' "$ERR" | grep -q 'error: could not compile x' || { echo "    a failing build hid its error: $ERR"; exit 1; }

# ---------------------------------------------------------------- the explicit build goes through it
# ADR 0224: the launcher never builds, so the build left to bound is the explicit one
grep -qE '^[^#]*mj_cargo_bounded "\$ROOT" build' "$ROOT/scripts/derive" \
  || { echo "    scripts/derive does not build through mj_cargo_bounded"; exit 1; }
if grep -nE '^[^#]*(mj_cargo_bounded|(^|[^_[:alnum:]])cargo )' "$ROOT/bin/majordomus-cli" | grep -v "printf"; then
  echo "    bin/majordomus-cli calls cargo; the launcher never builds"; exit 1
fi

# ---------------------------------------------------------------- the gate reads the shape
# scripts/liveness-check, in a fixture repository because it reads `git ls-files`: a direct
# call is a finding, a bounded one is not, a file whose `cargo` is the bounded function is
# not, and a sentence that names the command is not a call.
GATE="$ROOT/scripts/liveness-check"
F="$T/fixture"; mkdir -p "$F/scripts" "$F/.ai/repo"
( cd "$F" && git init -q . && git config user.email t@example.com && git config user.name t ) \
  || { echo "    could not build the fixture"; exit 1; }
cp "$GATE" "$F/scripts/liveness-check"
printf '# nothing here yet\n' > "$F/.ai/repo/liveness-baseline.txt"
printf '#!/usr/bin/env bash\nRUSTFLAGS= cargo build --quiet --locked\n' > "$F/scripts/direct"
printf '#!/usr/bin/env bash\nmj_cargo_bounded "$ROOT" build --quiet --locked\n' > "$F/scripts/bounded"
printf '#!/usr/bin/env bash\ncargo() { mj_cargo_bounded "$ROOT" "$@"; }\ncargo test --no-fail-fast\n' > "$F/scripts/shadow"
printf '#!/usr/bin/env bash\necho "run: cargo build --manifest-path x"\n# cargo build is what this used to call\nsay "the step (cargo build --locked) is gone"\n' > "$F/scripts/prose"
chmod +x "$F"/scripts/*
( cd "$F" && git add -A && git commit -qm fixture ) || { echo "    could not commit the fixture"; exit 1; }
out="$( cd "$F" && ./scripts/liveness-check 2>&1 )" && { echo "    the gate passed a direct cargo build:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$out" | grep -q 'scripts/direct:2:unbounded-build:' \
  || { echo "    the gate did not name the direct call: $out"; exit 1; }
for clean in bounded shadow prose; do
  if printf '%s\n' "$out" | grep -q "scripts/$clean:"; then
    echo "    the gate reported scripts/$clean, which calls cargo no unbounded way: $out"; exit 1
  fi
done

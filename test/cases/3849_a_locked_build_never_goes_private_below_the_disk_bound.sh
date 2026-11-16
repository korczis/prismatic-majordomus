# majordomus-covers: none
# A build behind another build's lock never falls back to a cold private build directory
# below the disk's high-water mark (I3839). On 2026-10-21 `scripts/rust-check --quality`
# timed out on the shared build directory's lock, went private, and grew a cold 5.3 GB
# directory while 32-35 GiB were free; with several builds waiting, every one of them does
# that at once. Below the mark (MAJORDOMUS_PRIVATE_BUILD_ABOVE_MB, by default the reclaim
# mark) the build waits on the shared lock again, bounded, and then refuses with 75, naming
# the free space and the mark. Above it, the fallback is what case 3002 proves.
#
# Driven with a stand-in cargo like case 3002, so it compiles nothing. "Low" is a mark
# higher than any disk; "high" is a mark of 1 MB.
. "$ROOT/test/lib.sh"

LIB="$ROOT/lib/rust_bin.sh"
FAKE="$T/fake"; mkdir -p "$FAKE"
CALLS="$T/calls.log"
cat > "$FAKE/cargo" <<'SH'
#!/bin/sh
[ "$1" = metadata ] && exit 1
printf 'dir=%s\n' "${CARGO_BUILD_BUILD_DIR:-shared}" >> "$FAKE_CALLS"
if [ -z "${CARGO_BUILD_BUILD_DIR:-}" ]; then
  printf '    Blocking waiting for file lock on build directory\n' >&2; exec sleep 300
fi
printf '   Compiling x v0.0.0\n' >&2; exit 0
SH
chmod +x "$FAKE/cargo"

# run <mark MB> -- mj_cargo_bounded behind a lock that never frees; sets RC, ERR
run() {
  : > "$CALLS"; RC=0
  ERR="$(env -u CARGO_BUILD_BUILD_DIR PATH="$FAKE:$PATH" FAKE_CALLS="$CALLS" CARGO_HOME="$T/cargo-home" \
        MAJORDOMUS_BUILD_LOCK_WAIT=1 MAJORDOMUS_BUILD_LOCK_WAIT_LOW_DISK=3 MAJORDOMUS_MIN_FREE_MB=0 \
        MAJORDOMUS_INCREMENTAL_IDLE_MIN=0 MAJORDOMUS_PRIVATE_BUILD_ABOVE_MB="$1" \
        sh -c '. "$1"; mj_cargo_bounded "$2" build --quiet --locked' sh "$LIB" "$T" 2>&1 >/dev/null)" || RC=$?
}

# ---------------------------------------------------------------- below the mark: wait, then refuse
start="$(date +%s)"
run 999999999
took=$(( $(date +%s) - start ))
[ "$RC" = 75 ] || { echo "    below the mark a locked build ended with $RC, not 75: $ERR"; exit 1; }
grep -q 'build-private' "$CALLS" && { echo "    below the mark a build went private: $(cat "$CALLS")"; exit 1; }
[ ! -d "$T/cargo-home/build-private" ] || { echo "    below the mark a private build directory was made"; exit 1; }
[ "$(grep -c '^dir=shared$' "$CALLS")" -ge 2 ] \
  || { echo "    below the mark the build did not wait on the shared lock again: $(cat "$CALLS")"; exit 1; }
printf '%s\n' "$ERR" | grep -q 'refusing a private build: [0-9]*MB free is under 999999999MB' \
  || { echo "    the refusal does not name the free space and the mark: $ERR"; exit 1; }
[ "$took" -le 60 ] || { echo "    a low-disk wait bounded at 3s took ${took}s"; exit 1; }

# ---------------------------------------------------------------- above the mark: the fallback as before
run 1
[ "$RC" = 0 ] || { echo "    above the mark a locked build did not finish privately: rc $RC: $ERR"; exit 1; }
sed -n 2p "$CALLS" | grep -q "^dir=$T/cargo-home/build-private/" \
  || { echo "    above the mark the second attempt was not private: $(cat "$CALLS")"; exit 1; }
echo "    below the disk mark a locked build waits and refuses; above it, it goes private as before"

# majordomus-covers: none
# majordomus-exclusive: it adds and removes cases in test/cases while the runner reads it
# A case's scratch dir ($TMPDIR/mj-test.*) is removed however the case ends: it passes, it
# fails, or the runner is interrupted while it runs (I3834). Orphaned ones were the machine's
# steady disk drain, 13 GiB on 2026-10-21, from runs a pre-push hook or a session had stopped.
# MJ_TEST_KEEP=1 is the one way to keep it, and the runner then says where it is.
. "$ROOT/test/lib.sh"

RUN="$ROOT/test/run.sh"
MARK="mj3834$$"
NAP=$(( 5000 + ($$ % 900) ))
TMP="$T/tmp"; mkdir -p "$TMP"

cat > "$ROOT/test/cases/zz_${MARK}_pass.sh" <<'INNER'
echo "passing on purpose"
INNER
cat > "$ROOT/test/cases/zz_${MARK}_fail.sh" <<'INNER'
echo "failing on purpose"; exit 1
INNER
cat > "$ROOT/test/cases/zz_${MARK}_nap.sh" <<INNER
echo "napping on purpose"
sleep $NAP
INNER
cleanup() {
  rm -f "$ROOT/test/cases/zz_${MARK}_"*.sh
  ps -eo pid,comm,args | awk -v n="$NAP" '$2 ~ /sleep$/ && $0 ~ ("sleep " n "$") {print $1}' \
    | while read -r p; do kill -9 "$p" 2>/dev/null || true; done
}
trap cleanup EXIT

left() { ls -d "$TMP"/mj-test.* 2>/dev/null | wc -l | tr -d ' '; }

# ---------------------------------------------------------------- a passing case
TMPDIR="$TMP" bash "$RUN" "zz_${MARK}_pass" >/dev/null 2>&1 \
  || { echo "    the passing fixture case did not pass"; exit 1; }
[ "$(left)" = 0 ] || { echo "    a passing case left $(left) scratch dir(s) in $TMP"; exit 1; }

# ---------------------------------------------------------------- a failing case
TMPDIR="$TMP" bash "$RUN" "zz_${MARK}_fail" >/dev/null 2>&1 \
  && { echo "    the failing fixture case was reported as a pass"; exit 1; }
[ "$(left)" = 0 ] || { echo "    a failing case left $(left) scratch dir(s) in $TMP"; exit 1; }

# ---------------------------------------------------------------- an interrupted run
# The runner is stopped while its case naps. Its trap stops the case's process group and
# removes the dir; before I3834 the dir stayed behind.
TMPDIR="$TMP" bash "$RUN" "zz_${MARK}_nap" >/dev/null 2>&1 &
runner=$!
i=0; while [ "$(left)" = 0 ] && [ "$i" -lt 60 ]; do i=$((i+1)); sleep 1; done
[ "$(left)" = 1 ] || { echo "    the napping case never made its scratch dir"; exit 1; }
kill -TERM "$runner"
i=0; while kill -0 "$runner" 2>/dev/null && [ "$i" -lt 30 ]; do i=$((i+1)); sleep 1; done
kill -0 "$runner" 2>/dev/null && { echo "    the runner outlived TERM by ${i}s"; exit 1; }
[ "$(left)" = 0 ] || { echo "    an interrupted run left $(left) scratch dir(s) in $TMP"; exit 1; }

# ---------------------------------------------------------------- unless it is kept
out="$(TMPDIR="$TMP" MJ_TEST_KEEP=1 bash "$RUN" "zz_${MARK}_pass" 2>&1)" \
  || { echo "    the passing fixture case did not pass with MJ_TEST_KEEP=1"; exit 1; }
[ "$(left)" = 1 ] || { echo "    MJ_TEST_KEEP=1 kept $(left) scratch dir(s), not one"; exit 1; }
kept="$(ls -d "$TMP"/mj-test.*)"
printf '%s\n' "$out" | grep -qF "kept $kept" \
  || { echo "    the runner kept $kept without saying where"; exit 1; }
echo "    a passing, a failing and an interrupted run leave nothing; MJ_TEST_KEEP=1 keeps one and names it"

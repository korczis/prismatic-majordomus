# majordomus-covers: none
# majordomus-exclusive: it adds and removes cases in test/cases while the runner reads it
# Every case test/run.sh runs is bounded. Before this, a case that wedged held the whole
# suite until CI's job timeout fired hours later, and a job killed by its own timeout is
# reported `cancelled` — which reads as infrastructure trouble rather than as the one case
# that stopped. The bound turns a hang into a verdict.
#
# Two things must hold, and the second is the one that was wrong in the first attempt: the
# bound fires, and it reaches the children. `set -m` has to be on in the runner when the
# case is spawned, so that the case leads a process group of its own; enabling it inside
# the subshell is too late, and a killed case then left its background server holding a
# port for the next case to fail on.
. "$ROOT/test/lib.sh"

RUN="$ROOT/test/run.sh"
MARK="mj123$$"
# a sleep duration no other run on this machine is using, so the count below measures
# this case's children and not somebody else's
NAP=$(( 4000 + ($$ % 900) ))

# ---------------------------------------------------------------- a case that hangs ends
# The case declares its own bound, so the assertion does not wait on the 2400s default.
cat > "$ROOT/test/cases/zz_${MARK}_hang.sh" <<INNER
# majordomus-timeout: 3
echo "hanging on purpose"
sleep $NAP &  # liveness-check: intentional - the child whose reaping this case proves
sleep $NAP
INNER
cleanup() {
  rm -f "$ROOT/test/cases/zz_${MARK}_hang.sh" "$ROOT/test/cases/zz_${MARK}_quick.sh"
  ps -eo pid,comm,args | awk -v n="$NAP" '$2 ~ /sleep$/ && $0 ~ ("sleep " n "$") {print $1}' \
    | while read -r p; do kill -9 "$p" 2>/dev/null || true; done
}
trap cleanup EXIT

t0="$(date +%s)"
out="$(bash "$RUN" "zz_${MARK}_hang" 2>&1)" && { echo "    a hanging case was reported as a pass"; exit 1; }
elapsed=$(( $(date +%s) - t0 ))

printf '%s\n' "$out" | grep -q 'TIMEOUT' \
  || { echo "    the runner did not report the case as TIMEOUT:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }

# The bound is 3s and the runner allows a grace period before it insists; anything under a
# minute proves the wait was bounded rather than left to CI's job timeout.
[ "$elapsed" -lt 60 ] \
  || { echo "    the bound did not fire: the case ran ${elapsed}s against a 3s bound"; exit 1; }

# ---------------------------------------------------------------- and takes its children
# The backgrounded sleep belongs to the case's process group. If it outlived the kill, the
# next case would inherit whatever it was holding.
# Bounded, not instant: the runner sends TERM, allows a grace period and only then insists,
# so the children go away within seconds rather than at once. A single check right after the
# verdict measures the grace period, not the leak.
naps() { ps -eo pid,comm,args 2>/dev/null | awk -v n="$NAP" '$2 ~ /sleep$/ && $0 ~ ("sleep " n "$") {c++} END {print c+0}'; }
i=0; while [ "$(naps)" != 0 ] && [ "$i" -lt 30 ]; do i=$((i+1)); sleep 1; done
alive="$(naps)"
[ "$alive" = 0 ] \
  || { echo "    $alive child(ren) of the terminated case were still running after ${i}s"; exit 1; }

# ---------------------------------------------------------------- a case that finishes is untouched
# The bound must not change the verdict of a case that ends on its own.
printf '# majordomus-timeout: 30\necho fine\n' > "$ROOT/test/cases/zz_${MARK}_quick.sh"
out="$(bash "$RUN" "zz_${MARK}_quick" 2>&1)" \
  || { echo "    a case that finishes was failed by the bound:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$out" | grep -q '^ok   ' \
  || { echo "    a passing case did not report ok: $out"; exit 1; }

echo "  a hanging case is bounded, reported TIMEOUT, and takes its children with it"

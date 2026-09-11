# Freshness is a dimension of its own, and every boundary of it is proven here.
#
# `exact | advanced | diverged | different_context` answers where a record's commit sits
# relative to HEAD. It is a true statement about git topology and it says nothing at all
# about age — a record is `advanced` on the day it is written and still `advanced` a month
# later. On 2026-09-05 that word carried a finished instruction into every new episode in
# this repository for six days (ADR 0041).
#
# So there is a second judgement beside it, in five words, decided against thresholds
# declared once in the policy. The clock is injected rather than waited on: the thresholds
# are measured in days and a suite that proved them by sleeping would take a week to run.
. "$ROOT/test/lib.sh"

"$MJ" init >/dev/null

# The subject under test is the library function, sourced into this shell so that the three
# variables it sets can be read. Calling it in a command substitution would run it in a
# subshell and lose them — the mistake that killed a briefing mid-sentence while this was
# being written, because under `set -u` an unbound variable is not a wrong answer but a
# dead process.
. "$ROOT/lib/common.sh"
MJ_POLICY_FILE=.ai/repo/policy.yaml
mj_load_policy || { echo "    the policy this case judges against does not parse"; exit 1; }

FRESH="$(mj_pol session.freshness.fresh_minutes)"
STALE="$(mj_pol session.freshness.stale_minutes)"
[ -n "$FRESH" ] && [ -n "$STALE" ] || {
  echo "    the policy declares no session.freshness thresholds, so nothing here is a test"; exit 1; }
[ "$STALE" -gt "$FRESH" ] || { echo "    stale_minutes ($STALE) is not beyond fresh_minutes ($FRESH)"; exit 1; }

# A fixed instant to judge against, and record timestamps computed backwards from it. Both
# halves come from the same clock, so the arithmetic below is the only thing under test.
NOW=2026-06-15T12:00:00Z
export MAJORDOMUS_NOW="$NOW"
NOW_EPOCH="$(mj_epoch "$NOW")"
ago() { date -u -r $(( NOW_EPOCH - $1 * 60 )) +%Y-%m-%dT%H:%M:%SZ 2>/dev/null \
     || date -u -d "@$(( NOW_EPOCH - $1 * 60 ))" +%Y-%m-%dT%H:%M:%SZ; }

# Asserts the state and, for the states a reader acts on, that a reason was given. A
# verdict with no reason is the thing this subsystem is being fixed for.
check() {
  local minutes="$1" want="$2" ts got
  ts="$(ago "$minutes")"
  mj_freshness "$ts" >/dev/null; got="$MJ_FRESH_STATE"
  [ "$got" = "$want" ] || {
    printf '    a record %s minute(s) old is %s, expected %s (ts %s, now %s)\n' \
      "$minutes" "$got" "$want" "$ts" "$NOW"; exit 1; }
  [ -n "$MJ_FRESH_REASON" ] || { printf '    %s at %s minute(s) came with no reason\n' "$got" "$minutes"; exit 1; }
}

# ---------------------------------------------------------------- the boundaries
# Each threshold is proven on both sides and exactly on it, because "at or beyond" and
# "beyond" are a different contract and prose cannot be trusted to say which one the code
# implements.
check 1                    fresh
check $(( FRESH - 1 ))     fresh
check "$FRESH"             aging
check $(( FRESH + 1 ))     aging
check $(( STALE - 1 ))     aging
check "$STALE"             stale
check $(( STALE + 1 ))     stale

# ---------------------------------------------------------------- the long tail
check $(( 6 * 24 * 60 ))   stale      # the 2026-09-05 outage, to the day
check $(( 30 * 24 * 60 ))  stale

# ---------------------------------------------------------------- the edges
# A timestamp that cannot be judged is reported as itself. `unknown` and `invalid` are
# distinct on purpose: a record with no timestamp is not thereby old, and one dated in the
# future is not merely unknown — it is evidence that something wrote it wrongly.
mj_freshness "" >/dev/null
[ "$MJ_FRESH_STATE" = unknown ] || { printf '    an absent timestamp is %s, expected unknown\n' "$MJ_FRESH_STATE"; exit 1; }
[ -n "$MJ_FRESH_REASON" ] || { echo "    an absent timestamp came with no reason"; exit 1; }

mj_freshness "not-a-timestamp" >/dev/null
[ "$MJ_FRESH_STATE" = invalid ] || { printf '    a corrupt timestamp is %s, expected invalid\n' "$MJ_FRESH_STATE"; exit 1; }

mj_freshness "$(ago -60)" >/dev/null
[ "$MJ_FRESH_STATE" = invalid ] || { printf '    a timestamp an hour in the future is %s, expected invalid\n' "$MJ_FRESH_STATE"; exit 1; }
case "$MJ_FRESH_REASON" in *future*) ;; *)
  printf '    a future timestamp did not say so: %s\n' "$MJ_FRESH_REASON"; exit 1 ;; esac

# ---------------------------------------------------------------- history, or not
# The two states a reader must refuse to present as current, and the three it may.
for s in stale invalid; do
  mj_freshness_is_history "$s" || { printf '    %s is not treated as history\n' "$s"; exit 1; }
done
for s in fresh aging unknown; do
  mj_freshness_is_history "$s" && { printf '    %s is treated as history; it is not\n' "$s"; exit 1; }
done

# ---------------------------------------------------------------- one place for the numbers
# The thresholds are policy. A second copy of either number anywhere in the shell tool is
# the drift the policy file exists to prevent, and it is cheap to refuse it here.
#
# The two human renderers are skipped by name. mj_age_human and mj_duration_human switch
# from hours to days at 2880 minutes, which is two days because that is where days start
# reading better than hours, and it is a coincidence rather than a copy that the policy
# currently calls a record stale at the same span. Change either and they part company;
# the scan then covers the new number with nothing excluded.
scan_literals() {
  awk -v a="$1" -v b="$2" '
    /^mj_(age|duration)_human\(\)/ { skip = 1 }
    skip && /^}/                    { skip = 0; next }
    skip                            { next }
    $0 ~ "[^0-9]" a "[^0-9]" || $0 ~ "[^0-9]" b "[^0-9]" { printf "%s:%s: %s\n", FILENAME, FNR, $0 }
  ' "$ROOT/lib/"*.sh
}
hits="$(scan_literals "$FRESH" "$STALE" | grep -v 'freshness' || true)"
[ -z "$hits" ] || {
  printf '    a freshness threshold (%s or %s) is written into lib/ as a literal:\n' "$FRESH" "$STALE"
  printf '%s\n' "$hits" | sed 's/^/    | /'; exit 1; }

# majordomus-covers: none
# The millisecond clock is digits under any locale's decimal separator.
#
# Bash formats EPOCHREALTIME with the locale's decimal separator: under LC_ALL=cs_CZ.UTF-8,
# bash 5.3 answers `1794507023,452598`. mj_ms (lib/common.sh) removed only a dot, so the
# comma survived into every caller's arithmetic, and `$(( $(mj_ms) - $2 ))` in
# lib/doctor.sh:94 died with "value too great for base (error token is "087")": doctor
# exited 1 on the algoglyph integrator's self-hosted runner (korczis/algoglyph CI run
# 43276351631, Majordomus v1.0.1), which had to pin LC_ALL=C.UTF-8 to get past it (I4591).
#
# What is proved, in a fresh bash each time, with lib/common.sh sourced:
#   - with EPOCHREALTIME given a comma separator (unset first, so bash drops its dynamic
#     meaning and the value is the one written), mj_ms prints digits only, the epoch
#     milliseconds of that value, and a doctor-shaped subtraction of two readings succeeds;
#   - the same holds with a dot separator, the form every C locale gives;
#   - under the first installed locale whose real EPOCHREALTIME carries a comma (cs_CZ,
#     de_DE, hr_HR, ...), the live clock is digits too; a host with none says so and the
#     two injected separators above still hold the fix.
. "$ROOT/test/lib.sh"

LIB="$ROOT/lib/common.sh"

# ms <EPOCHREALTIME value> -- mj_ms against that value; sets RC and OUT
ms() {
  RC=0
  OUT="$(bash -c '. "$1" >/dev/null 2>&1 || exit 97
    MJ_TIMING_CLOCK=epochrealtime; unset EPOCHREALTIME; EPOCHREALTIME="$2"
    a="$(mj_ms)"; printf "%s\n" "$a"; printf "%s\n" "$(( $(mj_ms) - a ))"' \
    bash "$LIB" "$1" 2>&1)" || RC=$?
}

injected() {
  ms "$1"
  [ "$RC" = 0 ] || {
    echo "    EPOCHREALTIME=$1: mj_ms or its arithmetic failed (exit $RC): $OUT"; exit 1; }
  got="$(printf '%s\n' "$OUT" | sed -n 1p)"
  diff="$(printf '%s\n' "$OUT" | sed -n 2p)"
  case "$got" in
    ''|*[!0-9]*) echo "    EPOCHREALTIME=$1: mj_ms printed '$got', not digits"; exit 1 ;;
  esac
  [ "$got" = "$2" ] || { echo "    EPOCHREALTIME=$1: mj_ms printed $got, expected $2"; exit 1; }
  [ "$diff" = 0 ] || {
    echo "    EPOCHREALTIME=$1: two readings of one instant differ by '$diff'"; exit 1; }
}

# ---------------------------------------------------------------- an injected separator
injected '1794507023,087598' 1794507023087
injected '1794507023.087598' 1794507023087

# ---------------------------------------------------------------- a real comma locale
comma=""
for l in cs_CZ.UTF-8 cs_CZ.utf8 de_DE.UTF-8 de_DE.utf8 hr_HR.UTF-8 hr_HR.utf8 \
  fr_FR.UTF-8 fr_FR.utf8; do
  v="$(LC_ALL="$l" bash -c 'printf "%s" "$EPOCHREALTIME"' 2>/dev/null)" || continue
  case "$v" in *,*) comma="$l"; break ;; esac
done
if [ -n "$comma" ]; then
  RC=0
  OUT="$(LC_ALL="$comma" bash -c '. "$1" >/dev/null 2>&1 || exit 97
    a="$(mj_ms)"; printf "%s\n" "$a"; printf "%s\n" "$(( $(mj_ms) - a ))"' bash "$LIB" 2>&1)" \
    || RC=$?
  [ "$RC" = 0 ] || {
    echo "    under LC_ALL=$comma mj_ms or its arithmetic failed (exit $RC): $OUT"; exit 1; }
  got="$(printf '%s\n' "$OUT" | sed -n 1p)"
  case "$got" in
    ''|*[!0-9]*) echo "    under LC_ALL=$comma mj_ms printed '$got', not digits"; exit 1 ;;
  esac
  echo "    mj_ms is digits under LC_ALL=$comma, whose EPOCHREALTIME carries a comma"
else
  echo "    no installed locale gives EPOCHREALTIME a comma; the injected separators hold the fix"
fi

echo "    mj_ms reads EPOCHREALTIME under any decimal separator"

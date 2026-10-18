# The machine sweeps its own build output, and a scheduler entry does it every fifteen minutes
# (I2881). On 2026-10-18 lundra reached 0 bytes free twice in one morning, 87 GB in its shared
# build directory, because every sweep this repository had ran only inside one of its own
# builds and a machine whose builds came from elsewhere never swept.
#
# Against a disposable cargo home: a private build directory idle for years is reclaimable and
# a check leaves it where it is; --reclaim removes it; a recent one stays through both. Against
# a disposable HOME: --install writes the scheduler entry this platform has (a launchd agent or
# a systemd --user service and timer) starting `majordomus machine sweep --reclaim`, status
# sees it, and --remove deletes it. --no-activate keeps launchctl and systemctl out of a test.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip_case "jq is absent, so the answers were not read"

R="$T/repo"
fixture_repo "$R" >/dev/null
mj() { ( cd "$R" && MAJORDOMUS_SHARE="$R/share" "$RB" "$@" ); }

C="$T/cargo"
mkdir -p "$C/build-private/idle/debug/deps" "$C/build-private/recent/debug/deps"
printf 'x' > "$C/build-private/idle/debug/deps/libold.rlib"
printf 'x' > "$C/build-private/recent/debug/deps/libnew.rlib"
find "$C/build-private/idle" -exec touch -t 202001010000 {} +

# --- a check: classified, nothing removed
out="$(mj machine sweep --cargo-home "$C" --format json)" || { echo "    the check failed: $out"; exit 1; }
verdict() { printf '%s' "$out" | jq -r --arg n "$1" '.items[] | select(.path | endswith("/" + $n)) | .verdict'; }
[ "$(verdict idle)" = reclaimable ] || { echo "    an idle build directory is not reclaimable: $out"; exit 1; }
[ "$(verdict recent)" = recent ] || { echo "    a recent build directory is not recent: $out"; exit 1; }
[ "$(printf '%s' "$out" | jq -r .check)" = true ] || { echo "    the sweep was not a check"; exit 1; }
[ -d "$C/build-private/idle" ] || { echo "    a check removed a directory"; exit 1; }

# --- --reclaim: the idle one goes, the recent one stays
out="$(mj machine sweep --cargo-home "$C" --reclaim --format json)" || { echo "    the sweep failed: $out"; exit 1; }
[ ! -e "$C/build-private/idle" ] || { echo "    --reclaim left the idle build directory"; exit 1; }
[ -d "$C/build-private/recent" ] || { echo "    --reclaim removed a recent build directory"; exit 1; }
[ "$(printf '%s' "$out" | jq -r '.reclaimed_bytes > 0')" = true ] || { echo "    nothing was counted as reclaimed: $out"; exit 1; }
expect_grep 'reclaimable' <(mj machine sweep --cargo-home "$C" --idle-minutes 0)

# --- the schedule, in a disposable HOME
H="$T/home"; mkdir -p "$H"
sched() { ( cd "$R" && HOME="$H" MAJORDOMUS_SHARE="$R/share" "$RB" machine schedule "$@" --program /opt/majordomus/bin/majordomus --no-activate --format json ); }
s="$(sched --install)" || { echo "    the install failed: $s"; exit 1; }
case "$(printf '%s' "$s" | jq -r .scheduler)" in
  launchd|systemd) ;;
  *) skip_case "this platform has neither launchd nor systemd, so the schedule was not installed" ;;
esac
[ "$(printf '%s' "$s" | jq -r .installed)" = true ] || { echo "    the entry was not installed: $s"; exit 1; }
f="$(printf '%s' "$s" | jq -r '.files[0]')"
case "$f" in "$H"/*) ;; *) echo "    the entry was written outside HOME: $f"; exit 1 ;; esac
expect_grep 'machine' "$f"
expect_grep 'reclaim' "$f"
expect_grep '/opt/majordomus/bin/majordomus' "$f"
[ "$(sched | jq -r .installed)" = true ] || { echo "    status does not see the entry"; exit 1; }
[ "$(printf '%s' "$s" | jq -r '.activation | length')" = 0 ] || { echo "    --no-activate ran a scheduler command"; exit 1; }
gone="$(sched --remove)" || { echo "    the removal failed: $gone"; exit 1; }
[ "$(printf '%s' "$gone" | jq -r .installed)" = false ] || { echo "    --remove left the entry"; exit 1; }
[ ! -e "$f" ] || { echo "    --remove left $f"; exit 1; }
exit 0

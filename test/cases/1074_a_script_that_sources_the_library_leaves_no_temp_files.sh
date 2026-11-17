# majordomus-covers: none
# A script that sources lib/common.sh leaves nothing in TMPDIR. Its temporary files live
# under a root of the process's own, which the exit trap removes, and a root whose process
# is gone is removed by the next process that sources the library. Before this, only the
# entry point asked for a root, and the derive's generators, the site build and its checks
# and the cases each left their flattenings in TMPDIR, until on 2026-10-11 it held 65,530
# entries and every `mktemp -d` on the machine failed.
#
#   a script that sources the library   its files are under its own root, and gone at exit
#   a script killed while running       its root is removed by the next script that starts
#   the library sourced twice           one root, the one the process already had
#   a subshell sourcing it again        the parent's root outlives the subshell
#   the entry point's deferral          MJ_TMP_ROOT_DEFER makes no root as the library loads
. "$ROOT/test/lib.sh"

tmp="$T.tmpdir"
mkdir -p "$tmp"
trap 'rm -rf "$tmp"' EXIT
left() { ls -A "$tmp" | sed 's/^/    | /'; }

cat > sourcing.sh <<'S'
. "$LIB/common.sh"
f="$(mktemp "${TMPDIR:-/tmp}/mj.pj.XXXXXX")"
d="$(mktemp -d "${TMPDIR:-/tmp}/mj.man.XXXXXX")"
printf '%s\n%s\n' "$f" "$d"
S

# ---------------------------------------------------------------- a script that sources it
out="$(TMPDIR="$tmp" LIB="$ROOT/lib" bash -eu sourcing.sh)"
case "$(printf '%s\n' "$out" | head -n 1)" in
  "$tmp"/mj.[0-9]*.*/mj.pj.*) ;;
  *) echo "    the file was not made under the process's own root: $out"; exit 1 ;;
esac
[ -z "$(ls -A "$tmp")" ] || { echo "    a script that sourced the library left this in TMPDIR:"; left; exit 1; }

# ---------------------------------------------------------------- a script killed while running
printf '. "$LIB/common.sh"\nmktemp "${TMPDIR:-/tmp}/mj.ksrc.XXXXXX" >/dev/null\nexec sleep 30\n' > killed.sh
TMPDIR="$tmp" LIB="$ROOT/lib" bash -eu killed.sh &
pid=$!
i=0
until ls -d "$tmp"/mj."$pid".* >/dev/null 2>&1 || [ "$i" -ge 50 ]; do sleep 0.1; i=$((i + 1)); done
kill -9 "$pid" 2>/dev/null; wait "$pid" 2>/dev/null || true
ls -d "$tmp"/mj."$pid".* >/dev/null 2>&1 \
  || { echo "    the killed script's root was never made, so this scenario proves nothing"; exit 1; }
TMPDIR="$tmp" LIB="$ROOT/lib" bash -eu sourcing.sh >/dev/null
[ -z "$(ls -A "$tmp")" ] \
  || { echo "    a killed script's root outlived it past the next script that started:"; left; exit 1; }

# ---------------------------------------------------------------- the library sourced twice
printf '. "$LIB/common.sh"\na="$MJ_TMP_ROOT"\n. "$LIB/common.sh"\n[ "$a" = "$MJ_TMP_ROOT" ] && [ -d "$a" ] && ls -d "${TMPDIR:-/tmp}"/mj.$$.* | wc -l\n' > twice.sh
n="$(TMPDIR="$tmp" LIB="$ROOT/lib" bash -eu twice.sh | tr -d ' ')"
[ "$n" = 1 ] || { echo "    sourcing the library twice made $n roots, or lost the first"; exit 1; }
[ -z "$(ls -A "$tmp")" ] || { echo "    sourcing the library twice left this:"; left; exit 1; }

# ---------------------------------------------------------------- a subshell that sources it again
# the subshell sets the exit trap for itself; the root it inherited is still its parent's,
# which `usecase run` was writing its steps into when a setup's subshell removed it
printf '. "$LIB/common.sh"\nr="$MJ_TMP_ROOT"\n( . "$LIB/common.sh"; mktemp "${TMPDIR:-/tmp}/mj.sub.XXXXXX" >/dev/null )\n[ -d "$r" ] && echo kept\n' > subshell.sh
k="$(TMPDIR="$tmp" LIB="$ROOT/lib" bash -eu subshell.sh)"
[ "$k" = kept ] || { echo "    a subshell sourcing the library removed its parent's root"; exit 1; }
[ -z "$(ls -A "$tmp")" ] || { echo "    the subshell's sourcing left this:"; left; exit 1; }

# ---------------------------------------------------------------- the entry point's deferral
printf 'MJ_TMP_ROOT_DEFER=1\n. "$LIB/common.sh"\nprintf "%%s" "$MJ_TMP_ROOT"\n' > deferred.sh
r="$(TMPDIR="$tmp" LIB="$ROOT/lib" bash -eu deferred.sh)"
[ -z "$r" ] || { echo "    a deferring entry point got a root as the library loaded: $r"; exit 1; }
[ -z "$(ls -A "$tmp")" ] || { echo "    the deferral left this:"; left; exit 1; }

echo "    a script that sources the library leaves nothing in TMPDIR, even when killed"

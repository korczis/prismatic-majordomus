# majordomus-covers: none
# A script that sources lib/common.sh leaves none of the library's own files in TMPDIR. The
# library's flattenings (the manifest's, the knowledge sources', the project model's) live in
# a directory of the process's own, which the exit trap removes, and a directory whose
# process is gone is removed by the next process that sources the library. Before this,
# every script that sources the library — the derive's generators, the site build and its
# checks, the cases — left those flattenings in TMPDIR, until on 2026-10-11 it held 65,530
# entries and every `mktemp -d` on the machine failed. A script's own `mktemp` is not moved:
# a file it makes may be its answer to its caller (case 991).
#
#   a script that sources the library   the library's files are gone at exit, the script's stay
#   a script killed while running       its directory is removed by the next script that starts
#   the library sourced twice           one directory, the one the process already had
#   a subshell sourcing it again        the parent's directory outlives the subshell
#   the entry point's deferral          MJ_TMP_ROOT_DEFER makes no directory as the library loads
. "$ROOT/test/lib.sh"

tmp="$T.tmpdir"
mkdir -p "$tmp"
trap 'rm -rf "$tmp"' EXIT
left() { ls -A "$tmp" | sed 's/^/    | /'; }

cat > sourcing.sh <<'S'
. "$LIB/common.sh"
f="$(mktemp "$(mj_lib_tmp)/mj.pj.XXXXXX")"
d="$(mktemp -d "$(mj_lib_tmp)/mj.man.XXXXXX")"
own="$(mktemp "${TMPDIR:-/tmp}/plain.XXXXXX")"
printf '%s\n%s\n%s\n' "$f" "$d" "$own"
S

# ---------------------------------------------------------------- a script that sources it
out="$(TMPDIR="$tmp" LIB="$ROOT/lib" bash -eu sourcing.sh)"
case "$(printf '%s\n' "$out" | head -n 1)" in
  "$tmp"/mj.[0-9]*.*/mj.pj.*) ;;
  *) echo "    the library's file was not made under the process's own directory: $out"; exit 1 ;;
esac
own="$(printf '%s\n' "$out" | sed -n 3p)"
case "$own" in "$tmp"/plain.*) ;; *) echo "    the script's own mktemp was moved: $own"; exit 1 ;; esac
[ -f "$own" ] || { echo "    the script's own file is not the library's to remove: $own"; exit 1; }
rm -f "$own"
[ -z "$(ls -A "$tmp")" ] || { echo "    a script that sourced the library left this in TMPDIR:"; left; exit 1; }

# ---------------------------------------------------------------- a script killed while running
printf '. "$LIB/common.sh"\nmktemp "$(mj_lib_tmp)/mj.ksrc.XXXXXX" >/dev/null\nexec sleep 30\n' > killed.sh
TMPDIR="$tmp" LIB="$ROOT/lib" bash -eu killed.sh &
pid=$!
i=0
until ls -d "$tmp"/mj."$pid".* >/dev/null 2>&1 || [ "$i" -ge 50 ]; do sleep 0.1; i=$((i + 1)); done
kill -9 "$pid" 2>/dev/null; wait "$pid" 2>/dev/null || true
ls -d "$tmp"/mj."$pid".* >/dev/null 2>&1 \
  || { echo "    the killed script's directory was never made, so this scenario proves nothing"; exit 1; }
printf '. "$LIB/common.sh"\n' > quiet.sh
TMPDIR="$tmp" LIB="$ROOT/lib" bash -eu quiet.sh
[ -z "$(ls -A "$tmp")" ] \
  || { echo "    a killed script's directory outlived it past the next script that started:"; left; exit 1; }

# ---------------------------------------------------------------- the library sourced twice
printf '. "$LIB/common.sh"\na="$MJ_LIB_TMP"\n. "$LIB/common.sh"\n[ "$a" = "$MJ_LIB_TMP" ] && [ -d "$a" ] && ls -d "${TMPDIR:-/tmp}"/mj.$$.* | wc -l\n' > twice.sh
n="$(TMPDIR="$tmp" LIB="$ROOT/lib" bash -eu twice.sh | tr -d ' ')"
[ "$n" = 1 ] || { echo "    sourcing the library twice made $n directories, or lost the first"; exit 1; }
[ -z "$(ls -A "$tmp")" ] || { echo "    sourcing the library twice left this:"; left; exit 1; }

# ---------------------------------------------------------------- a subshell that sources it again
# the subshell sets the exit trap for itself; the directory it inherited is still its
# parent's, which `usecase run` was writing into when a setup's subshell removed it
printf '. "$LIB/common.sh"\nr="$MJ_LIB_TMP"\n( . "$LIB/common.sh"; mktemp "$(mj_lib_tmp)/mj.sub.XXXXXX" >/dev/null )\n[ -d "$r" ] && echo kept\n' > subshell.sh
k="$(TMPDIR="$tmp" LIB="$ROOT/lib" bash -eu subshell.sh)"
[ "$k" = kept ] || { echo "    a subshell sourcing the library removed its parent's directory"; exit 1; }
[ -z "$(ls -A "$tmp")" ] || { echo "    the subshell's sourcing left this:"; left; exit 1; }

# ---------------------------------------------------------------- the entry point's deferral
printf 'MJ_TMP_ROOT_DEFER=1\n. "$LIB/common.sh"\nprintf "%%s" "$MJ_LIB_TMP$MJ_TMP_ROOT"\n' > deferred.sh
r="$(TMPDIR="$tmp" LIB="$ROOT/lib" bash -eu deferred.sh)"
[ -z "$r" ] || { echo "    a deferring entry point got a directory as the library loaded: $r"; exit 1; }
[ -z "$(ls -A "$tmp")" ] || { echo "    the deferral left this:"; left; exit 1; }

echo "    a script that sources the library leaves none of its files in TMPDIR, even when killed"

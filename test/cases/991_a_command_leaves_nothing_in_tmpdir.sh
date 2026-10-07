# majordomus-covers: none
# A command leaves nothing behind in TMPDIR.
#
# Every temporary file the shell tool makes lives under one directory of the process's own,
# and the exit trap removes it. Before that, `mj_cleanup` removed the four files it knew by
# name and some hundred and fifty `mktemp` call sites made others; one session start left 25
# entries and 1335 files, a provider hook runs the tool on every prompt, and on 2026-10-07
# macOS spent 7 min 38 s of a login deleting the pile, because it empties TMPDIR at boot
# before anything else may start.
#
# What is held here: a command that succeeds, one that is refused, one that dies of a usage
# error and one that is terminated each leave TMPDIR as they found it; the process a command
# starts still sees the caller's TMPDIR; and a script that sources the library without asking
# for a root gets `mktemp` as it was.
. "$ROOT/test/lib.sh"

# The probe is the TMPDIR every command below is given, and it is outside the fixture so
# that nothing in it is a file of the repository under test. With a trailing slash, as
# macOS hands it out: a template built as "$TMPDIR/name" then carries two.
PROBE="$(mktemp -d "${TMPDIR:-/tmp}/mj-tmp-probe.XXXXXX")"
trap 'rm -rf "$PROBE" "$PROBE.child"' EXIT
left() { find "$PROBE" -mindepth 1 | wc -l | tr -d ' '; }
# run a command under the probe; its status is the caller's to judge
probed() { TMPDIR="$PROBE/" "$@"; }
clean() {
  [ "$(left)" = 0 ] && return 0
  printf '    %s left %s entr(ies) in TMPDIR:\n' "$1" "$(left)"
  find "$PROBE" -mindepth 1 | sed "s|^$PROBE/|      |" | head -20
  exit 1
}

probed "$MJ" init >/dev/null;   clean init
probed "$MJ" update >/dev/null; clean update
# what a verify command inherits is written outside the fixture, where it is no file of the task
SEEN="$PROBE.child"
mkdir -p lib && echo a > lib/a
printf 'printf %%s "$TMPDIR" > "$MJ_CASE_SEEN"\ntest -s lib/a\n' > lib/verify.sh
git add . && git commit -qm base

# a command that succeeds, across the loaders that used to leak: the manifest, the event
# registry, the current task, the policy, the profile and the context documents
probed "$MJ" context >/dev/null;                 clean context
probed "$MJ" start "t1" --scope lib >/dev/null;  clean start
probed "$MJ" context >/dev/null;                 clean "context with a task"
probed "$MJ" check >/dev/null || true;           clean check
probed "$MJ" doctor >/dev/null 2>&1 || true;     clean doctor
printf 'progress\n' | probed "$MJ" checkpoint >/dev/null 2>&1 || true; clean checkpoint

# one that is refused, and one that dies of a usage error: the trap runs on every exit
expect_exit 10 probed "$MJ" finish --outcome completed; clean "a refused finish"
expect_exit 2  probed "$MJ" finish --outcome nonsense;  clean "a usage error"

# the process a command starts is handed the caller's TMPDIR, not the root: it may outlive
# the command, and a root is removed when the command ends
echo b >> lib/a
printf '# Objective\no\n# Current State\ns\n# Next Action\nn\n' | probed "$MJ" handover >/dev/null 2>&1 || true
clean handover
MJ_CASE_SEEN="$SEEN" probed "$MJ" finish --outcome completed --verify-command 'sh lib/verify.sh' >/dev/null 2>&1 || true
clean "finish with a verify command"
[ -f "$SEEN" ] || { echo "    the verify command did not run, so what it inherits was not observed"; exit 1; }
[ "$(cat "$SEEN")" = "$PROBE/" ] \
  || { echo "    a verify command saw TMPDIR=$(cat "$SEEN"), not the caller's $PROBE/"; exit 1; }

# one that is terminated. `doctor` is long enough to be caught mid-run; whether the signal
# lands before or after it finishes, nothing may remain
# (started without `probed`: a function in the background is a subshell, and its pid is
# not the tool's)
for after in 0.1 0.3 0.6 1; do
  TMPDIR="$PROBE/" "$MJ" doctor >/dev/null 2>&1 & pid=$!
  sleep "$after"; kill -TERM "$pid" 2>/dev/null || true
  st=0; wait "$pid" 2>/dev/null || st=$?
  clean "a doctor terminated after ${after}s (status $st)"
done

# a script that sources the library and never asks for a root: `mktemp` answers where it
# was asked, and a template that is not under TMPDIR is never moved
out="$(TMPDIR="$PROBE/" MJ_BIN_DIR="$ROOT/bin" MJ_LIB_DIR="$ROOT/lib" MJ_VERSION=0 bash -c '
  set -eu; . "$MJ_LIB_DIR/common.sh"
  a="$(mktemp "${TMPDIR:-/tmp}/plain.XXXXXX")"; printf "%s\n" "$a"
  mj_tmp_root_init
  b="$(mktemp "${TMPDIR:-/tmp}/rooted.XXXXXX")"; printf "%s\n" "$b"
  c="$(mktemp "$PWD/beside.XXXXXX")"; printf "%s\n" "$c"
  d="$(mktemp -d "${TMPDIR:-/tmp}/dir.XXXXXX")"; e="$(mktemp "$d/inner.XXXXXX")"; printf "%s\n" "$e"
  printf "%s\n" "$MJ_TMP_ROOT"
')"
plain="$(printf '%s\n' "$out" | sed -n 1p)"; rooted="$(printf '%s\n' "$out" | sed -n 2p)"
beside="$(printf '%s\n' "$out" | sed -n 3p)"; inner="$(printf '%s\n' "$out" | sed -n 4p)"
root="$(printf '%s\n' "$out" | sed -n 5p)"
case "$plain" in "$PROBE"//plain.*|"$PROBE"/plain.*) ;; *) echo "    without a root, mktemp answered $plain"; exit 1 ;; esac
case "$rooted" in "$root"/*rooted.*) ;; *) echo "    with a root, mktemp answered $rooted, outside $root"; exit 1 ;; esac
case "$beside" in "$PWD"/beside.*) ;; *) echo "    a template outside TMPDIR was moved to $beside"; exit 1 ;; esac
case "$inner" in "$root"/*dir.*/inner.*) ;; *) echo "    a template already under the root was moved to $inner"; exit 1 ;; esac
[ -f "$plain" ] || { echo "    the file made without a root is not this script's to lose: $plain"; exit 1; }
[ ! -e "$root" ] || { echo "    the root $root outlived the process that made it"; exit 1; }
rm -f "$plain" "$beside"
clean "a sourcing script"

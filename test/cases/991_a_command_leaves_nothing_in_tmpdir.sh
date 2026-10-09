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
# error each leave TMPDIR as they found it; one that is terminated, killed or whose reader
# closed the pipe leaves at most a root that a following command removes; the process a command
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

# A shell that ends on a signal does not take what it was waiting for with it. The awk, the
# sort or the subshell it had started runs on as an orphan and goes on writing under the
# root, for as long as that work takes, so "nothing is left the moment the shell is gone"
# is a claim about scheduling and not about the tool: on a loaded runner a doctor
# terminated after one second had its root back, four entries deep, when this case looked
# (PR #800, suite shard 1). What the tool does promise is that a root whose process is gone
# is removed by a command that starts after its last writer has stopped. So the question
# is asked the way the promise is made: a command is run, and again, until TMPDIR is empty
# or two minutes have gone, and only then is what remains a failure.
settled() {
  local tries=0
  while :; do
    probed "$MJ" context >/dev/null
    [ "$(left)" = 0 ] && return 0
    tries=$((tries + 1)); [ "$tries" -ge 40 ] && break
    sleep 3
  done
  clean "$1"
}

# one that is terminated. `doctor` is long enough to be caught mid-run, wherever in it the
# signal lands (started without `probed`: a function in the background is a subshell, and
# its pid is not the tool's)
for after in 0.1 0.3 0.6 1; do
  TMPDIR="$PROBE/" "$MJ" doctor >/dev/null 2>&1 & pid=$!
  sleep "$after"; kill -TERM "$pid" 2>/dev/null || true
  st=0; wait "$pid" 2>/dev/null || st=$?
  settled "what follows a doctor terminated after ${after}s (status $st)"
done

# one whose reader left, and one that was killed: neither runs an exit trap at all, so the
# root stays whole until the next command
TMPDIR="$PROBE/" "$MJ" doctor 2>&1 | grep -q . || true
settled "what follows a doctor whose reader left"
TMPDIR="$PROBE/" "$MJ" doctor >/dev/null 2>&1 & pid=$!
sleep 0.3; kill -KILL "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true
[ "$(find "$PROBE" -mindepth 1 -maxdepth 1 -name "mj.$pid.*" | wc -l | tr -d ' ')" = 1 ] \
  || { echo "    a killed doctor left no root named for its process, so the sweep below proves nothing"; exit 1; }
settled "what follows a killed doctor"
# a root whose process is alive is not touched: this shell's own id stands in for one
mkdir "$PROBE/mj.$$.living"
probed "$MJ" context >/dev/null
[ -d "$PROBE/mj.$$.living" ] || { echo "    a root whose process is alive was removed"; exit 1; }
rmdir "$PROBE/mj.$$.living"

# a script that sources the library and never asks for a root: `mktemp` answers where it
# was asked, and a template that is not under TMPDIR is never moved
out="$(TMPDIR="$PROBE/" MJ_BIN_DIR="$ROOT/bin" MJ_LIB_DIR="$ROOT/lib" MJ_VERSION=0 bash -c '
  set -eu; . "$MJ_LIB_DIR/common.sh"
  a="$(mktemp "${TMPDIR:-/tmp}/plain.XXXXXX")"; printf "%s\n" "$a"
  mj_tmp_root_init
  b="$(mktemp "${TMPDIR:-/tmp}/rooted.XXXXXX")"; printf "%s\n" "$b"
  c="$(mktemp "$PWD/beside.XXXXXX")"; printf "%s\n" "$c"
  d="$(mktemp -d "${TMPDIR:-/tmp}/dir.XXXXXX")"; e="$(mktemp "$d/inner.XXXXXX")"; printf "%s\n" "$e"
  mkdir -p "${TMPDIR%/}/a-repository/records"
  f="$(mktemp "${TMPDIR%/}/a-repository/records/.tmp.XXXXXX")"; printf "%s\n" "$f"
  printf "%s\n" "$MJ_TMP_ROOT"
')"
plain="$(printf '%s\n' "$out" | sed -n 1p)"; rooted="$(printf '%s\n' "$out" | sed -n 2p)"
beside="$(printf '%s\n' "$out" | sed -n 3p)"; inner="$(printf '%s\n' "$out" | sed -n 4p)"
below="$(printf '%s\n' "$out" | sed -n 5p)"; root="$(printf '%s\n' "$out" | sed -n 6p)"
case "$plain" in "$PROBE"//plain.*|"$PROBE"/plain.*) ;; *) echo "    without a root, mktemp answered $plain"; exit 1 ;; esac
case "$rooted" in "$root"/*rooted.*) ;; *) echo "    with a root, mktemp answered $rooted, outside $root"; exit 1 ;; esac
case "$beside" in "$PWD"/beside.*) ;; *) echo "    a template outside TMPDIR was moved to $beside"; exit 1 ;; esac
case "$inner" in "$root"/*dir.*/inner.*) ;; *) echo "    a template already under the root was moved to $inner"; exit 1 ;; esac
# a repository that lives below TMPDIR — every fixture does, where TMPDIR is unset and the
# fixture is made in /tmp — keeps the temporary file it asked for beside its own record
case "$below" in "$PROBE"/a-repository/records/.tmp.*) ;; *) echo "    a template below TMPDIR, not in it, was moved to $below"; exit 1 ;; esac
[ -f "$plain" ] || { echo "    the file made without a root is not this script's to lose: $plain"; exit 1; }
[ ! -e "$root" ] || { echo "    the root $root outlived the process that made it"; exit 1; }
rm -f "$plain" "$beside"; rm -rf "$PROBE/a-repository"
clean "a sourcing script"

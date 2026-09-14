# majordomus-covers: none
# The reaper fails in exactly the situation it exists for: three faults measured on
# 2026-09-13, on a machine that had been at 98% used since the morning.
#
#   $ scripts/reap-orphans --reclaim
#   PID      AGE          RSS       PORTS  STATE
#   79500    01:21:25     17MB      56042  would reap
#
#   exit 141      (nothing reclaimed; disk unchanged)
#
# 1. EXIT 141 IS SIGPIPE, and the reader that caused it was the reaper's own. The primary
#    checkout is excluded from the sweep by name, and its name was read with
#    `git worktree list --porcelain | awk 'NR == 1 ... { print; exit }'`. That `exit` closes
#    the pipe on record one. This repository has 143 worktrees and 24KB of porcelain, git
#    writes it a line at a time, and the kernel's pipe buffer is smaller than that — so git
#    took SIGPIPE on every single run, `pipefail` turned the assignment into 141, and `set -e`
#    killed the sweep between the server table and the worktree table. It had been dying
#    there for as long as the machine had had that many worktrees.
#
#    The repair is the reader, not a trap. A reaper that swallowed SIGPIPE and carried on
#    would be worse than one that dies loudly, and `|| true` would have hidden a *real* git
#    failure behind an empty `main_wt` — which is not survivable, because a primary checkout
#    whose name could not be read is a primary checkout the sweep cannot exclude, and it
#    holds 34GB of build output that a predicate would happily reclaim on a quiet afternoon.
#
# 2. `--reclaim` TABLED A SUBJECT IT DOES NOT ACT ON. 79500 on port 56042 was the checkout's
#    own shared server, printed under the word "would reap" by a run whose consent was for
#    build output. The two consents are separate exactly so that a caller acting on one never
#    acts on the other's subject (ADR 0055), and a run that reports the other subject hands
#    the reader the belief the separation exists to prevent: that `--reclaim` is about to end
#    their server, or that it already has.
#
# 3. A PARTIAL SWEEP READ AS A VERDICT. What that run printed was a column header and one row
#    — the visual shape of an answer. The only thing saying otherwise was an exit status
#    nobody reads off a terminal. A run that stops before its summary must say so in words and
#    name the subject it never reached (project.a-verdict-states-its-subject).
#
# What this case proves, and how it avoids proving it about a fixture too small to fail: the
# SIGPIPE needs a producer bigger than the pipe buffer, so a `git` shim pads the porcelain
# with synthetic records to ~200KB and writes them a line at a time. The reaper skips them
# (nothing exists at those paths), so they change no verdict — they only make the stream long
# enough that a reader which abandons it takes the signal. Every assertion here is then
# mutation-tested against a copy of the real script, and each mutation is checked to have
# actually applied before its verdict is believed: a patch that silently no-ops plus a passing
# test makes the suite the lying instrument.
. "$ROOT/test/lib.sh"

REAPER="$ROOT/scripts/reap-orphans"
expect_file "$REAPER"

# ---------------------------------------------------------------- a fixture repository
FIX="$T/fix"
WTS="$T/fix-wt"
mkdir -p "$FIX" "$WTS"
(
  cd "$FIX"
  git init -q .
  git config user.email t@example.com
  git config user.name Test
  mkdir -p apps/majordomus-cli/src
  echo 'fn main(){}' > apps/majordomus-cli/src/main.rs
  echo 'target' > .gitignore
  git add -A && git commit -qm base
  git update-ref refs/remotes/origin/master HEAD
) || { echo "    the fixture repository could not be built"; exit 2; }

mkbuild() {
  mkdir -p "$1/apps/majordomus-cli/target/debug"
  printf 'Signature: 8a477f597d28d172789f06886806bc55\n# cargo cache directory tag\n' \
    > "$1/apps/majordomus-cli/target/CACHEDIR.TAG"
  echo build-output > "$1/apps/majordomus-cli/target/MARKER"
}

git -C "$FIX" worktree add -q -b idle "$WTS/idle" >/dev/null 2>&1 \
  || { echo "    could not add the idle worktree"; exit 2; }
mkbuild "$WTS/idle"
# The primary checkout gets build output too, so that the name-based exclusion is something
# the report has to say out loud. Reading that name is the pipeline that was taking SIGPIPE:
# without a build directory here the sweep would pass over the primary in silence and a
# broken reading of it would look exactly like a working one.
mkbuild "$FIX"

# ------------------------------------------------- a porcelain larger than the pipe buffer
# A `git` that is git in every respect but one: `worktree list --porcelain` is followed by
# synthetic records, printed a line at a time as git prints its own. Nothing exists at those
# paths, so the sweep passes over them without a verdict; what they change is the length of
# the stream, which is the whole of the condition under test. The subcommand is found
# positionally so that a `--porcelain` belonging to `git status` is never mistaken for this.
REALGIT="$(command -v git)"
[ -n "$REALGIT" ] || { echo "    no git on PATH"; exit 2; }
mkdir -p "$T/bin"
cat > "$T/bin/git" <<SHIM
#!/bin/sh
sub=""
for a in "\$@"; do
  case "\$a" in -C|--git-dir|--work-tree) skip=1; continue ;; esac
  if [ "\${skip:-0}" = 1 ]; then skip=0; continue; fi
  case "\$a" in -*) continue ;; esac
  sub="\$a"; break
done
PAD="\${MJ_PORCELAIN_PAD:-0}"
if [ "\$sub" = worktree ] && [ "\$PAD" -gt 0 ]; then
  case " \$* " in
    *" --porcelain "*)
      "$REALGIT" "\$@" || exit \$?
      i=0
      while [ \$i -lt "\$PAD" ]; do
        i=\$((i + 1))
        printf 'worktree %s/nowhere/pad-%s\n' "$T" "\$i"
        printf 'HEAD 0000000000000000000000000000000000000000\n'
        printf 'branch refs/heads/pad-%s\n\n' "\$i"
      done
      exit 0 ;;
  esac
fi
exec "$REALGIT" "\$@"
SHIM
chmod +x "$T/bin/git"

# the shim is git, and with padding asked for it is a much longer git
plain_bytes="$(PATH="$T/bin:$PATH" git -C "$FIX" worktree list --porcelain | wc -c | tr -d ' ')"
pad_bytes="$(PATH="$T/bin:$PATH" MJ_PORCELAIN_PAD=2000 git -C "$FIX" worktree list --porcelain | wc -c | tr -d ' ')"
[ "$plain_bytes" -gt 0 ] || { echo "    the git shim does not delegate: an unpadded porcelain came back empty"; exit 2; }
[ "$pad_bytes" -gt 131072 ] || {
  echo "    the padded porcelain is only $pad_bytes bytes, which no pipe buffer is smaller than;"
  echo "    this case would pass without measuring anything"; exit 2; }

# and the padding does not invent a worktree the sweep has a verdict about
PATH="$T/bin:$PATH" MJ_PORCELAIN_PAD=2000 MJ_ROOT="$FIX" "$REAPER" --targets > "$T/big.txt" 2>&1 || true
expect_no_grep 'pad-[0-9]' "$T/big.txt"

# ---------------------------------------------------------------- 1. it survives the stream
rc=0
PATH="$T/bin:$PATH" MJ_PORCELAIN_PAD=2000 MJ_ROOT="$FIX" "$REAPER" --targets > "$T/big.txt" 2>&1 || rc=$?
LAST_OUT="$(cat "$T/big.txt")"
[ "$rc" != 141 ] || {
  echo "    the sweep died of SIGPIPE (141) reading a $pad_bytes-byte worktree listing:"
  sed 's/^/      /' "$T/big.txt"; exit 1; }
[ "$rc" = 10 ] || { echo "    the sweep over a long listing exited $rc, not 10:"; sed 's/^/      /' "$T/big.txt"; exit 1; }
# it did not merely survive: it reached the end and classified
expect_grep 'idle.*would reclaim'
expect_grep 'reap-orphans: build output —'
# the primary checkout is still excluded by name, which is the reading that was breaking
expect_grep "keep — this checkout's own"

# ---------------------------------------------------------------- 2. a consent's subject
# `--reclaim` is consent for build output. It does not table servers.
rc=0
MJ_ROOT="$FIX" "$REAPER" --reclaim > "$T/reclaim.txt" 2>&1 || rc=$?
LAST_OUT="$(cat "$T/reclaim.txt")"
[ "$rc" = 0 ] || { echo "    --reclaim exited $rc, not 0:"; sed 's/^/      /' "$T/reclaim.txt"; exit 1; }
expect_no_grep '^PID +AGE'
expect_no_grep 'would reap'
expect_no_grep 'reap-orphans: servers'
expect_grep 'idle.*reclaimed'
[ -e "$WTS/idle/apps/majordomus-cli/target" ] && { echo "    --reclaim did not reclaim"; exit 1; }

# `--kill` is consent for servers. It does not table a hundred gigabytes of build output.
#
# `--min-age 999d` is load-bearing and not a detail: `candidates()` reads `ps` over the whole
# machine, not over the fixture, so a `--kill` here acts on this machine's real servers. It
# does so correctly — nothing without ppid 1 and zero attached clients is a candidate — but a
# case that ends another session's server is a case with a side effect on the machine it is
# measuring, and this one was written after doing exactly that to a server on port 8741. An
# age no process can have leaves the subject empty while still exercising the selection, which
# is the only thing being asserted here.
mkbuild "$WTS/idle"
rc=0
MJ_ROOT="$FIX" "$REAPER" --kill --min-age 999d > "$T/kill.txt" 2>&1 || rc=$?
LAST_OUT="$(cat "$T/kill.txt")"
expect_no_grep '^WORKTREE +SIZE'
expect_no_grep 'would reclaim'
expect_no_grep 'reap-orphans: build output'
expect_file "$WTS/idle/apps/majordomus-cli/target/MARKER"

# with no flags at all both are reported, and nothing is touched
rc=0
MJ_ROOT="$FIX" "$REAPER" > "$T/both.txt" 2>&1 || rc=$?
LAST_OUT="$(cat "$T/both.txt")"
expect_grep '^PID +AGE'
expect_grep '^WORKTREE +SIZE'
expect_file "$WTS/idle/apps/majordomus-cli/target/MARKER"

# and a consent whose subject was excluded by name is refused rather than silently inert
rc=0
MJ_ROOT="$FIX" "$REAPER" --servers --reclaim > "$T/conflict.txt" 2>&1 || rc=$?
LAST_OUT="$(cat "$T/conflict.txt")"
[ "$rc" = 12 ] || { echo "    --servers --reclaim exited $rc, not 12"; sed 's/^/      /' "$T/conflict.txt"; exit 1; }
expect_grep 'reclaim is consent for build output'
expect_file "$WTS/idle/apps/majordomus-cli/target/MARKER"

# ---------------------------------------------------------------- 3. a partial sweep says so
# The shape of the incident: the servers are examined, the build output is never reached, and
# what is on the screen is a header with a row under it. The exit status is not the only thing
# that may say this was a failure.
mkdir -p "$T/stub"
printf '#!/bin/sh\nexit 0\n' > "$T/stub/ps"
chmod +x "$T/stub/ps"
rc=0
PATH="$T/stub:$PATH" MJ_ROOT="$FIX" "$REAPER" > "$T/partial.txt" 2>&1 || rc=$?
LAST_OUT="$(cat "$T/partial.txt")"
[ "$rc" = 12 ] || { echo "    a blind sweep exited $rc, not 12:"; sed 's/^/      /' "$T/partial.txt"; exit 1; }
expect_grep 'INCOMPLETE'
expect_grep 'failure, not a report'
# it names both subjects: the one it examined, and the one it never reached
expect_grep 'servers +— examined'
expect_grep 'build output +— NOT REACHED'
expect_grep 'says nothing about reclaimable build directories'
expect_grep 'No consent was acted on for a subject this run did not reach'
# and it is still fail-closed: nothing was removed
expect_file "$WTS/idle/apps/majordomus-cli/target/MARKER"
rm "$T/stub/ps"

# a run that reaches its summary is never reported as incomplete, whatever it exits with
LAST_OUT="$(cat "$T/both.txt")"
expect_no_grep 'INCOMPLETE'

# ---------------------------------------------------------------- mutation
# Each mutation is a copy of the real script with one property removed. Before its verdict is
# believed, four things are asserted about the patch itself: the file changed, the sentinel
# the mutation introduces is present, the text it replaced is gone, and the result is still a
# shell script bash will parse. A sed that matched nothing would otherwise produce a pristine
# copy, and a pristine copy passes — which is the suite lying rather than the code failing.
# A mutant resolves `$0/../lib` like the real script does, so it needs a tree shaped like the
# repository's. Its lib is the repository's own: what is under test is the reaper, and a
# mutant reading a different library would be measuring something else.
MUT="$T/mutroot/scripts"
mkdir -p "$MUT"
ln -s "$ROOT/lib" "$T/mutroot/lib"

# mutate <name> <sed-program> <gone-pattern> <sentinel-pattern>
# Prints the mutant's path on stdout; every complaint goes to stderr, because the caller
# captures stdout and a diagnostic written there would become the path it then runs.
mutate() {
  local name="$1" prog="$2" gone="$3" sentinel="$4" m="$MUT/$1"
  sed "$prog" "$REAPER" > "$m"
  chmod +x "$m"
  if cmp -s "$REAPER" "$m"; then
    echo "    mutation '$name' changed nothing: its sed matched no line" >&2; return 1
  fi
  if ! grep -qE -- "$sentinel" "$m"; then
    echo "    mutation '$name' did not introduce its sentinel /$sentinel/" >&2; return 1
  fi
  if grep -qE -- "$gone" "$m"; then
    echo "    mutation '$name' left the original /$gone/ in place" >&2; return 1
  fi
  if ! bash -n "$m" 2>&1; then
    echo "    mutation '$name' is not valid shell; its verdict would be meaningless" >&2; return 1
  fi
  printf '%s' "$m"
}

# M0 — the signal itself, on the two readers in isolation. The reaper no longer exits 141
# because the guard beside the repaired reader converts any failure of that pipeline into a
# refusal, so the raw 141 is proved here, where nothing stands between it and the assertion.
cat > "$T/readers.sh" <<'READERS'
#!/usr/bin/env bash
set -u
set -o pipefail
old=0
git -C "$1" worktree list --porcelain 2>/dev/null \
  | awk 'NR == 1 && $1 == "worktree" { print substr($0, 10); exit }' >/dev/null || old=$?
new=0
git -C "$1" worktree list --porcelain 2>/dev/null \
  | awk '$1 == "worktree" && !seen { seen = 1; f = substr($0, 10) } END { if (seen) print f }' >/dev/null || new=$?
echo "old=$old new=$new"
READERS
chmod +x "$T/readers.sh"
sig="$(PATH="$T/bin:$PATH" MJ_PORCELAIN_PAD=2000 "$T/readers.sh" "$FIX")"
[ "$sig" = "old=141 new=0" ] || {
  echo "    over a $pad_bytes-byte porcelain the two readers gave '$sig', not 'old=141 new=0':"
  echo "    the signal this repair is about was not reproduced, so nothing below measures it"
  exit 1; }

# M1 — put the early-exiting reader back into the reaper. It must stop producing a verdict,
# and must say why: the primary checkout is excluded by name, and over a listing this long
# its name cannot be read at all.
m="$(mutate sigpipe \
  's/{ seen = 1; first = substr(\$0, 10) }/{ print substr($0, 10); exit }/' \
  'seen = 1; first = substr' \
  'print substr\(\$0, 10\); exit')" || exit 1
rc=0
PATH="$T/bin:$PATH" MJ_PORCELAIN_PAD=2000 MJ_ROOT="$FIX" "$m" --targets > "$T/m1.txt" 2>&1 || rc=$?
[ "$rc" != 10 ] || {
  echo "    M1 (the early-exiting reader restored) still exited 10: the repaired reader is not"
  echo "    load-bearing and the assertion above proves nothing"; sed 's/^/      /' "$T/m1.txt"; exit 1; }
grep -q 'primary checkout could not be identified' "$T/m1.txt" || {
  echo "    M1 failed for some reason other than the reading under test:"; sed 's/^/      /' "$T/m1.txt"; exit 1; }
# fail-closed even mutated: it refused rather than sweeping without the exclusion
grep -q 'INCOMPLETE' "$T/m1.txt" || { echo "    M1 did not report itself incomplete"; exit 1; }
expect_file "$FIX/apps/majordomus-cli/target/MARKER"

# M2 — stop a consent from selecting its subject. The case must see `--reclaim` table servers.
m="$(mutate scope \
  's/^  DO_SERVERS="\$KILL"$/  : # subject selection removed/' \
  '^  DO_SERVERS="\$KILL"$' \
  'subject selection removed')" || exit 1
mkbuild "$WTS/idle"
rc=0
MJ_ROOT="$FIX" "$m" --reclaim > "$T/m2.txt" 2>&1 || rc=$?
grep -qE '^PID +AGE' "$T/m2.txt" || {
  echo "    M2 (subject selection removed) did not make --reclaim table servers, so the"
  echo "    assertion above proves nothing:"; sed 's/^/      /' "$T/m2.txt"; exit 1; }

# M3 — silence the incomplete report. The case must see a partial sweep go back to looking
# like a verdict.
m="$(mutate silent \
  's/^FINISHED=0$/FINISHED=1 # report silenced/' \
  '^FINISHED=0$' \
  'report silenced')" || exit 1
printf '#!/bin/sh\nexit 0\n' > "$T/stub/ps"
chmod +x "$T/stub/ps"
rc=0
PATH="$T/stub:$PATH" MJ_ROOT="$FIX" "$m" > "$T/m3.txt" 2>&1 || rc=$?
grep -q 'INCOMPLETE' "$T/m3.txt" && {
  echo "    M3 (the incomplete report silenced) still printed INCOMPLETE, so the assertion"
  echo "    above does not depend on the report:"; sed 's/^/      /' "$T/m3.txt"; exit 1; }
rm "$T/stub/ps"

# and an unmutated copy in the same tree passes every assertion the mutants failed, so what
# the three verdicts above measured was the mutation and not the relocation.
cp "$REAPER" "$MUT/control"
chmod +x "$MUT/control"
mkbuild "$WTS/idle"   # M2's --reclaim consumed it, and a control with nothing to find is not one
rc=0
PATH="$T/bin:$PATH" MJ_PORCELAIN_PAD=2000 MJ_ROOT="$FIX" "$MUT/control" --targets > "$T/control.txt" 2>&1 || rc=$?
[ "$rc" = 10 ] || {
  echo "    the unmutated control exited $rc, not 10: the mutants above failed because they"
  echo "    were moved, not because they were mutated"; sed 's/^/      /' "$T/control.txt"; exit 1; }
exit 0

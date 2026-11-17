# majordomus-covers: none
# A name on PATH is not a tool that runs.
#
# On 2026-10-17 a push ran sixty-eight minutes of gates and then failed seven cases, the UI
# check and a site build, none of them because of the change. `node` was an asdf shim, and
# asdf answers "No version is set for command node" in any directory with no pinned version,
# which is every fixture directory the suite creates outside the repository. Every check asked
# only `command -v node`, took the shim, and read its failure as the subject's. The JSON parser
# of `doctor` had cost a day on 2026-10-08 in the same shape (case 999); this holds the class.
#
# What is held, over a shim that behaves as asdf's does and an asdf that lists what is installed:
#
#   mj_runs / mj_runs_why   a shim that cannot start is not a tool that runs, and the reason
#                           names the version manager and the variable that sets one
#   mj_node_env             picks the installed version of the Node major CI declares (else the
#                           newest) for this process, says so on stderr, never overrides a choice
#                           the person made (even a wrong one), and reports that node cannot run
#                           when nothing can make it
#   mj_zola_ready           zola runs and is at least the release CI builds the site with: an
#                           older one fails parsing a template mid-build, which reads as a
#                           broken change
#   the entry points        a script that needs node says why it cannot have it, with the
#                           reason, instead of failing in the middle of its work
#   the whole tree          no check gates on `command -v node`, `python3` or `zola` any more
. "$ROOT/test/lib.sh"

S="$(mktemp -d "${TMPDIR:-/tmp}/mj3180.XXXXXX")"; trap 'rm -rf "$S"' EXIT
mkdir -p "$S/asdf-bin" "$S/dead-bin"
# the shim: runs only when a version is chosen, and only an installed one
cat > "$S/asdf-bin/node" <<'SH'
#!/bin/sh
case "${ASDF_NODEJS_VERSION:-}" in
  18.1.0|22.11.0|24.8.0|26.4.0) echo "v$ASDF_NODEJS_VERSION"; exit 0 ;;
esac
echo "No version is set for command node" >&2
exit 126
SH
cat > "$S/asdf-bin/asdf" <<'SH'
#!/bin/sh
[ "$1 $2" = "list nodejs" ] && printf '  18.1.0\n  22.11.0\n *24.8.0\n  26.4.0\n'
exit 0
SH
# a node that never starts, with no version manager to ask
printf '#!/bin/sh\necho "node: cannot allocate" >&2\nexit 1\n' > "$S/dead-bin/node"
chmod +x "$S/asdf-bin/node" "$S/asdf-bin/asdf" "$S/dead-bin/node"
BASE="/usr/bin:/bin"

lib() { # lib PATH ENV... -- shell-code : run code with runnable.sh sourced under a controlled PATH
  local p="$1"; shift
  local envs=(); while [ "$1" != "--" ]; do envs+=("$1"); shift; done; shift
  env -i HOME="$S" PATH="$p:$BASE" ${envs[@]+"${envs[@]}"} bash -c ". '$ROOT/lib/runnable.sh'; $1" 2>"$S/err"
}
fail() { printf '    %s\n    stderr: %s\n' "$1" "$(cat "$S/err" 2>/dev/null)"; exit 1; }

# 1. a shim that cannot start is not a tool that runs, and the reason says what to do
lib "$S/asdf-bin" -- 'mj_runs node' && fail "a shim with no version set was taken for a node that runs"
why="$(lib "$S/asdf-bin" -- 'mj_runs_why node')"
case "$why" in *'on PATH but does not start'*'No version is set'*'ASDF_NODEJS_VERSION='*) ;; *) fail "the reason does not name the failure and the variable: $why" ;; esac
# 2. a tool that is not there says so, and one that runs says nothing
why="$(lib "$S/dead-bin" -- 'mj_runs_why no-such-tool-3180')"
[ "$why" = "no-such-tool-3180 is not installed" ] || fail "an absent tool is described as: $why"
lib "$S/asdf-bin" ASDF_NODEJS_VERSION=24.8.0 -- 'mj_runs node' || fail "a node that starts was refused"
[ -z "$(lib "$S/asdf-bin" ASDF_NODEJS_VERSION=24.8.0 -- 'mj_runs_why node')" ] || fail "a node that starts has a reason"

# 3. mj_node_env: the newest installed version, announced, for this process and its children
out="$(lib "$S/asdf-bin" -- 'mj_node_env && printf "%s %s" "$ASDF_NODEJS_VERSION" "$(node --version)"')" || fail "mj_node_env did not make node run"
[ "$out" = "26.4.0 v26.4.0" ] || fail "mj_node_env chose '$out' with no declaration to read, not the newest installed 26.4.0"
grep -q 'using the installed 26.4.0' "$S/err" || fail "mj_node_env did not say which version it used"
grep -q 'ASDF_NODEJS_VERSION to choose another' "$S/err" || fail "mj_node_env did not say how to choose another"
# ... and with the repository to read, the Node major CI declares wins over a newer one
pin="$(lib "$S/asdf-bin" -- "mj_ci_pin '$ROOT' node-version")"
[ -n "$pin" ] || fail "the Node major CI declares could not be read from the action that declares it"
out="$(lib "$S/asdf-bin" -- "mj_node_env '$ROOT' && printf '%s' \"\$ASDF_NODEJS_VERSION\"")" || fail "mj_node_env with a repository did not make node run"
case "$out" in "$pin".*) ;; *) fail "mj_node_env chose '$out', not an installed $pin.x that CI declares" ;; esac
# ... which a child process inherits, as the cases and gates of a push are
out="$(lib "$S/asdf-bin" -- 'mj_node_env && sh -c "node --version"')" || fail "a child did not inherit the version"
[ "$out" = "v26.4.0" ] || fail "a child saw '$out'"

# 4. a choice the person made is theirs, a working one and a broken one
out="$(lib "$S/asdf-bin" ASDF_NODEJS_VERSION=18.1.0 -- 'mj_node_env && printf "%s" "$ASDF_NODEJS_VERSION"')" || fail "a working choice was refused"
[ "$out" = "18.1.0" ] || fail "a working choice was overridden by $out"
! grep -q 'using the installed' "$S/err" || fail "a working choice was announced as if the tool had chosen"
rc=0; out="$(lib "$S/asdf-bin" ASDF_NODEJS_VERSION=99.0.0 -- 'mj_node_env; echo "rc=$? v=$ASDF_NODEJS_VERSION"')" || true
case "$out" in 'rc=1 v=99.0.0') ;; *) fail "a broken choice was second-guessed: $out" ;; esac

# 5. nothing to choose from: reported, not invented
lib "$S/dead-bin" -- 'mj_node_env' && fail "mj_node_env claimed a node that never starts"

# 6. an entry point that needs node says why it cannot have it, instead of failing midway
rc=0; env -i HOME="$S" PATH="$S/dead-bin:$BASE" "$ROOT/scripts/ci/interaction-check" >"$S/out" 2>"$S/err" || rc=$?
[ "$rc" = 12 ] || fail "interaction-check with a node that never starts exited $rc, not 12"
grep -q 'node is required: node is on PATH but does not start (node: cannot allocate)' "$S/err" \
  || fail "interaction-check does not name why it cannot have node"
# ... and with the shim and an asdf that can answer, the entry point finds a version itself
rc=0; env -i HOME="$S" PATH="$S/asdf-bin:$BASE" "$ROOT/scripts/ci/interaction-check" >"$S/out" 2>"$S/err" || rc=$?
grep -q 'using the installed 22.11.0' "$S/err" || fail "interaction-check did not find the version CI declares for itself"
! grep -q 'node is required' "$S/err" || fail "interaction-check still refused a node it had just made runnable"

# 6b. zola: absent, too old for the templates, current. The pin is read, not copied
zpin="$(lib "$S/dead-bin" -- "mj_ci_pin '$ROOT' zola-version")"
[ -n "$zpin" ] || fail "the Zola release CI declares could not be read"
mkdir -p "$S/zola-old" "$S/zola-new" "$S/zola-newer"
printf '#!/bin/sh\necho "zola 0.22.1"\n' > "$S/zola-old/zola"
printf '#!/bin/sh\necho "zola %s"\n' "$zpin" > "$S/zola-new/zola"
printf '#!/bin/sh\necho "zola 9.0.0"\n' > "$S/zola-newer/zola"
chmod +x "$S/zola-old/zola" "$S/zola-new/zola" "$S/zola-newer/zola"
lib "$S/zola-old" -- "mj_zola_ready '$ROOT'" && fail "a zola older than $zpin was taken for one that can build the site"
why="$(lib "$S/zola-old" -- "mj_zola_why '$ROOT'")"
case "$why" in "zola 0.22.1 is older than the $zpin the site is built with"*) ;; *) fail "the reason for an old zola is: $why" ;; esac
lib "$S/zola-new" -- "mj_zola_ready '$ROOT'" || fail "the pinned zola $zpin was refused"
lib "$S/zola-newer" -- "mj_zola_ready '$ROOT'" || fail "a zola newer than the pin was refused"
[ -z "$(lib "$S/zola-new" -- "mj_zola_why '$ROOT'")" ] || fail "a zola that is ready has a reason"
why="$(lib "$S/dead-bin" -- "mj_zola_why '$ROOT'")"
[ "$why" = "zola is not installed" ] || fail "an absent zola is described as: $why"

# 7. the whole tree: nothing gates on presence alone any more
bad="$(cd "$ROOT" && git grep -n -E 'command -v (node|python3|zola)( |$|>)' -- scripts test/cases test/lib.sh ':!scripts/ci/*-allow*' 2>/dev/null | grep -v 'lib/runnable.sh' || true)"
# (the loader line of a script that may be copied without lib/ carries a one-line fallback to
# the old question on purpose; it is the only place that is allowed to ask it)
if [ -n "$bad" ]; then
  printf '    these still ask only whether node, python3 or zola is on PATH; use mj_runs or mj_zola_ready (lib/runnable.sh):\n%s\n' "$bad"
  exit 1
fi

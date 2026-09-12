# The shell entry point is an adapter: it resolves the tool and makes exactly one call to
# it — the bootstrap command — evaluating what that call exports. It reads nothing about the
# repository itself, builds nothing, and reaches no network of its own.
#
# Since ADR 0043 that one call brings the runtime up, which is why the "exactly one" half is
# now checked rather than assumed: two calls are two readings of the repository on the hot
# path of every `cd`, and the moment the file chooses between commands it has started
# deciding things. What the one call *does* is project.entry-converges' and case 190's; this
# case is about the file.
#
# The rule is `project.envrc-is-an-adapter`, and this is its verification. There is no
# runtime enforcement and there should not be: the file is evaluated by a shell this tool
# does not control, and a check that ran there would be one more thing running on every
# `cd`. So the check runs here, over the file as written, and the last section proves the
# check can fail — a forbidden line introduced into a copy must be rejected by the same
# reader, or the reader has quietly stopped reading.
#
# The reader is written by shape rather than by naming today's lines: it looks for a
# *command* the rule forbids in a position where a shell would run it, so it keeps working
# when the file is rewritten and keeps failing when a pipeline is added to it.
. "$ROOT/test/lib.sh"

ENVRC="$ROOT/.envrc"
[ -f "$ENVRC" ] || { echo "    the repository has no .envrc"; exit 1; }

# ---------------------------------------------------------------- the reader
#
# One function, used twice: once over the real file, once over a mutated copy. Prints each
# offending line; returns 1 when it found any.
adapter_check() {
  local file="$1" bad=0 n=0 line stripped
  # every program the rule names, as a word at the start of a command or after a pipe,
  # a substitution or a `&&`
  local forbidden='git|grep|sed|awk|find|jq|wc|curl|wget|cargo|npm|pnpm|yarn|docker|make|xargs|perl|python|python3|ruby|node'
  while IFS= read -r line; do
    n=$((n + 1))
    case "$line" in
      \#*|"") continue ;;
    esac
    # the text a shell would execute, with the leading keyword noise removed
    stripped="$(printf '%s' "$line" | sed 's/^[[:space:]]*//')"
    if printf '%s' "$stripped" | grep -Eq "(^|[|;&]|\\\$\\(|\`)[[:space:]]*($forbidden)([[:space:]]|\$)"; then
      echo "    .envrc:$n runs a program the rule forbids: $stripped"
      bad=1
    fi
    # a loop or a conditional is repository logic wearing a shell's clothes
    if printf '%s' "$stripped" | grep -Eq '^(for|while|until|case|if)[[:space:]]'; then
      echo "    .envrc:$n carries control flow: $stripped"
      bad=1
    fi
    # a parameter of the runtime is the bootstrap command's own default, declared once in
    # the command line's declaration and changed there, not carried here
    if printf '%s' "$stripped" | grep -Eq -- '--(wait|port|idle)[ =]'; then
      echo "    .envrc:$n carries a parameter of the runtime: $stripped"
      bad=1
    fi
  done < "$file"
  # the budget: an adapter is a handful of statements, and a file that grows past it has
  # started to become a program
  local statements
  statements="$(grep -cvE '^[[:space:]]*(#|$)' "$file")"
  if [ "$statements" -gt 8 ]; then
    echo "    .envrc carries $statements statements, over the budget of 8"
    bad=1
  fi
  # exactly one call to the tool, and it is the bootstrap command (ADR 0043). Counted over
  # the text a shell would execute, with comments and single-quoted strings gone, so that a
  # command named inside a message is not mistaken for a call.
  local stripped_file calls
  stripped_file="$(mktemp "${TMPDIR:-/tmp}/mj100.XXXXXX")"
  sed -e 's/^[[:space:]]*#.*$//' -e "s/'[^']*'//g" "$file" > "$stripped_file"
  calls="$(grep -cE '(^|[|;&(){]|\$\()[[:space:]]*(bin/)?majordomus[a-z-]*[[:space:]]' "$stripped_file" || true)"
  if [ "$calls" != 1 ]; then
    echo "    .envrc makes $calls call(s) to the tool; entering the repository is one call"
    bad=1
  fi
  if ! grep -Eq '(^|[|;&(){]|\$\()[[:space:]]*(bin/)?majordomus[a-z-]*[[:space:]]+enter([[:space:]]|"|$)' "$stripped_file"; then
    echo "    .envrc's one call is not the bootstrap command (majordomus env enter)"
    bad=1
  fi
  rm -f "$stripped_file"
  return "$bad"
}

adapter_check "$ENVRC" || exit 1

# it does the three things the rule allows, and they are what it is for
grep -q 'PATH_add' "$ENVRC" || { echo "    the adapter puts nothing on the path"; exit 1; }
grep -q 'watch_file' "$ENVRC" || { echo "    the adapter declares nothing to watch"; exit 1; }
grep -q 'majordomus-env' "$ENVRC" || { echo "    the adapter does not call the tool"; exit 1; }
# the lease is what it watches: the entry that starts a server returns before that server has
# published an address, and this is what brings the address in when it does (ADR 0043)
grep -q 'state/mcp/server.json' "$ENVRC" || {
  echo "    the adapter does not watch the lease; a cold entry's address would never arrive"; exit 1; }

# ---------------------------------------------------------------- the adapter it calls
#
# A checkout that has not built the executable must not fail, hang or start a compiler: one
# line naming the recipe that builds it, and exit 0.
MISSING="$T/missing"; mkdir -p "$MISSING/bin" "$MISSING/lib"
cp "$ROOT/bin/majordomus-env" "$MISSING/bin/"
cp "$ROOT/lib/rust_bin.sh" "$MISSING/lib/"
# CARGO_TARGET_DIR empty as well as MAJORDOMUS_BIN: the adapter reads that variable, so a
# suite run by somebody whose worktrees share one build directory would find a real
# executable here and this would assert nothing. "No executable anywhere" is the case.
( cd "$MISSING" && MAJORDOMUS_BIN="" CARGO_TARGET_DIR="" bin/majordomus-env enter >"$T/missing.out" 2>"$T/missing.err" )
code=$?
[ "$code" = 0 ] || { echo "    the adapter exited $code where the executable is absent"; exit 1; }
grep -q 'not built' "$T/missing.err" || {
  echo "    the adapter did not say what to run:"; cat "$T/missing.err"; exit 1; }
[ ! -s "$T/missing.out" ] || {
  echo "    the adapter wrote to standard output where the executable is absent"; exit 1; }

# ---------------------------------------------------------------- what it exports
#
# direnv reads standard output as the environment it applies, so every line of it must be
# an assignment a shell can evaluate and nothing else.
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

"$RB" env export --shell direnv > "$T/export.sh" 2>"$T/export.err"
[ -s "$T/export.sh" ] || { echo "    the export is empty"; exit 1; }
while IFS= read -r line; do
  case "$line" in
    ""|\#*) continue ;;
    export\ *=*|[A-Za-z_]*=*) continue ;;
    *) echo "    the export carries something that is not an assignment: $line"; exit 1 ;;
  esac
done < "$T/export.sh"
# and a shell evaluates it back without complaint
bash -c ". '$T/export.sh'" || { echo "    a shell could not evaluate the export"; exit 1; }

# the banner goes to standard error, because standard output is the environment
"$RB" env export --shell direnv --banner --mode full >"$T/b.out" 2>"$T/b.err"
grep -qE '^(export )?[A-Za-z_]+=' "$T/b.out" || { echo "    the export lost its assignments"; exit 1; }
[ -s "$T/b.err" ] || { echo "    the banner was not drawn"; exit 1; }

# ---------------------------------------------------------------- what entering exports
#
# The same contract as `export`, because `enter` is what the entry file actually calls: every
# line of standard output is an assignment a shell can evaluate and nothing else. `--no-runtime`
# because this asserts the shape of the output, not that a server came up — that is case 190's.
"$RB" env enter --shell direnv --no-banner --no-bridge --no-runtime > "$T/enter.sh" 2>"$T/enter.err"
[ -s "$T/enter.sh" ] || { echo "    entering exported nothing:"; cat "$T/enter.err"; exit 1; }
while IFS= read -r line; do
  case "$line" in
    ""|\#*) continue ;;
    export\ *=*|[A-Za-z_]*=*) continue ;;
    *) echo "    entering exported something that is not an assignment: $line"; exit 1 ;;
  esac
done < "$T/enter.sh"
bash -c ". '$T/enter.sh'" || { echo "    a shell could not evaluate what entering exported"; exit 1; }

# ---------------------------------------------------------------- the check can fail
#
# A reader that silently stops reading and a rule that silently stops being enforced are
# the same failure. The probe asserts it took effect before the reader is asked.
PROBE="$T/probe.envrc"
{ cat "$ENVRC"; printf '%s\n' 'export MJ_BRANCH="$(git rev-parse --abbrev-ref HEAD)"'; } > "$PROBE"
grep -q 'git rev-parse' "$PROBE" || { echo "    the probe did not take"; exit 1; }
if adapter_check "$PROBE" >/dev/null 2>&1; then
  echo "    the reader accepted an .envrc that runs git"; exit 1
fi

# and a second call to the tool, which is what version 2 of the rule added: one call, or the
# file has started choosing between commands
SECOND="$T/second.envrc"
{ cat "$ENVRC"; printf '%s\n' 'bin/majordomus-env banner'; } > "$SECOND"
if adapter_check "$SECOND" >/dev/null 2>&1; then
  echo "    the reader accepted an .envrc that calls the tool twice"; exit 1
fi

# and a runtime parameter carried here rather than left to the command's own default
PARAM="$T/param.envrc"
sed 's|enter --shell direnv|enter --shell direnv --wait 20|' "$ENVRC" > "$PARAM"
grep -q -- '--wait 20' "$PARAM" || { echo "    the parameter probe did not take"; exit 1; }
if adapter_check "$PARAM" >/dev/null 2>&1; then
  echo "    the reader accepted an .envrc that makes entering a wait"; exit 1
fi

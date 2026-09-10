# The shell entry point is an adapter: it resolves the tool, evaluates what the tool
# exports and asks it to render. It reads nothing about the repository itself, builds
# nothing, and reaches no network.
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
  done < "$file"
  # the budget: an adapter is a handful of statements, and a file that grows past it has
  # started to become a program
  local statements
  statements="$(grep -cvE '^[[:space:]]*(#|$)' "$file")"
  if [ "$statements" -gt 8 ]; then
    echo "    .envrc carries $statements statements, over the budget of 8"
    bad=1
  fi
  return "$bad"
}

adapter_check "$ENVRC" || exit 1

# it does the four things the rule allows, and they are what it is for
grep -q 'PATH_add' "$ENVRC" || { echo "    the adapter puts nothing on the path"; exit 1; }
grep -q 'watch_file' "$ENVRC" || { echo "    the adapter declares nothing to watch"; exit 1; }
grep -q 'majordomus-env' "$ENVRC" || { echo "    the adapter does not call the tool"; exit 1; }

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
( cd "$MISSING" && MAJORDOMUS_BIN="" CARGO_TARGET_DIR="" bin/majordomus-env status >"$T/missing.out" 2>"$T/missing.err" )
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

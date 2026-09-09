# The shell completion, in a real shell.
#
# Shell completion is notorious for implementations that look correct in a file and do
# nothing at all: the function is never defined, `compdef` is never reached, the word array
# is parsed wrongly, and the only symptom is that TAB does what TAB always did. Reading the
# generated script proves none of that, so this loads it into a real zsh, stubs the two
# completion builtins the adapter calls, sets the variables zsh itself would have set, and
# reads what the adapter offers.
#
# What it does not prove is the terminal: driving a pseudo-terminal to press TAB is a test
# of zsh rather than of this repository. Everything between "the shell loaded the script"
# and "these are the candidates" is proved here.
#
# Skips itself where there is no zsh, and where there is no cargo and no MAJORDOMUS_BIN.
. "$ROOT/test/lib.sh"
command -v zsh >/dev/null 2>&1 || { echo "    skip: no zsh"; exit 0; }
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
# A workflow of this repository's own, so the runner's completion can be asked about both
# halves: a recipe the bridge projects, and a recipe a person declared.
if command -v just >/dev/null 2>&1; then
  printf '%s\n' '# A workflow this repository declares.' 'demo-workflow:' '    @echo ran' > justfile
fi
git add -A >/dev/null && git commit -qm install
# the graph, and the cache the completion reads, for this repository
"$RB" commands bridge >/dev/null

cat > "$T/smoke.zsh" <<'ZSH'
emulate -L zsh
setopt err_exit no_unset
autoload -Uz compinit && compinit -u -d "$TMP/zcompdump" 2>/dev/null

eval "$("$MAJORDOMUS_COMPLETION_BIN" completion init --shell zsh)"

typeset -f _majordomus_query >/dev/null || { print -r -- "MISSING query"; exit 1 }
typeset -f _majordomus_complete >/dev/null || { print -r -- "MISSING cli"; exit 1 }
typeset -f _majordomus_workflow_complete >/dev/null || { print -r -- "MISSING workflow"; exit 1 }

# the builtins the adapter calls, stubbed so what it offers is visible
_describe() { local arr="${@[-1]}"; print -r -- "OFFER ${(P@)arr}" }
_files() { print -r -- "OFFER-FILES" }

run() { local surface="$1"; shift; words=("$@"); CURRENT=$(( $#words + 1 )); _majordomus_query "$surface" }

print -r -- "=== cli-root";     run cli majordomus
print -r -- "=== cli-nested";   run cli majordomus worktree
print -r -- "=== workflow";     run workflow just
print -r -- "=== workflow-arg"; words=(just worktree-status --); CURRENT=3; _majordomus_query workflow
ZSH

MAJORDOMUS_COMPLETION_BIN="$RB" TMP="$T" zsh "$T/smoke.zsh" > "$T/offers.txt" 2>"$T/offers.err" || {
  echo "    the adapter failed in zsh:"; cat "$T/offers.err"; exit 1; }

grep -q 'MISSING' "$T/offers.txt" && { echo "    the adapter defined nothing:"; cat "$T/offers.txt"; exit 1; }

section() { awk -v s="=== $1" '$0==s{f=1;next} /^=== /{f=0} f' "$T/offers.txt"; }

# the root of the command line offers commands of both programs, with their descriptions
section cli-root > "$T/root.txt"
grep -q 'worktree:' "$T/root.txt" || { echo "    the root offered no command of the executable"; exit 1; }
grep -q 'check:'    "$T/root.txt" || { echo "    the root offered no command of the shell tool"; exit 1; }

# a nested command offers its subcommands, and the alias clap declares
section cli-nested > "$T/nested.txt"
grep -q 'status:' "$T/nested.txt" || { echo "    a nested command offered no subcommand"; exit 1; }

# the workflow runner offers the recipes the bridge projects
section workflow > "$T/workflow.txt"
grep -q 'worktree-status:' "$T/workflow.txt" || { echo "    the runner offered no bridged recipe"; exit 1; }
if command -v just >/dev/null 2>&1; then
  grep -q 'demo-workflow:' "$T/workflow.txt" || {
    echo "    the runner offered no declared workflow:"; head -c 400 "$T/workflow.txt"; exit 1; }
fi

# and a bridged recipe's arguments are the command's own: one engine, two surfaces
section workflow-arg > "$T/arg.txt"
grep -q '\-\-repo:' "$T/arg.txt" || {
  echo "    a recipe did not offer the flags of the command it bridges:"; cat "$T/arg.txt"; exit 1; }

# every adapter this executable can print is syntactically loadable by its own shell, and
# fish's protocol is the same value-tab-description the engine already prints
if command -v fish >/dev/null 2>&1; then
  "$RB" completion init --shell fish > "$T/adapter.fish"
  fish -n "$T/adapter.fish" || { echo "    the fish adapter does not parse"; exit 1; }
fi
bash -n <("$RB" completion init --shell bash) || { echo "    the bash adapter does not parse"; exit 1; }

# no adapter carries a command of its own: the same script, with the executable removed,
# offers nothing rather than a stale list
MAJORDOMUS_COMPLETION_BIN="$T/nonesuch" TMP="$T" zsh "$T/smoke.zsh" > "$T/none.txt" 2>/dev/null || true
if grep -q 'worktree-status:' "$T/none.txt"; then
  echo "    the adapter offered a command without asking the executable"; exit 1
fi

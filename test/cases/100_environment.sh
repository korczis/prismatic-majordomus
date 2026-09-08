# majordomus-covers: none
# The shell entry point, held to what `project.envrc-is-an-adapter` allows it to be.
#
# `.envrc` runs on every entry into this directory, in whatever shell somebody uses, and
# nothing else in the repository would ever notice what it grew into. The rule enumerates
# what such a file may do — put a directory on the path, watch a file, `eval` what the tool
# exports, ask the tool to render — and this case is the only thing that holds it to that
# list. There is deliberately no runtime enforcement: a check running on every `cd` is one
# more thing running on every `cd`.
#
# Four things are proved here: this repository's entry point passes the check, the adapter
# it calls exits 0 and names the recipe when the executable is not built, what the tool
# exports is assignments and nothing else, and — because a gate that cannot fail is
# decoration — the same check rejects a copy with one forbidden line added.
. "$ROOT/test/lib.sh"

ENVRC="$ROOT/.envrc"
ADAPTER="$ROOT/bin/majordomus-env"

expect_file "$ENVRC"
[ -x "$ADAPTER" ] || { echo "    $ADAPTER is missing or not executable"; exit 1; }

# ---------------------------------------------------------------- the check
# Reads a candidate entry point and prints the offending line when it does more than adapt.
# The list is the rule's: a program that inspects the repository, a build, a network call.
adapter_check() {
  local file="$1" body rc=0 cmd n subs
  # comments carry the explanation of the file and name the very commands it must not run
  body="$(sed 's/#.*$//' "$file")"

  for cmd in git grep sed awk find jq wc cat head tail curl wget nc ssh cargo npm yarn make cmake python python3 perl ruby; do
    if printf '%s\n' "$body" | grep -qE "(^|[;&|(\`]|[[:space:]]|\\\$\()[[:space:]]*$cmd([[:space:]]|\$|\))"; then
      printf '    %s runs %s, which reads the repository or builds it\n' "$(basename "$file")" "$cmd"
      printf '%s\n' "$body" | grep -nE "(^|[;&|(\`]|[[:space:]])$cmd([[:space:]]|\$)" | head -2 | sed 's/^/      /'
      rc=1
    fi
  done

  for word in if while for case until function; do
    if printf '%s\n' "$body" | grep -qE "^[[:space:]]*$word([[:space:]]|\$)"; then
      printf '    %s carries control flow (%s); an adapter decides nothing\n' "$(basename "$file")" "$word"
      rc=1
    fi
  done

  # One command substitution is the `eval` of what the tool exports. A second is a pipeline.
  subs="$(printf '%s\n' "$body" | grep -o '\$(' | wc -l | tr -d ' ')"
  [ "$subs" -le 1 ] || {
    printf '    %s runs %s command substitutions; the adapter needs one, for the eval\n' \
      "$(basename "$file")" "$subs"; rc=1; }

  # The budget is the whole point: this file is read by a person deciding whether to trust
  # it, on a hot path nobody chose to wait for. Eight lines of shell is already generous.
  n="$(printf '%s\n' "$body" | grep -cE '[^[:space:]]')"
  [ "$n" -le 8 ] || {
    printf '    %s carries %s lines of shell, over the budget of 8\n' "$(basename "$file")" "$n"
    rc=1; }

  return "$rc"
}

# ---------------------------------------------------------------- this repository passes
adapter_check "$ENVRC" || { echo "    the repository's own .envrc does not hold the rule"; exit 1; }

# ---------------------------------------------------------------- and the check can fail
# The same check, over a copy that grew the most natural line in the world.
cp "$ENVRC" "$T/envrc-with-git"
printf '\nexport BRANCH="$(git rev-parse --abbrev-ref HEAD)"\n' >> "$T/envrc-with-git"
if adapter_check "$T/envrc-with-git" >/dev/null 2>&1; then
  echo "    the check accepts an entry point that runs git; it proves nothing"; exit 1
fi

cp "$ENVRC" "$T/envrc-too-long"
i=0; while [ "$i" -lt 12 ]; do printf 'export MJ_PADDING_%s=1\n' "$i" >> "$T/envrc-too-long"; i=$((i+1)); done
if adapter_check "$T/envrc-too-long" >/dev/null 2>&1; then
  echo "    the check accepts an entry point over its complexity budget"; exit 1
fi

# ---------------------------------------------------------------- entering cannot fail
# A checkout that has not built the executable is the normal state of a fresh clone, and a
# `cd` into it must print one line and succeed — not start a compiler, not report that the
# whole environment failed.
out="$(MAJORDOMUS_BIN=/nonexistent/majordomus "$ADAPTER" export --shell posix 2>&1)" || {
  echo "    the adapter exits non-zero when the executable is absent"; exit 1; }
printf '%s\n' "$out" | grep -q 'just build' || {
  printf '    the adapter does not name the recipe that builds it: %s\n' "$out"; exit 1; }

# ---------------------------------------------------------------- assignments, and nothing else
RB="$(rust_bin)" || rust_bin_exit $?
# the case runs from a fixture repository of its own, so the checkout under test is named
script="$(MAJORDOMUS_BIN="$RB" MAJORDOMUS_LOG=error "$ADAPTER" export --shell posix --repo "$ROOT")" || {
  echo "    the adapter failed to export"; exit 1; }
printf '%s\n' "$script" | grep -q 'MAJORDOMUS_ROOT=' || {
  echo "    the exported script names no repository root"; exit 1; }
printf '%s\n' "$script" | sed 's/#.*$//' | grep -vE '^[[:space:]]*$' \
  | grep -vE "^export [A-Za-z_][A-Za-z0-9_]*='[^']*'$" > "$T/not-an-assignment" || true
[ -s "$T/not-an-assignment" ] && {
  echo "    the exported script carries something that is not a single-quoted assignment:"
  sed 's/^/      /' "$T/not-an-assignment"; exit 1; }

# and a shell really does evaluate it back to the same values
eval "$script"
[ -n "${MAJORDOMUS_ROOT:-}" ] || { echo "    evaluating the exported script sets no root"; exit 1; }
[ "$MAJORDOMUS_ROOT" = "$ROOT" ] || {
  printf '    the exported root is %s, not %s\n' "$MAJORDOMUS_ROOT" "$ROOT"; exit 1; }

# ---------------------------------------------------------------- the banner stays off stdout
banner_out="$(MAJORDOMUS_BIN="$RB" MAJORDOMUS_LOG=error MAJORDOMUS_BANNER=compact "$ADAPTER" banner --repo "$ROOT" 2>/dev/null)"
[ -z "$banner_out" ] || {
  echo "    the banner wrote to standard output, which direnv reads as the environment"; exit 1; }

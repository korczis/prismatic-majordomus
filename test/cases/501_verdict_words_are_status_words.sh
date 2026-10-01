# majordomus-covers: none
# Every evidence verdict is a status word of the design system, so the site badge and the
# Cockpit colour a verdict from the declaration and never from a branch written for it.
#
# The verdicts are read from the executable's own schema (ProofState in
# docs/generated/openapi.json, each word with `_` spelled `-`), never from a list here. Each
# must be filed exactly once in share/design/tokens.yaml status.states, under the role the
# table below decides; a verdict the table does not place fails the case by name, so a
# verdict added to ProofState gets a declared role before it can render. Both stylesheets
# that colour a badge carry its selector. Then, in a fixture copy and through the generator,
# a verdict filed under a second role and a verdict spelled with an underscore are refused.
#
# The static assertions need neither cargo nor MAJORDOMUS_BIN; the generator section skips
# without them, as the other Rust cases do.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq is required"; exit 1; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj501.XXXXXX")"; trap 'rm -rf "$S"' EXIT

fail=0
note() { echo "    $*"; fail=1; }

# --- the vocabulary: the verdicts the executable declares, spelled as state words
schema="$ROOT/docs/generated/openapi.json"
raw="$(jq -r '.components.schemas.ProofState.oneOf[].const' "$schema")" || {
  echo "    the schema ProofState in docs/generated/openapi.json is not a oneOf of const words"
  exit 1
}
words="$(printf '%s\n' "$raw" | tr _ -)"
[ -n "$words" ] || { echo "    the schema ProofState yields no word"; exit 1; }

# --- the decision under test: which status role each verdict renders as. This is the one
# list the case holds; every other list is read.
roles='proven ok
inputs-unchanged info
stale warn
failing bad
unrunnable bad
not-run neutral
no-test neutral'

# the table decides verdicts, and only verdicts: a row the schema no longer declares is a
# decision about a word nothing emits
for w in $(printf '%s\n' "$roles" | awk '{print $1}'); do
  printf '%s\n' "$words" | grep -qxF -- "$w" \
    || note "the table places '$w', which the schema ProofState does not declare"
done

# --- filing: every (word, role) pair status.states files, read from the declaration
tokens="$ROOT/share/design/tokens.yaml"
filed="$(awk '
  /^[^ #]/ { in_status = ($0 ~ /^status:/); in_states = 0; next }
  in_status && /^  [^ #]/ { in_states = ($0 ~ /^  states:/); next }
  in_states && /^    [a-z][a-z0-9-]*: *\[/ {
    role = $0; sub(/^ +/, "", role); sub(/:.*$/, "", role)
    list = $0; sub(/^[^[]*\[/, "", list); sub(/\].*$/, "", list)
    n = split(list, w, ",")
    for (i = 1; i <= n; i++) { gsub(/^[ \t]+|[ \t]+$/, "", w[i]); if (w[i] != "") print w[i], role }
  }' "$tokens")"
[ -n "$filed" ] || { echo "    status.states in share/design/tokens.yaml yields no word"; exit 1; }

for w in $words; do
  want="$(printf '%s\n' "$roles" | awk -v w="$w" '$1 == w { print $2 }')"
  if [ -z "$want" ]; then
    note "the verdict '$w' has no declared role: decide it in this case's table first"
    continue
  fi
  got="$(printf '%s\n' "$filed" | awk -v w="$w" '$1 == w { print $2 }' | paste -sd, -)"
  n="$(printf '%s\n' "$filed" | awk -v w="$w" '$1 == w' | wc -l | tr -d ' ')"
  if [ "$n" != 1 ]; then
    note "the verdict '$w' is filed $n times in status.states (${got:-nowhere});" \
      "file it once, under $want"
  elif [ "$got" != "$want" ]; then
    note "the verdict '$w' is filed under $got; its role is $want"
  fi
done
[ "$fail" = 0 ] || exit 1

# --- selectors: both stylesheets that colour a badge carry one for every verdict
for f in share/design/status.css share/cockpit/cockpit.css; do
  [ -f "$ROOT/$f" ] || { note "$f is missing"; continue; }
  for w in $words; do
    grep -qE -- "\.mj-badge--$w([^a-z0-9-]|\$)" "$ROOT/$f" \
      || note "$f has no .mj-badge--$w selector"
  done
done
[ "$fail" = 0 ] || exit 1

# --- the generator refuses a verdict filed twice and a verdict spelled with an underscore.
# The fixture is 109_design_system.sh's: a repository carrying the distribution beside it,
# so that the design generated is this tree's. $ROOT is only read.
RB="$(rust_bin)" || rust_bin_exit $?
"$MJ" init >/dev/null
cp -R "$ROOT/share" share
mkdir -p apps/majordomus-cli
cp "$ROOT/apps/majordomus-cli/Cargo.toml" apps/majordomus-cli/Cargo.toml
git add -A >/dev/null && git commit -qm fixture
gen() { env -u MAJORDOMUS_SHARE "$RB" generate design --repo "$T" "$@"; }
decl=share/design/tokens.yaml
cp "$decl" "$S/tokens.yaml"

# file <word> first in <role>'s list, on a fresh copy of the declaration
file_word() { # role word
  sed "s/^    $1: \\[$1, /    $1: [$1, $2, /" "$S/tokens.yaml" > "$decl"
  grep -q "^    $1: \\[$1, $2, " "$decl" \
    || { echo "    the mutation did not file '$2' under $1"; exit 1; }
}
unprojected() {
  cmp -s share/design/status.css "$S/status.before" \
    || { echo "    a refused declaration was projected"; exit 1; }
}

# the control: the declaration as it is projects
expect_exit 0 gen
cp share/design/status.css "$S/status.before"

# a verdict filed under a second role
file_word warn failing
expect_exit 10 gen
expect_grep "'failing' is already filed under"
unprojected

# a verdict spelled as the enum spells it, not as a state word
file_word neutral not_run
expect_exit 10 gen
expect_grep "'not_run' is not a state word"
unprojected

cp "$S/tokens.yaml" "$decl"
expect_exit 0 gen

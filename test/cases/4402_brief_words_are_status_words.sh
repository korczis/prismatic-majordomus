# majordomus-covers: none
# Every word the brief judges with is a status word of the design system (I4390), so the
# Cockpit's badge and the report's brackets colour and read a word from the declaration and
# never from a branch written for it. Sibling of case 501, which holds the evidence verdicts.
#
# The words are read from the executable's own schema (BriefVerdict and BriefSectionState in
# docs/generated/openapi.json) and from the report contract's attention rows
# (share/brief/report.yaml), never from a list here. Each must be filed exactly once in
# share/design/tokens.yaml status.states, under the role the table below decides; a word the
# table does not place fails the case by name. Then, in a fixture copy and through the
# generator, a word filed under a second role is refused.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq is required"; exit 1; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj4402.XXXXXX")"; trap 'rm -rf "$S"' EXIT
fail=0
note() { echo "    $*"; fail=1; }

schema="$ROOT/docs/generated/openapi.json"
verdicts="$(jq -r '.components.schemas.BriefVerdict.oneOf[].const, .components.schemas.BriefSectionState.oneOf[].const' "$schema")" || {
  echo "    BriefVerdict or BriefSectionState in docs/generated/openapi.json is not a oneOf of const words"; exit 1; }
attention="$(awk '/^attention:/ { on = 1; next } /^[a-z_]+:/ { on = 0 } on && /^    word: / { sub(/^    word: /, ""); print }' "$ROOT/share/brief/report.yaml")"
[ -n "$verdicts" ] && [ -n "$attention" ] || { echo "    no word was read"; exit 1; }
words="$(printf '%s\n%s\n' "$verdicts" "$attention" | tr _ - | sort -u)"

# --- the decision under test: which role each word renders as. The one list the case holds.
roles='current ok
read ok
stale warn
partial warn
degraded warn
dirty warn
warn warn
unread bad
failing bad
blocked bad
superseded bad
overdue bad
owed bad
queued info
pending info
unmerged info
undeclared neutral
inactive neutral'

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

for w in $words; do
  want="$(printf '%s\n' "$roles" | awk -v w="$w" '$1 == w { print $2 }')"
  [ -n "$want" ] || { note "the brief word '$w' has no declared role: decide it in this case's table first"; continue; }
  got="$(printf '%s\n' "$filed" | awk -v w="$w" '$1 == w { print $2 }' | paste -sd, -)"
  n="$(printf '%s\n' "$filed" | awk -v w="$w" '$1 == w' | wc -l | tr -d ' ')"
  if [ "$n" != 1 ]; then note "'$w' is filed $n times in status.states (${got:-nowhere}); file it once, under $want"
  elif [ "$got" != "$want" ]; then note "'$w' is filed under $got; its role is $want"; fi
done
# --- one vocabulary in the contract itself (2026-10-30): its delivered section asked for
# VERIFIED / PARTIAL / BLOCKED in capitals and unbracketed two blocks above the rule that
# demands bracketed filed words, and the rule banned `done` and `OK`, which tokens.yaml files
contract="$ROOT/share/brief/report.yaml"
# (the sections and the rules, which the report is written by; an attention fact quotes the
# forge's own enum, `mergeStateStatus BLOCKED`, and is not a verdict of the report)
if grep -E '^    (holds|evidence|text): ' "$contract" | grep -nwE 'VERIFIED|PARTIAL|BLOCKED|PASSED|FAILED|DONE|OK'; then
  note "share/brief/report.yaml writes a verdict in capitals: write the filed word in brackets ([verified])"
fi
banned="$(sed -n 's/.*No other verdict adjective (\([^)]*\)).*/\1/p' "$contract" | tr ',' '\n' | sed 's/^ *//; s/ *$//')"
[ -n "$banned" ] || note "the status-words rule names no adjective it bans"
while IFS= read -r w; do
  [ -n "$w" ] || continue
  if printf '%s\n' "$filed" | awk -v w="$w" '$1 == w { found = 1 } END { exit !found }'; then
    note "the status-words rule bans '$w', which share/design/tokens.yaml files as a status word"
  fi
done <<EOF2
$banned
EOF2
[ "$fail" = 0 ] || exit 1

# --- the generator refuses a brief word filed under a second role (the fixture of case 501)
RB="$(rust_bin)" || rust_bin_exit $?
"$MJ" init >/dev/null
cp -R "$ROOT/share" share
mkdir -p apps/majordomus-cli
cp "$ROOT/apps/majordomus-cli/Cargo.toml" apps/majordomus-cli/Cargo.toml
git add -A >/dev/null && git commit -qm fixture
gen() { env -u MAJORDOMUS_SHARE "$RB" generate design --repo "$T" "$@"; }
decl=share/design/tokens.yaml
cp "$decl" "$S/tokens.yaml"
expect_exit 0 gen
sed 's/^    warn: \[warn, /    warn: [warn, unread, /' "$S/tokens.yaml" > "$decl"
grep -q '^    warn: \[warn, unread, ' "$decl" || { echo "    the mutation did not file 'unread' under warn"; exit 1; }
expect_exit 10 gen
expect_grep "'unread' is already filed under"
cp "$S/tokens.yaml" "$decl"
expect_exit 0 gen

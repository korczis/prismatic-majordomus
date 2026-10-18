# What the served surfaces change is said as the registry says it (I2962). The README, the
# interfaces feature and three guarantee pages called the server read-only, named "the one
# capability of kind command" and "the one mutating mesh operation", and said "all but two" of
# the capabilities are reads "counted off the registry". The registry held eight capabilities
# that write the repository, thirty-one commands and thirteen mesh POST routes. A sentence
# that promises a read-only surface is the sentence an agent acts on, so:
#   1. the interfaces feature names exactly the capabilities whose registry effect is
#      repository_mutation — one added to the registry fails this case until the page names it;
#   2. none of the retired sentences returns to the README, docs/ or the features.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
REG="$ROOT/docs/generated/registry.json"
PAGE="$ROOT/.ai/repo/features/interfaces.md"
expect_file "$REG"; expect_file "$PAGE"

# ---------------------------------------------------------------- 1. the page names the registry's set
named() { # <feature page> -> the backticked ids of its "The ones that write the repository are" sentence
  awk '/The ones that write the repository are/ { on = 1 } on { print } on && /;/ { exit }' "$1" \
    | grep -oE '`[a-z][a-z0-9_.]*`' | tr -d '`' | sort -u
}
registry="$(jq -r '.capabilities[] | select(.execution.effect == "repository_mutation") | .id' "$REG" | sort -u)"
[ -n "$registry" ] || { echo "    the registry names no repository_mutation capability: the read is wrong"; exit 1; }
page="$(named "$PAGE")"
[ -n "$page" ] || { echo "    the interfaces feature has no sentence naming what writes the repository"; exit 1; }
if [ "$registry" != "$page" ]; then
  echo "    the interfaces feature and the registry disagree on what writes the repository"
  printf '      registry only: %s\n' "$(comm -23 <(printf '%s\n' "$registry") <(printf '%s\n' "$page") | tr '\n' ' ')"
  printf '      page only:     %s\n' "$(comm -13 <(printf '%s\n' "$registry") <(printf '%s\n' "$page") | tr '\n' ' ')"
  exit 1
fi
# ... and the comparison is held: a page that drops one is refused
sed 's/`plan.transition`, //' "$PAGE" > "$T/interfaces.md"
[ "$(named "$T/interfaces.md")" != "$registry" ] || { echo "    a page missing plan.transition was not told apart"; exit 1; }

# ---------------------------------------------------------------- 2. the retired sentences stay retired
files="$ROOT/README.md $ROOT/docs/*.md $ROOT/docs/claims/*.md $ROOT/.ai/repo/features/*.md"
bad=0
for phrase in \
  'are read-only processes' \
  'the one capability of kind `command`' \
  'all but two of the capabilities' \
  'the one mutating mesh' \
  'read-only projection it always served' \
  "serving the repository's AI layer read-only" \
  'layer to programs, read-only' \
  'serves the layer read-only' \
  'to an MCP client over stdio, read-only'; do
  # shellcheck disable=SC2086
  hits="$(grep -nF -- "$phrase" $files 2>/dev/null || true)"
  if [ -n "$hits" ]; then
    printf '    a retired sentence is back: %s\n' "$phrase"
    printf '%s\n' "$hits" | sed "s|$ROOT/||; s/^/      /"
    bad=1
  fi
done
[ "$bad" = 0 ] || exit 1
exit 0

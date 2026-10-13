# majordomus-covers: none
# The intents on the site (I2658) are the records, read through the command the API answers
# from, and nothing else:
#
#   site/data/generated/intents.json   holds exactly the intents `majordomus intent list`
#                                      answers for this tree, each with the milestones and the
#                                      status the command gives them and with every criterion
#   site/content/intents/              one page per intent and no page for any other
#   what is not there                  no stage, no verdict, no state of any criterion: those
#                                      follow the evidence ledger, which goes stale with files
#                                      the site does not list among its inputs
#   the generator, the check, the nav  the records are inputs of the fingerprint, the site's
#                                      check has the section, the navigation leads to it
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || skip_case "no jq, so the projection was not read"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

J="$ROOT/site/data/generated/intents.json"
[ -f "$J" ] || { echo "    $J is missing; run scripts/generate-site-data"; exit 1; }
"$RB" intent list --repo "$ROOT" --format json > live.json 2> live.err \
  || { echo "    intent list did not answer:"; sed 's/^/      /' live.err; exit 1; }

# ---------------------------------------------------------------- the same intents
jq -S '[.intents[] | {id, title, statement,
        milestones: [(.milestones // [])[] | {id, resolved, status: (.status // "")}],
        criteria: [(.satisfaction // [])[] | {id, criterion, evidence, ref: (.ref // ""), reproduce: (.reproduce // "")}]}]' live.json > want.json
jq -S '[.intents[] | {id, title, statement, milestones, criteria}]' "$J" > have.json
cmp -s want.json have.json || {
  echo "    the site's intents are not the ones the command answers; run scripts/generate-site-data"
  diff want.json have.json | head -20 | sed 's/^/      /'; exit 1; }
[ "$(jq '.count' "$J")" = "$(jq '.intents | length' live.json)" ] || { echo "    the count is not the number of intents"; exit 1; }

# ---------------------------------------------------------------- nothing the ledger decides
leaked="$(jq -r '[paths | map(tostring) | join(".")] | map(select(test("(^|\\.)(stage|verdict|state|met|proof)$"))) | .[]' "$J" | head -5)"
[ -z "$leaked" ] || { echo "    the projection carries what the evidence ledger decides: $leaked"; exit 1; }

# ---------------------------------------------------------------- a page each, and no other
D="$ROOT/site/content/intents"
[ -f "$D/_index.md" ] || { echo "    the intents section has no index"; exit 1; }
for id in $(jq -r '.intents[].id' "$J"); do
  [ -f "$D/$id.md" ] || { echo "    no page for intent $id"; exit 1; }
  grep -qF "id = \"$id\"" "$D/$id.md" || { echo "    the page of $id does not carry its id"; exit 1; }
  grep -qF "$(jq -r --arg i "$id" '.intents[] | select(.id==$i) | .path' "$J")" "$D/$id.md" \
    || { echo "    the page of $id does not name its canonical file"; exit 1; }
done
for page in "$D"/*.md; do
  id="$(basename "$page" .md)"; [ "$id" = _index ] && continue
  jq -e --arg i "$id" '.intents[] | select(.id==$i)' "$J" >/dev/null \
    || { echo "    a page exists for '$id', which the records do not hold"; exit 1; }
done

# ---------------------------------------------------------------- the generator, the check, the nav
bash "$ROOT/scripts/generate-site-data" --inputs > inputs.txt
for f in "$ROOT"/.ai/repo/project/intents/*.yaml; do
  grep -qxF "${f#"$ROOT"/}" inputs.txt || { echo "    ${f#"$ROOT"/} is not an input of the site's fingerprint"; exit 1; }
done
grep -qF 'intent list --format json' "$ROOT/scripts/generate-site-data" \
  || { echo "    the generator does not read the intents through the command"; exit 1; }
grep -qF 'bad intents' "$ROOT/scripts/site-check" || { echo "    the site's check has no intents section"; exit 1; }
grep -qF 'href = "/intents/"' "$ROOT/site/data/nav.toml" || { echo "    the navigation does not lead to /intents/"; exit 1; }
grep -qF 'href = "/docs/idd/"' "$ROOT/site/data/nav.toml" || { echo "    the navigation does not lead to the method"; exit 1; }
[ -f "$ROOT/docs/IDD.md" ] || { echo "    docs/IDD.md is missing"; exit 1; }
for t in intents-section.html intent.html; do
  [ -f "$ROOT/site/templates/$t" ] || { echo "    the template $t is missing"; exit 1; }
done
# the template reads nothing the projection does not carry
if grep -qE '\{\{[^}]*\.(stage|verdict|state|met|proof)[^a-z_]' "$ROOT/site/templates/intent.html"; then
  echo "    the intent template prints a state the site cannot know"; exit 1
fi

echo "    $(jq '.count' "$J") intent(s): the site holds what the command answers, a page each and no other, and nothing the ledger decides"

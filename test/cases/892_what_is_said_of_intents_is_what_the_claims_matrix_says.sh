# majordomus-covers: none
# claims: none
# What is said of the intent model is what docs/CLAIMS.yaml says of it.
#
# The homepage's account of the intent model was prose beside the matrix, and it drifted both
# ways at once. It said the Cockpit had no intent view after /cockpit/intents had shipped, and
# that work starting before the plan's critique was refused while `plan start` let it through
# and only `intent validate` named it afterwards. The guide called intents branch only and the
# glossary called them open pull requests. Every sentence was written once and never read
# against anything.
#
# Two things now hold it, and this case pins both against this checkout:
#
#   derived    the homepage lists the intent claims under the status docs/CLAIMS.yaml declares,
#              from the derived data, so the built and unbuilt halves cannot be typed beside
#              the matrix; scripts/site-check compares the built page with the data, and here
#              the template, the data and the matrix are held to one another without a build
#   prose      a hand-written sentence that is true only while an intent claim has a status
#              other than the one it has is a contradiction, named with the claim, the status
#              it presumes and the file. The table runs in both directions: a planned claim
#              that lands makes the sentences describing its absence wrong, and those are
#              named here too, so promoting a claim and leaving the prose behind fails.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || skip "jq absent"
cd "$ROOT" || exit 1

M=site/data/manifesto.toml
C=docs/CLAIMS.yaml
D=site/data/generated/capabilities.json
TPL=site/templates/index.html

# --- the matrix: every claim of the prefix the homepage names, with the status it declares
prefix="$(sed -n 's/^claim_prefix = "\(.*\)"$/\1/p' "$M")"
[ -n "$prefix" ] || { echo "    $M names no claim_prefix: the homepage's intent lists would be prose again"; exit 1; }
pairs="$(awk -v p="$prefix" '
  /^  - id: / { id = $3; inside = (index(id, p) == 1); next }
  inside && /^    status: / { print id " " $2; inside = 0 }' "$C")"
declared="$(awk '/^statuses:/ { s = 1; next } /^claims:/ { s = 0 } s && /^  - id: / { print $3 }' "$C")"
printf '%s\n' "$pairs" | grep -q ' guaranteed$' || { echo "    no guaranteed claim carries the prefix '$prefix'"; exit 1; }
printf '%s\n' "$pairs" | grep -q ' planned$'    || { echo "    no planned claim carries the prefix '$prefix'"; exit 1; }
while read -r id st; do
  printf '%s\n' "$declared" | grep -qxF "$st" || { echo "    $id has the status '$st', which $C does not declare"; exit 1; }
  # the data the homepage renders carries the claim under the same status
  got="$(jq -r --arg i "$id" '.claims[] | select(.id == $i) | .status' "$D")"
  [ "$got" = "$st" ] || { echo "    $D gives $id the status '${got:-nothing}', $C gives '$st'; run scripts/derive"; exit 1; }
done <<EOF
$pairs
EOF

# --- the homepage renders the claims rather than a sentence about them
expect_grep 'caps\.claims if c\.id is starting_with\(pat=x\.direction\.claim_prefix\)' "$TPL"
expect_grep 'data-direction-claim="\{\{ c\.id \}\}"' "$TPL"
# and the lead above the lists names no command: a command in it is a capability stated
# beside the matrix, which is how the list it replaced began
lead="$(sed -n 's/^planned_lead = "\(.*\)"$/\1/p' "$M")"
case "$lead" in *'`'*) echo "    planned_lead names a command; the claims under it say what is built"; exit 1 ;; esac

# --- the prose: claim, the status a sentence presumes, and the sentence
# A sentence is the text from one full stop to the next, with the file's lines joined, so a
# sentence wrapped across lines is still one sentence.
corpus="$(printf '%s\n' README.md docs/*.md site/data/*.toml | grep -vxE 'docs/(SITE_CLAIMS|PLAN_STATUS|PAGES_STATUS)\.md'
  find site/content-src .ai/repo/features .ai/repo/why -name '*.md' 2>/dev/null)"
TAB="$(printf '\t')"
fails=0
while IFS="$TAB" read -r id presumes ere; do
  [ -n "$id" ] || continue
  st="$(printf '%s\n' "$pairs" | awk -v i="$id" '$1 == i { print $2 }')"
  [ -n "$st" ] || { echo "    the table names $id, which $C does not hold under '$prefix'"; exit 1; }
  [ "$st" = "$presumes" ] && continue
  for f in $corpus; do
    hit="$(tr '\n' ' ' < "$f" | grep -oE "[^.]*($ere)[^.]*" | head -1)" || true
    [ -z "$hit" ] && continue
    printf '    %s says what is true only while %s is %s; %s declares it %s:\n      %s\n' \
      "$f" "$id" "$presumes" "$C" "$st" "$(printf '%s' "$hit" | tr -s ' ' | cut -c1-200)"
    fails=$((fails + 1))
  done
done <<'EOF'
intent-stage-derived	planned	[Ii]ntents?[^.]*(branch only|not on the default branch|open pull requests)
intent-cockpit-pages	planned	Cockpit has no intent view
intent-criteria-covered	planned	the intent above them is not|none of them are questions this repository can answer
intent-refused-at-plan-start	guaranteed	starts before the critique is refused|[Ww]ork does not start before the plan has been reviewed|must resolve its findings before work starts
intent-refused-at-plan-start	planned	`plan start` (itself )?does not refuse|`plan start` lets|`plan start` succeeds
intent-in-session-context	guaranteed	session loads (the|its|an) intent
intent-in-session-context	planned	session does not load (the |an )?intents?
intent-closes-github-milestones	guaranteed	GitHub milestones are reconciled against intents
intent-closes-github-milestones	planned	closes a milestone without reading|GitHub milestones are not reconciled|milestones close without reading
intent-command-deployment-evidence	planned	`command` or `deployment` criterion is never met
EOF
[ "$fails" -eq 0 ] || { echo "    $fails sentence(s) contradict $C"; exit 1; }

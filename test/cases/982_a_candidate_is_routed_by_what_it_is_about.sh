# majordomus-covers: knowledge capture
# majordomus-negative: knowledge
# claim: a-candidate-is-routed-by-what-it-is-about
# Every observation candidate says which owner should look first, read from what it is about
# and never from its words; and in a repository that adopted the tool, the project's own
# share/ is the project's (ADR 0118).
#
# The repository here is an adopter: `init` gave it the layer, and it has a share/ directory
# of its own, which is not the tool. One episode observes one subject of each structure:
#
#   command:finish, capability:intents.binding, rule:majordomus.*,
#   a vendored rule file                              -> platform  (the tool itself)
#   rule:project.*, gate:*                            -> enforcement
#   a path the scope declares generated (by name)     -> generator
#   docs/...                                          -> documentation
#   share/..., lib/...                                -> project   (the adopter's own)
#
# Two observations about one subject in different words carry the same route, and deriving
# writes no event the deriver reads, so a derivation never starts another.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "${TMPDIR:-/tmp}/mj.pol.$$" \
  && cp "${TMPDIR:-/tmp}/mj.pol.$$" .ai/repo/policy.yaml && rm -f "${TMPDIR:-/tmp}/mj.pol.$$"
"$MJ" capture install >/dev/null
mkdir -p share lib docs build
printf 'adopter data\n' > share/data.txt; printf 'echo a\n' > lib/a.sh; printf '# Guide\n' > docs/guide.md
# the skeleton's scope declares generated names (`*.generated.*` among them); a file of
# that name is the generator's
git add -A >/dev/null 2>&1; git commit -qm "an adopter with a share/ of its own" >/dev/null 2>&1 || true
PATH="$(dirname "$MJ"):$PATH"; export PATH

STATE=.ai/local/state
LEDGER="$STATE/ledger.jsonl"
CAND=.ai/repo/knowledge/candidates
VENDORED="$(ls .ai/repo/rules/vendor/majordomus/rules/*.md 2>/dev/null | head -n 1)"
[ -n "$VENDORED" ] || { echo "    init vendored no rule to observe"; exit 1; }
printf '{"session_id":"r1","source":"startup"}' | ./.claude/hooks/majordomus-session-start >/dev/null 2>&1
SID="$(sed -n 's/^session_id: //p' "$STATE/sessions-open/r1.yaml" 2>/dev/null | head -n 1)"
[ -n "$SID" ] || { echo "    the start event opened no episode"; exit 1; }

# subject <TAB> the route it must carry
cat > "${TMPDIR:-/tmp}/mj.routes.$$" <<ROUTES
command:finish	platform
capability:intents.binding	platform
rule:majordomus.knowledge-observed	platform
$VENDORED	platform
rule:project.alpha	enforcement
gate:intent-check	enforcement
build/report.generated.json	generator
docs/guide.md	documentation
share/data.txt	project
lib/a.sh	project
ROUTES
derived_before="$(grep -c '"event":"knowledge.derived"' "$LEDGER" 2>/dev/null || true)"
while IFS="$(printf '\t')" read -r subject route; do
  "$MJ" knowledge observe --kind friction --subject "$subject" "first words about $subject" >/dev/null \
    || { echo "    the subject $subject was refused"; exit 1; }
done < "${TMPDIR:-/tmp}/mj.routes.$$"
"$MJ" knowledge observe --kind defect --subject lib/a.sh "entirely different words about the same file" >/dev/null
printf '{"session_id":"r1","reason":"clear"}' | ./.claude/hooks/majordomus-session-end >/dev/null 2>&1

while IFS="$(printf '\t')" read -r subject route; do
  got="$(grep -l "^about: \"$subject\"\$" "$CAND"/"$SID"-*.md 2>/dev/null | xargs -n1 sed -n 's/^route: //p' | sort -u | tr '\n' ' ')"
  [ "$got" = "$route " ] || { echo "    $subject routed '${got% }', expected $route"; exit 1; }
done < "${TMPDIR:-/tmp}/mj.routes.$$"
rm -f "${TMPDIR:-/tmp}/mj.routes.$$"
[ "$(grep -l '^about: "lib/a.sh"$' "$CAND"/"$SID"-*.md | wc -l | tr -d ' ')" = 2 ] \
  || { echo "    two observations of lib/a.sh should be two candidates"; exit 1; }

# one derivation for the one episode: the deriver started no other
derived_after="$(grep -c '"event":"knowledge.derived"' "$LEDGER" 2>/dev/null || true)"
[ "$((derived_after - derived_before))" = 1 ] \
  || { echo "    the end event derived $((derived_after - derived_before)) times; a derivation must not start another"; exit 1; }
"$MJ" knowledge check >/dev/null 2>&1 || { echo "    the routed candidates fail the integrity check:"; "$MJ" knowledge check 2>&1 | sed 's/^/    | /'; exit 1; }
exit 0

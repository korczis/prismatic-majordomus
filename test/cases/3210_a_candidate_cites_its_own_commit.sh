# majordomus-covers: knowledge decision
# A knowledge candidate cites what it is about, not the whole episode it came from (I1985).
#
# Each candidate's provenance names the commit its own ledger line was written at, and its
# relations name the rules and decision files that commit changed. It used to cite every
# commit the episode made under the rules and the ADRs, and relate to every file they
# touched, so in one adopter every candidate of an episode carried the same 52 relates_to
# targets and a decision of 10-01 cited commits of 10-04. And a decision a later one
# superseded yields no candidate: the standing decision is the knowledge, not both.
#
# The fixture is one episode with three decisions:
#   D1 recorded at commit A, which changed .ai/repo/rules/alpha.md;
#   D2 recorded at commit B, which changed .ai/repo/rules/beta.md;
#   D3 recorded at B too, superseding D2.
# One derivation then writes D1 and D3, each citing only its own commit and file, and
# writes nothing for D2.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "${TMPDIR:-/tmp}/mj.pol.$$" \
  && cp "${TMPDIR:-/tmp}/mj.pol.$$" .ai/repo/policy.yaml && rm -f "${TMPDIR:-/tmp}/mj.pol.$$"
"$MJ" capture install >/dev/null
git add -A >/dev/null 2>&1; git commit -qm "the layer and the hooks" >/dev/null 2>&1 || true
PATH="$(dirname "$MJ"):$PATH"; export PATH

STATE=.ai/local/state
CAND=.ai/repo/knowledge/candidates
start_event() { printf '{"session_id":"%s","source":"startup"}' "$1" | ./.claude/hooks/majordomus-session-start; }
sid_of() { sed -n 's/^session_id: //p' "$STATE/sessions-open/$1.yaml" | head -n 1; }
# the candidate whose title is exactly $1
record_of() { grep -lFx -- "title: \"$1\"" "$CAND"/*.md 2>/dev/null | head -n 1; }
# the derived_from commits and the relates_to targets of one record, one per line
commits_of() { sed -n 's/^    - commit://p' "$1"; }
targets_of() { sed -n 's/^    target: //p' "$1"; }

start_event e1 >/dev/null 2>&1
SID="$(sid_of e1)"
[ -n "$SID" ] || { echo "    the start event opened no episode"; exit 1; }
"$MJ" start "two decisions at two commits" --scope .ai/repo/rules >/dev/null

mkdir -p .ai/repo/rules
printf '# alpha\n' > .ai/repo/rules/alpha.md
git add .ai/repo/rules/alpha.md >/dev/null; git commit -qm "alpha" >/dev/null
A="$(git rev-parse HEAD)"
"$MJ" decision add "Alpha is decided at A" --why "the first commit carries it" >/dev/null

printf '# beta\n' > .ai/repo/rules/beta.md
git add .ai/repo/rules/beta.md >/dev/null; git commit -qm "beta" >/dev/null
B="$(git rev-parse HEAD)"
"$MJ" decision add "Beta is decided at B" --why "the second commit carries it" >/dev/null
"$MJ" decision add "Beta is decided again at B" --why "the first wording was wrong" \
  --supersedes "Beta is decided at B" >/dev/null

expect_exit 0 "$MJ" knowledge derive
printf '%s\n' "$LAST_OUT" | sed 's/^/    | /'

# --- each candidate cites only its own commit and the file that commit changed
# MUTATION, RUN: cite every commit of the episode again (`sed -n 's/^commit:/    - commit:/p'`
# over the episode's reference list in mj_knowledge_compose) -> D1 cites B as well.
D1="$(record_of "Alpha is decided at A")"
[ -n "$D1" ] || { echo "    no candidate for the decision recorded at A"; exit 1; }
[ "$(commits_of "$D1")" = "$A" ] \
  || { echo "    the decision recorded at A cites [$(commits_of "$D1" | paste -sd, -)], expected only $A"; exit 1; }
[ "$(targets_of "$D1")" = "file:.ai/repo/rules/alpha.md" ] \
  || { echo "    the decision recorded at A relates to [$(targets_of "$D1" | paste -sd, -)], expected only alpha.md"; exit 1; }

D3="$(record_of "Beta is decided again at B")"
[ -n "$D3" ] || { echo "    no candidate for the standing decision recorded at B"; exit 1; }
[ "$(commits_of "$D3")" = "$B" ] \
  || { echo "    the decision recorded at B cites [$(commits_of "$D3" | paste -sd, -)], expected only $B"; exit 1; }
[ "$(targets_of "$D3")" = "file:.ai/repo/rules/beta.md" ] \
  || { echo "    the decision recorded at B relates to [$(targets_of "$D3" | paste -sd, -)], expected only beta.md"; exit 1; }

# --- a superseded decision yields no candidate, and the derivation says why
# MUTATION, RUN: drop the supersession skip in mj_knowledge_derive_cmd -> D2 is written.
[ -z "$(record_of "Beta is decided at B")" ] || { echo "    the superseded decision was written as a candidate"; exit 1; }
expect_grep "^skipped $CAND/$SID-[0-9a-f]+\.md \(decision superseded\)$"
expect_grep "^knowledge derive: 2 written, 0 unchanged, 1 skipped for episode $SID"

# --- the candidates still pass the knowledge validator
expect_exit 0 "$MJ" knowledge check

# --- a second derivation over the same evidence writes nothing (the ids and bytes hold)
expect_exit 0 "$MJ" knowledge derive
expect_grep "^knowledge derive: 0 written, 2 unchanged, 1 skipped for episode $SID"

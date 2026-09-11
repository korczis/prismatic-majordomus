# The context compiler, against the real executable in a disposable repository: what it
# selects and why, what it refuses and why, that the same request answers identically on
# every surface, and that a second run over one tree is the same answer.
#
# The load-bearing assertion is the cross-surface one. A context compiler that answered
# differently on the command line, over HTTP and over MCP would not be one compiler with
# three projections — it would be three compilers, and a session would be given different
# context depending on how it asked. So the same request is put to all three and the
# selected identifiers, the exclusions and the budget are compared byte for byte.
#
# The rest is the contract the answer makes: every entry names the selector that reached it
# and why, an inference is marked as one and carries a confidence below one, what the budget
# could not afford is listed with what it would have cost, a superseded decision leaves with
# its reason, one object reached several ways is one entry with a record of the fold, and a
# source the repository does not hold is a refusal rather than a silently smaller answer.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    skip: no jq"; exit 0; }
command -v curl >/dev/null 2>&1 || { echo "    skip: no curl"; exit 0; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

S="$(mktemp -d "${TMPDIR:-/tmp}/mj130.XXXXXX")"
SRV=""
trap 'rm -rf "$S"; [ -n "$SRV" ] && kill "$SRV" 2>/dev/null; :' EXIT

"$MJ" init >/dev/null

# --- a repository with a plan, a decision that supersedes another, and code in scope
pj_init
pj_milestone m1 0
pj_issue I0001 m1
pj_issue I0002 m1 I0001
# the issue's scope is `src/I0001`; put something under it that the index holds
mkdir -p src/I0001 lib test/cases
cat > lib/thing.sh <<'SH'
#!/usr/bin/env bash
mj_thing() { :; }
SH
cat > test/cases/01_thing.sh <<'SH'
#!/usr/bin/env bash
set -eu
SH
mkdir -p .ai/repo/adrs
cat > .ai/repo/adrs/0001-first.md <<'MD'
---
schema: adr/v1
id: adr-0001
kind: adr
title: The first decision
status: accepted
date: 2026-09-10
tags:
  - context
---

## Context

Something had to be decided.

## Decision

It was decided.

## Consequences

Things follow from it.
MD
cat > .ai/repo/adrs/0002-second.md <<'MD'
---
schema: adr/v1
id: adr-0002
kind: adr
title: The decision that stands in for the first
status: accepted
date: 2026-09-11
tags:
  - context
supersedes:
  - adr-0001
---

## Context

The first decision did not survive contact.

## Decision

This one stands in for it.

## Consequences

The first is no longer what applies.
MD
git add -A >/dev/null && git commit -qm plan

# --- the compiler's own rules are readable, and the refusal is state rather than an absence
expect_exit 0 "$RB" devcontext policy --repo "$PWD"
expect_grep 'TIER'
expect_grep 'task'
expect_grep 'governance'
expect_grep 'is_a'
expect_grep 'REFUSED'
# the one inference the compiler makes says that it is one
expect_grep 'intent_match'
"$RB" devcontext policy --repo "$PWD" --format json > "$S/policy.json"
jq -e '.tiers | length == 6 and (.[0].tier == "task") and (.[0].position == 1)' "$S/policy.json" >/dev/null \
  || { echo "    the tiers are not the order the budget spends in"; exit 1; }
jq -e '[.edges[] | select(.edge == "is_a")] | length == 1 and .[0].refused' "$S/policy.json" >/dev/null \
  || { echo "    the hub edge is not reported as refused"; exit 1; }
jq -e '[.selectors[] | select(.declared | not)] | length >= 1' "$S/policy.json" >/dev/null \
  || { echo "    no selector is marked as inferring"; exit 1; }

# --- compiling for one issue: the seed, its milestone, and the reason for each
expect_exit 0 "$RB" devcontext compile --repo "$PWD" --issue I0001 --budget-tokens 1000000
expect_grep 'SELECTED'
expect_grep 'majordomus://issue/I0001'
expect_grep 'EXCLUDED'
expect_grep 'seed named in the request'
expect_grep 'the milestone this work belongs to'

"$RB" devcontext compile --repo "$PWD" --issue I0001 --budget-tokens 1000000 --format json > "$S/a.json"
# every entry keeps its canonical identifier, its provenance, a reason and a cost
jq -e '.selected | length > 0' "$S/a.json" >/dev/null || { echo "    nothing was selected"; exit 1; }
jq -e '.selected | all(
         (.uri | startswith("majordomus://") or startswith("local:")) and
         (.provenance.path | length > 0) and
         (.provenance.source_class | length > 0) and
         (.cost_tokens > 0) and
         (.tier | length > 0) and
         (.confidence >= 0 and .confidence <= 1) and
         (.relevance >= 0 and .relevance <= 1) and
         (.discovered_by | length > 0) and
         (.discovered_by | all(.reason | length > 0)))' "$S/a.json" >/dev/null \
  || { echo "    an entry is missing its identifier, provenance, reason, confidence or cost"; jq '.selected[] | select((.discovered_by|length)==0 or (.cost_tokens|.<=0))' "$S/a.json"; exit 1; }
# the seed is in the task tier, is required, and says it was named
jq -e '[.selected[] | select(.uri == "majordomus://issue/I0001")] | length == 1
        and .[0].tier == "task" and .[0].required and .[0].relevance == 1
        and ([.[0].discovered_by[].selector] | index("seed") != null)' "$S/a.json" >/dev/null \
  || { echo "    the seed is not in its own answer as a required task entry"; exit 1; }
# the milestone came along the declared edge of the composed graph, not from a guess
jq -e '[.selected[] | select(.kind == "milestone")] | length == 1
        and ([.[0].discovered_by[] | select(.edge == "belongs_to")] | length == 1)
        and ([.[0].discovered_by[] | select(.edge == "belongs_to")][0].confidence == 1)' "$S/a.json" >/dev/null \
  || { echo "    the milestone was not reached along belongs_to with full confidence"; jq '.selected[]|select(.kind=="milestone")' "$S/a.json"; exit 1; }
# the answer names the tree it is true of
jq -e '.fingerprint | length > 0' "$S/a.json" >/dev/null || { echo "    the answer names no index"; exit 1; }
jq -e '.git.head | length > 0' "$S/a.json" >/dev/null || { echo "    the answer names no head"; exit 1; }

# --- deterministic: the same tree and the same request twice
"$RB" devcontext compile --repo "$PWD" --issue I0001 --budget-tokens 1000000 --format json > "$S/b.json"
cmp -s "$S/a.json" "$S/b.json" || { echo "    two runs over one tree disagreed"; diff "$S/a.json" "$S/b.json" | head -20; exit 1; }

# --- the order is total: tier, then relevance descending, then identifier
jq -r '.selected[] | "\(.tier)\t\(.relevance)\t\(.uri)"' "$S/a.json" > "$S/order.txt"
awk -F'\t' '
  BEGIN { split("task governance source decision knowledge history", w, " "); for (i in w) rank[w[i]] = i }
  NR > 1 {
    if (rank[$1] < rank[pt]) { print "tier out of order at line " NR; exit 1 }
    if (rank[$1] == rank[pt] && $2 + 0 > pr + 0) { print "relevance out of order at line " NR; exit 1 }
    if (rank[$1] == rank[pt] && $2 + 0 == pr + 0 && $3 < pu) { print "identifier out of order at line " NR; exit 1 }
  }
  { pt = $1; pr = $2; pu = $3 }
' "$S/order.txt" > "$S/order.err" || true
# `[ ... ] && { ... }` would return 1 when the file is empty, and every case runs under
# `bash -eu`: the healthy path would abort the case without a word. An `if` cannot.
if [ -s "$S/order.err" ]; then
  echo "    the selection is not in one total order:"; cat "$S/order.err"; exit 1
fi

# --- the budget: what did not fit is named, with what it would have cost, and the exit
# code is the shell tool's own code for the same condition
rc=0; "$RB" devcontext compile --repo "$PWD" --issue I0001 --budget-tokens 1 --format json > "$S/tight.json" || rc=$?
[ "$rc" = 10 ] || { echo "    a budget that cannot hold what it must keep exited $rc, not 10"; exit 1; }
jq -e '.budget.over_budget and .budget.used_tokens > .budget.limit_tokens' "$S/tight.json" >/dev/null \
  || { echo "    the answer exceeded its ceiling and did not say so"; jq .budget "$S/tight.json"; exit 1; }
jq -e '[.excluded[] | select(.reason == "budget")] | length > 0
        and all(.[]; .cost_tokens > 0 and (.detail | length > 0))' "$S/tight.json" >/dev/null \
  || { echo "    nothing was dropped for budget, or dropped without a cost and a reason"; exit 1; }
# what may not be dropped is still there
jq -e '[.selected[] | select(.required)] | length > 0' "$S/tight.json" >/dev/null \
  || { echo "    a one-token budget dropped what it may not drop"; exit 1; }
# and the per-tier spend adds up to what was used
jq -e '([.budget.tiers[].tokens] | add) == .budget.used_tokens' "$S/tight.json" >/dev/null \
  || { echo "    the per-tier spend does not add up to the total"; exit 1; }

# a generous budget keeps strictly more than a tight one
jq -e --argjson n "$(jq '.selected | length' "$S/tight.json")" '(.selected | length) > $n' "$S/a.json" >/dev/null \
  || { echo "    a larger budget did not buy more context"; exit 1; }

# --- deduplication: one object reached several ways is one entry, with the fold recorded
jq -e '[.selected[].uri] | length == ([.selected[].uri] | unique | length)' "$S/a.json" >/dev/null \
  || { echo "    an identifier is in the answer twice"; exit 1; }
"$RB" devcontext compile --repo "$PWD" --issue I0001 --intent 'the first decision and the context it needs' \
  --budget-tokens 1000000 --format json > "$S/dedup.json"
jq -e '[.selected[] | select(.discovered_by | length > 1)] | length > 0' "$S/dedup.json" >/dev/null \
  || { echo "    nothing was reached by more than one path; the case proves nothing"; exit 1; }
jq -e '. as $d | [$d.selected[] | select(.discovered_by | length > 1)] | all(
         .uri as $u | ($d.deduplicated | map(select(.kept == $u and .key == "uri")) | length) == 1)' "$S/dedup.json" >/dev/null \
  || { echo "    an entry was folded with no record of the fold"; exit 1; }
# and the fold names the ways in
jq -e '[.deduplicated[] | select(.key == "uri")] | all(.detail | contains("discovery path"))' "$S/dedup.json" >/dev/null \
  || { echo "    a fold does not say what reached it"; exit 1; }

# --- an inference is marked as one, and never as confident as a declaration
jq -e '[.selected[] | select([.discovered_by[].selector] | index("intent_match") != null)] | length > 0' "$S/dedup.json" >/dev/null \
  || { echo "    the intent matched nothing"; exit 1; }
jq -e '.selected | all(.discovered_by[] | select(.selector == "intent_match") | .confidence < 1 or .confidence == 1)' "$S/dedup.json" >/dev/null \
  || { echo "    an inferred discovery carries no confidence"; exit 1; }
jq -e '[.selected[] | select([.discovered_by[].selector] | unique == ["intent_match"])] | all(.relevance < 1)' "$S/dedup.json" >/dev/null \
  || { echo "    something reached only by inference is as relevant as a seed"; exit 1; }

# --- conflicting sources: a later decision stands in for an earlier one, and it is said
"$RB" devcontext compile --repo "$PWD" --uri majordomus://adr/adr-0002 --budget-tokens 1000000 --format json > "$S/adr.json"
jq -e '[.conflicts[] | select(.kind == "supersedes")] | length == 1
        and (.[0].current | endswith("adr-0002")) and (.[0].against | endswith("adr-0001"))' "$S/adr.json" >/dev/null \
  || { echo "    the superseding decision did not stand against the one it replaces"; jq .conflicts "$S/adr.json"; exit 1; }
jq -e '[.selected[] | select(.uri | endswith("adr-0001"))] | length == 0' "$S/adr.json" >/dev/null \
  || { echo "    the superseded decision is still in the answer"; exit 1; }
jq -e '[.excluded[] | select(.uri | endswith("adr-0001") and (. != null))] | length == 1
        and .[0].reason == "superseded"' "$S/adr.json" >/dev/null \
  || { echo "    the superseded decision left without a reason"; jq .excluded "$S/adr.json"; exit 1; }

# --- a source the repository does not hold is a refusal, not a smaller answer
expect_exit 12 "$RB" devcontext compile --repo "$PWD" --issue I9999
expect_grep 'I9999'
expect_exit 12 "$RB" devcontext compile --repo "$PWD" --milestone nothing-like-this
expect_exit 12 "$RB" devcontext compile --repo "$PWD" --uri majordomus://rule/nothing-like-this@1
# and a path that is not repository-relative is refused before anything is read
rc=0; "$RB" devcontext compile --repo "$PWD" --path ../outside >/dev/null 2>&1 || rc=$?
[ "$rc" != 0 ] || { echo "    a path outside the repository was accepted as scope"; exit 1; }

# --- explaining one identifier, under the same request
expect_exit 0 "$RB" devcontext explain --repo "$PWD" majordomus://issue/I0001 --issue I0001
expect_grep 'selected'
expect_grep 'reached by'
"$RB" devcontext explain --repo "$PWD" majordomus://rule/nothing-like-this@1 --format json > "$S/unknown.json"
jq -e '.standing == "unknown" and (.detail | contains("holds nothing"))' "$S/unknown.json" >/dev/null \
  || { echo "    an identifier the index does not hold was not reported as unknown"; exit 1; }
# something reached and dropped for budget explains itself as that, with the budget attached
u="$(jq -r '[.excluded[] | select(.reason == "budget")][0].uri' "$S/tight.json")"
if [ -z "$u" ] || [ "$u" = null ]; then echo "    nothing was dropped for budget to explain"; exit 1; fi
"$RB" devcontext explain --repo "$PWD" "$u" --issue I0001 --budget-tokens 1 --format json > "$S/why.json"
jq -e '.standing == "excluded" and .excluded.reason == "budget" and .budget.limit_tokens == 1' "$S/why.json" >/dev/null \
  || { echo "    an entry dropped for budget did not explain itself as one"; cat "$S/why.json"; exit 1; }

# --- a changed tree is a different answer
before="$(jq -r .fingerprint "$S/a.json")"
printf '\n# one more line\n' >> lib/thing.sh
git add -A >/dev/null && git commit -qm touch
"$RB" devcontext compile --repo "$PWD" --issue I0001 --budget-tokens 1000000 --format json > "$S/moved.json"
after="$(jq -r .fingerprint "$S/moved.json")"
[ "$before" != "$after" ] || { echo "    the tree changed and the answer claims the same index"; exit 1; }
# the head moved too, and the answer carries the new one
[ "$(jq -r .git.head "$S/moved.json")" = "$(git rev-parse HEAD)" ] \
  || { echo "    the answer does not name the head it was compiled against"; exit 1; }

# ==========================================================================================
# The same request, on every surface. Three projections of one declaration must answer
# identically; a difference here means the compiler has been implemented more than once.
# ==========================================================================================
CLI="$S/surface-cli.json"; HTTP="$S/surface-http.json"; MCP="$S/surface-mcp.json"
# the fields compared: what was selected and why, what was refused and why, and the budget.
# The fingerprint and the head are compared too — three surfaces reading two different trees
# would agree about nothing worth agreeing about.
norm() { jq -S '{fingerprint, git, seeds, selected, excluded, deduplicated, conflicts, budget}' "$1"; }

"$RB" devcontext compile --repo "$PWD" --issue I0001 --budget-tokens 4000 --format json > "$S/cli.raw"
norm "$S/cli.raw" > "$CLI"

# --- over HTTP
"$RB" serve --repo "$PWD" --port 0 > "$S/serve.out" 2> "$S/serve.err" & SRV=$!
i=0
until grep -q 'listening on http://' "$S/serve.err" 2>/dev/null; do
  i=$((i+1)); [ "$i" -lt 300 ] || { echo "    the server never listened"; cat "$S/serve.err"; exit 1; }
  kill -0 "$SRV" 2>/dev/null || { echo "    the server exited before listening"; cat "$S/serve.err"; exit 1; }
  sleep 0.1
done
U="$(sed -n 's#.*listening on \(http://127\.0\.0\.1:[0-9]*\).*#\1#p' "$S/serve.err" | head -n 1)"
[ -n "$U" ] || { echo "    no URL on the listening line"; exit 1; }
curl -s -m 30 "$U/api/v1/devcontext?issue=I0001&budget_tokens=4000" > "$S/http.raw" \
  || { echo "    the HTTP route did not answer"; exit 1; }
jq -e '.selected' "$S/http.raw" >/dev/null 2>&1 \
  || { echo "    the HTTP route answered something that is not a compiled context:"; head -c 400 "$S/http.raw"; exit 1; }
norm "$S/http.raw" > "$HTTP"
kill "$SRV" 2>/dev/null || true; wait "$SRV" 2>/dev/null || true; SRV=""

# --- over MCP
req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
{
  req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case130","version":"0"}}'
  req 2 tools/call '{"name":"majordomus_devcontext","arguments":{"issue":"I0001","budget_tokens":4000}}'
} > "$S/mcp.in"
rc=0; "$RB" mcp --standalone --repo "$PWD" < "$S/mcp.in" > "$S/mcp.out" 2>/dev/null || rc=$?
[ "$rc" = 0 ] || { echo "    mcp exited $rc"; exit 1; }
sed -n 2p "$S/mcp.out" | jq -e '.result.structuredContent.selected' >/dev/null 2>&1 \
  || { echo "    the MCP tool answered no compiled context:"; sed -n 2p "$S/mcp.out" | head -c 400; exit 1; }
sed -n 2p "$S/mcp.out" | jq '.result.structuredContent' > "$S/mcp.raw"
norm "$S/mcp.raw" > "$MCP"

cmp -s "$CLI" "$HTTP" || { echo "    the command line and the HTTP route compiled different context:"; diff "$CLI" "$HTTP" | head -30; exit 1; }
cmp -s "$CLI" "$MCP" || { echo "    the command line and the MCP tool compiled different context:"; diff "$CLI" "$MCP" | head -30; exit 1; }

# --- resolving a path answers the same on every surface too, which is the question a
# session actually asks: "what applies to the file I am about to edit"
CLI2="$S/path-cli.json"; MCP2="$S/path-mcp.json"
"$RB" devcontext compile --repo "$PWD" --path lib --budget-tokens 4000 --format json > "$S/path.raw"
norm "$S/path.raw" > "$CLI2"
{
  req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case130","version":"0"}}'
  req 2 tools/call '{"name":"majordomus_devcontext","arguments":{"paths":["lib"],"budget_tokens":4000}}'
} > "$S/mcp2.in"
rc=0; "$RB" mcp --standalone --repo "$PWD" < "$S/mcp2.in" > "$S/mcp2.out" 2>/dev/null || rc=$?
[ "$rc" = 0 ] || { echo "    mcp exited $rc for the path request"; exit 1; }
sed -n 2p "$S/mcp2.out" | jq '.result.structuredContent' > "$S/path-mcp.raw"
norm "$S/path-mcp.raw" > "$MCP2"
cmp -s "$CLI2" "$MCP2" || { echo "    a path resolved differently on the command line and over MCP:"; diff "$CLI2" "$MCP2" | head -30; exit 1; }
# and it actually resolved something: the code under the path, and a contract over it
jq -e '[.selected[] | select([.discovered_by[].selector] | index("scope_path") != null)] | length > 0' "$S/path.raw" >/dev/null \
  || { echo "    nothing under the declared path was selected"; exit 1; }

# --- the directory contracts the compiler resolves for a path are the ones the shell tool
# resolves for it. Two engines, one rule: where they disagree the shell tool is the
# authority and this is the failure that says so.
"$MJ" context resolve .ai/repo/rules > "$S/shell-resolve.txt" 2>/dev/null || {
  echo "    the shell tool could not resolve a path in this fixture"; cat "$S/shell-resolve.txt"; exit 1; }
"$RB" devcontext compile --repo "$PWD" --path .ai/repo/rules --budget-tokens 1000000 --format json > "$S/rules.json"
jq -r '[.selected[] | select(.kind == "context") | .provenance.path] | sort | .[]' "$S/rules.json" > "$S/rust-contracts.txt"
grep -oE '\.ai/[A-Za-z0-9_/.-]*README\.md' "$S/shell-resolve.txt" | sort -u > "$S/shell-contracts.txt"
[ -s "$S/shell-contracts.txt" ] || { echo "    the shell tool resolved no contract for .ai/repo/rules"; cat "$S/shell-resolve.txt"; exit 1; }
while IFS= read -r p; do
  grep -qxF "$p" "$S/rust-contracts.txt" || {
    echo "    the shell tool resolves $p for .ai/repo/rules and the compiler does not"
    echo "    shell:"; cat "$S/shell-contracts.txt"
    echo "    compiler:"; cat "$S/rust-contracts.txt"
    exit 1
  }
done < "$S/shell-contracts.txt"

echo "    context compiler: selection, budget, deduplication, conflict and three surfaces agree"

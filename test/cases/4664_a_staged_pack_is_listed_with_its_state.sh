# majordomus-covers: doctor context
# Every prompt pack in the primary checkout's staging place is listed with its derived state
# from any linked worktree, and a pack left staged too long is reported where a session starts
# (ADR 0236, I4715). A repository is adopted, a linked worktree made, and packs are staged in
# the primary's tmp/owner-packs/ (and a decoy in the worktree's own tmp/, which must not be
# read). From the worktree:
#
#   list      campaigns staged names the primary checkout and its packs, never the decoy
#   states    staged, kept, diverged, planned and unplanned, each from the tracked tree
#   stale     a pack staged past the policy's bound is stale; a fresh one is not
#   mcp       majordomus_campaigns_staged answers what the command answers
#   doctor    WARN names the stale pack; with no bound declared, no WARN
#   context   a STAGED PACKS section names it; with no bound, no section
#   absent    no staging place is an empty list with the reason, exit 0
# claims: none
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || { echo "    jq is required by this case"; exit 1; }

expect_exit 0 "$MJ" init
printf '/tmp/\n' >> .gitignore
git add -A && git commit -qm adopt
W="$T.linked"
git worktree add -q "$W"
trap 'git -C "$T" worktree remove --force "$W" >/dev/null 2>&1 || true' EXIT

listing() { MAJORDOMUS_SHARE="$ROOT/share" "$RB" campaigns staged --repo "${1:-$W}" --format json; }
bad() { echo "    $1"; [ -z "${2:-}" ] || jq -c . "$2" | head -c 900; echo; exit 1; }

# ---------------------------------------------------------------- absent
rc=0; listing > none.json 2> none.err || rc=$?
[ "$rc" = 0 ] || bad "with no staging place the listing exits $rc" none.json
[ "$(jq '.packs | length' none.json)" = 0 ] && jq -e '.places[0].present == false and (.places[0].reason | test("tmp/owner-packs"))' none.json >/dev/null \
  || bad "an absent staging place is not an empty list with its reason" none.json
echo "    ok: no staging place is an empty list with the reason, exit 0"

# ---------------------------------------------------------------- the packs
P="$T/tmp/owner-packs"
mk() { mkdir -p "$P/$1"; printf '# %s brief\n' "$1" > "$P/$1/README.md"; }
for n in old fresh kept planned unplanned diverged; do mk "$n"; done
mkdir -p "$P/drafts/x" && printf 'draft\n' > "$P/drafts/x/a.md"
touch -t 202001010000 "$P/old/README.md"
mkdir -p "$W/tmp/owner-packs/decoy" && printf '# decoy\n' > "$W/tmp/owner-packs/decoy/README.md"

# keep four of them in the worktree's tracked tree, byte for byte, with the manifest lines
mkdir -p "$W/campaigns" "$W/.ai/repo/ci" "$W/.ai/repo/project/issues"
for n in kept planned unplanned diverged; do cp -R "$P/$n" "$W/campaigns/$n"; done
if command -v sha256sum >/dev/null 2>&1; then sum() { sha256sum "$@"; }; else sum() { shasum -a 256 "$@"; }; fi
( cd "$W/campaigns" && for n in kept planned unplanned diverged; do sum "$n/README.md"; done > MANIFEST.sha256 )
printf 'unplanned  executed\n' > "$W/.ai/repo/ci/campaigns-unplanned.txt"
printf 'id: I1\nwhy: "kept as campaigns/planned."\n' > "$W/.ai/repo/project/issues/I1.yaml"
printf 'changed after it was kept\n' >> "$P/diverged/README.md"

printf '\ncampaigns:\n  staged_max_age_days: 7\n' >> "$W/.ai/repo/policy.yaml"

listing > staged.json || bad "the listing failed from the linked worktree" staged.json
[ "$(cd "$(jq -r .primary staged.json)" && pwd -P)" = "$(cd "$T" && pwd -P)" ] \
  || bad "the primary checkout is not the one the worktree was made from" staged.json
jq -e '[.packs[].name] == ["diverged","fresh","kept","old","planned","unplanned"]' staged.json >/dev/null \
  || bad "the listing is not the primary's packs (the decoy and drafts/ must be absent)" staged.json
jq -e '.places[0].holding == ["drafts"]' staged.json >/dev/null || bad "drafts/ is not named as holding" staged.json
echo "    ok: from a linked worktree the primary checkout's packs are listed, never the worktree's own"

state() { jq -r --arg n "$1" '.packs[] | select(.name == $n) | .state' staged.json; }
for pair in old:staged fresh:staged kept:kept planned:planned unplanned:unplanned diverged:diverged; do
  [ "$(state "${pair%%:*}")" = "${pair#*:}" ] || bad "${pair%%:*} reads $(state "${pair%%:*}"), not ${pair#*:}" staged.json
done
jq -e '.packs[] | select(.name == "planned") | .cited_by == [".ai/repo/project/issues/I1.yaml"]' staged.json >/dev/null \
  || bad "the planned pack does not name the record that cites it" staged.json
jq -e '.packs[] | select(.name == "diverged") | .differing == ["README.md"]' staged.json >/dev/null \
  || bad "the diverged pack does not name the file that differs" staged.json
echo "    ok: staged, kept, diverged, planned and unplanned are each derived from the tracked tree"

jq -e '.stale == 1 and (.packs[] | select(.name == "old") | .stale) and ((.packs[] | select(.name == "fresh") | .stale) | not)' staged.json >/dev/null \
  || bad "only the pack staged past 7 days is stale" staged.json
echo "    ok: a pack staged past the policy's bound is stale, a fresh one is not"

# ---------------------------------------------------------------- mcp
{ printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case4664","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_campaigns_staged","arguments":{}}}\n'
} | env MAJORDOMUS_SHARE="$ROOT/share" "$RB" mcp --standalone --repo "$W" 2>mcp.err | sed -n 2p > frame.json
jq -e '.result.structuredContent' frame.json > mcp.json 2>/dev/null || bad "majordomus_campaigns_staged returned no typed answer" frame.json
[ "$(jq -c '[.packs[] | [.name, .state, .stale]]' mcp.json)" = "$(jq -c '[.packs[] | [.name, .state, .stale]]' staged.json)" ] \
  || bad "the MCP tool and the command disagree" mcp.json
echo "    ok: majordomus_campaigns_staged answers what the command answers"

# ---------------------------------------------------------------- doctor and context
( cd "$W" && MAJORDOMUS_BIN="$RB" MAJORDOMUS_SHARE="$ROOT/share" "$MJ" doctor ) > doctor.out 2>&1 || true
grep -E '^WARN +campaigns' doctor.out | grep -q 'tmp/owner-packs/old' \
  || { echo "    doctor does not warn of the stale pack:"; grep -i campaigns doctor.out | sed 's/^/      /'; exit 1; }
grep -E '^WARN +campaigns' doctor.out | grep -q 'tmp/owner-packs/fresh' && bad "doctor warns of a fresh pack"
( cd "$W" && MAJORDOMUS_BIN="$RB" MAJORDOMUS_SHARE="$ROOT/share" "$MJ" context ) > context.out 2>&1 || true
grep -q '^## STAGED PACKS' context.out && grep -q 'tmp/owner-packs/old (' context.out \
  || { echo "    context names no stale pack:"; head -40 context.out | sed 's/^/      /'; exit 1; }
echo "    ok: doctor warns of the stale pack and context names it"

git -C "$W" checkout -q -- .ai/repo/policy.yaml
( cd "$W" && MAJORDOMUS_BIN="$RB" MAJORDOMUS_SHARE="$ROOT/share" "$MJ" doctor ) > doctor.out 2>&1 || true
grep -E '^WARN +campaigns' doctor.out && bad "doctor warns with no bound declared"
grep -qE 'campaigns.*none staged past a bound \(none declared\)' doctor.out \
  || { echo "    doctor does not say no bound is declared:"; grep -i campaigns doctor.out | sed 's/^/      /'; exit 1; }
( cd "$W" && MAJORDOMUS_BIN="$RB" MAJORDOMUS_SHARE="$ROOT/share" "$MJ" context ) > context.out 2>&1 || true
grep -q '^## STAGED PACKS' context.out && bad "context names staged packs with no bound declared"
echo "    ok: with no bound declared, neither doctor nor context reports a pack"

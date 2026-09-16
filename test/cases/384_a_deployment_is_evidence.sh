# A deployment is evidence: `majordomus served observe` reads the build identity a site
# serves and judges it against a commit by containment, and `served show` re-judges what was
# recorded. Driven end to end through the real executable, a real git history and a real HTTP
# server, because the failure this exists for is a green job and a stale site: nothing short
# of fetching what was served and asking git about it can tell those apart.
#
# The history is A <- B <- C. The site serves B, so:
#   expected A or B  -> served (exit 0)    the build contains them
#   expected C       -> stale  (exit 10)   the build is behind
#   a dirty build    -> dirty  (exit 10)   it proves no commit
#   garbage          -> malformed (12), nothing -> unreachable (12),
#   a commit this clone lacks -> undecided (12)
# and a record of B, read back after C, is stale without anyone re-observing.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?

repo="$PWD/repo"; site="$PWD/site"
mkdir -p "$repo" "$site"
g() { git -C "$repo" -c user.name=t -c user.email=t@t "$@"; }
g init -q
# The command reads a repository, so the fixture carries the smallest layer one needs: the
# manifest discovery reads, the policy and scope it resolves against, and the source classes
# the index is built from. Copied rather than written, so that a change to their shape moves
# this fixture with them instead of leaving it asserting an older contract.
mkdir -p "$repo/.ai/repo/knowledge"
cp "$ROOT/.ai/manifest.yaml" "$repo/.ai/"
cp "$ROOT/.ai/repo/policy.yaml" "$ROOT/.ai/repo/scope.yaml" "$repo/.ai/repo/"
cp "$ROOT/.ai/repo/knowledge/sources.yaml" "$repo/.ai/repo/knowledge/"
mkdir -p "$repo/.ai/repo/profiles" "$repo/.ai/repo/rules"
cp "$ROOT/.ai/README.md" "$repo/.ai/"
cp "$ROOT"/.ai/repo/profiles/*.yaml "$repo/.ai/repo/profiles/" 2>/dev/null || true
cp "$ROOT"/.ai/repo/rules/project/*.md "$repo/.ai/repo/rules/" 2>/dev/null || true
g add -A .ai
for c in a b c; do echo "$c" > "$repo/$c"; g add "$c"; g commit -q -m "$c"; done
A="$(g rev-parse HEAD~2)"; B="$(g rev-parse HEAD~1)"; C="$(g rev-parse HEAD)"

printf '{"schema":1,"commit":"%s","dirty":false,"source_version":"0.0.0"}\n' "$B" > "$site/build.json"
printf '{"schema":1,"commit":"%s","dirty":true}\n' "$B" > "$site/dirty.json"
printf '<html>not an identity</html>\n' > "$site/garbage.json"
printf '{"schema":1,"commit":"%s","dirty":false}\n' "ffffffffffffffffffffffffffffffffffffffff" > "$site/foreign.json"
start_http "$site" || { echo "    skip: no python3 or node to serve the fixture site"; exit 0; }
trap stop_http EXIT

observe() { # <expected-exit> <verdict> <args...>
  local want="$1" verdict="$2"; shift 2
  "$RB" served observe --repo "$repo" --url "$HTTP_BASE" --format json "$@" > out.json 2> err.txt
  local rc=$?
  [ "$rc" = "$want" ] || { echo "    served observe $* exited $rc, expected $want"; cat out.json err.txt; exit 1; }
  jq -e --arg v "$verdict" '.observation.verdict == $v' out.json >/dev/null \
    || { echo "    served observe $* judged $(jq -r .observation.verdict out.json), expected $verdict"; cat out.json; exit 1; }
}

# 1. the build contains the expected commit, exactly or as an ancestor
observe 0 served --commit "$B"
jq -e --arg b "$B" '.observation.served.commit == $b and .exit_code == 0 and (.recorded | type) == "string"' out.json >/dev/null \
  || { echo "    the observation does not carry the served identity and where it was recorded"; cat out.json; exit 1; }
observe 0 served --commit "$A"

# 2. behind is a measured no, and so is a build from an uncommitted tree
observe 10 stale --commit "$C"
observe 10 dirty --commit "$B" --identity dirty.json

# 3. an unanswered question is never a yes, and each says which question it did not answer
observe 12 malformed --commit "$B" --identity garbage.json
observe 12 undecided --commit "$B" --identity foreign.json
observe 12 unreachable --commit "$B" --identity missing.json --dry-run
jq -e 'has("recorded") | not' out.json >/dev/null || { echo "    a dry run recorded"; cat out.json; exit 1; }

# 4. input that is not a site or not a commit is refused before any probe
"$RB" served observe --repo "$repo" --url "file:///etc/passwd" --commit "$B" >/dev/null 2>err.txt \
  && { echo "    a file:// base was probed"; exit 1; }
"$RB" served observe --repo "$repo" --url "$HTTP_BASE" --commit "--output=/tmp/x" >/dev/null 2>err.txt \
  && { echo "    an option-shaped revision was accepted"; exit 1; }
"$RB" served observe --repo "$repo" --url "$HTTP_BASE" --commit "$B" --identity "../build.json" >/dev/null 2>err.txt \
  && { echo "    an identity escaping the site was accepted"; exit 1; }

# 5. the record is re-judged, not replayed: observe a clean B again so it is the newest record
#    of pages, then read it against B and against C without probing anything
observe 0 served --commit "$B"
"$RB" served show --repo "$repo" --commit "$B" --format json > show.json || { echo "    served show failed"; exit 1; }
jq -e '.deployments | length == 1 and .[0].deployment == "pages" and .[0].now.verdict == "served"' show.json >/dev/null \
  || { echo "    show does not report pages as served against B"; cat show.json; exit 1; }
"$RB" served show --repo "$repo" --commit "$C" --format json > show.json
jq -e '.deployments[0].observation.verdict == "served" and .deployments[0].now.verdict == "stale"' show.json >/dev/null \
  || { echo "    a record of B still reads as served once C is asked about"; cat show.json; exit 1; }

# 6. the record is checkout-local and never tracked
[ -s "$repo/.ai/local/state/served/observations.jsonl" ] || { echo "    no observation file was written"; exit 1; }
[ "$(wc -l < "$repo/.ai/local/state/served/observations.jsonl" | tr -d ' ')" = 7 ] \
  || { echo "    expected seven recorded observations: every probe but the dry run and the refusals"; wc -l "$repo/.ai/local/state/served/observations.jsonl"; exit 1; }
[ -z "$(g status --porcelain --untracked-files=no)" ] || { echo "    observing changed a tracked file"; exit 1; }

echo "    served: contains=0 stale/dirty=10 unanswered=12; records re-judged by containment"

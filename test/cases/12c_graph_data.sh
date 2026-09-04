# The interactive graphs are derived, not drawn. This proves it by changing the sources they
# are scanned from and watching the graph follow: a new module dependency appears as an edge,
# a removed state reference removes one, and a claim's evidence reaches its test node. A graph
# that survives those edits unchanged would be a hand-written picture wearing a JSON coat.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }
fixture_repo "$T" AGENTS.md docs
mkdir -p "$T/site/data" "$T/test"; cp "$ROOT/site/data/marketing.toml" "$ROOT/site/data/nav.toml" "$T/site/data/"; cp -R "$ROOT/site/content-src" "$T/site/"; cp -R "$ROOT/test/cases" "$T/test/"
git -C "$T" add -A >/dev/null; git -C "$T" commit -qm fixture
A="$T/site/data/generated/architecture.json"; C="$T/site/data/generated/claims-graph.json"

expect_exit 0 "$T/scripts/generate-site-data"

# the entry point reaches every command that has a module of its own
for c in start finish doctor; do
  [ "$(jq -r --arg c "$c" '[.edges[] | select(.source=="bin/majordomus" and .target==("lib/"+$c+".sh"))] | length' "$A")" = 1 ] \
    || { echo "    no dispatch edge for $c"; exit 1; }
done
# and every module node that is a command carries the route its page is served at
jq -e '[.nodes[] | select(.kind=="module" and .command != null) | select(.route != ("/commands/" + .command + "/"))] | length == 0' "$A" >/dev/null \
  || { echo "    a command module carries a route its command page does not use"; exit 1; }

# a new dependency between two modules becomes an edge
[ "$(jq -r '[.edges[] | select(.source=="lib/history.sh" and .target=="lib/search.sh")] | length' "$A")" = 0 ]
printf '. "$MJ_LIB_DIR/search.sh"\n' >> "$T/lib/history.sh"
expect_exit 0 "$T/scripts/generate-site-data"
[ "$(jq -r '[.edges[] | select(.source=="lib/history.sh" and .target=="lib/search.sh" and .kind=="sources")] | length' "$A")" = 1 ] \
  || { echo "    a new module dependency did not appear in the graph"; exit 1; }

# a module that stops naming a state file loses its edge to it
[ "$(jq -r '[.edges[] | select(.source=="lib/question.sh" and .target==".majordomus/state/open-questions.md")] | length' "$A")" = 1 ]
grep -v 'state/open-questions.md' "$T/lib/question.sh" > "$T/lib/question.sh.new" && mv "$T/lib/question.sh.new" "$T/lib/question.sh"
expect_exit 0 "$T/scripts/generate-site-data"
[ "$(jq -r '[.edges[] | select(.source=="lib/question.sh" and .target==".majordomus/state/open-questions.md")] | length' "$A")" = 0 ] \
  || { echo "    the graph kept an edge for a reference that is no longer in the source"; exit 1; }

# no edge points at a node that does not exist, in either graph
for g in "$A" "$C"; do
  jq -e '[.nodes[].id] as $n | [.edges[] | select((.source | IN($n[]) | not) or (.target | IN($n[]) | not))] | length == 0' "$g" >/dev/null \
    || { echo "    $(basename "$g") has a dangling edge"; exit 1; }
done

# every guaranteed claim reaches a test node; that is what "guaranteed" means on the site
jq -e '[.nodes[] | select(.kind=="claim" and .status=="guaranteed") | .id] as $g
       | ([.edges[] | select(.kind=="proved_by") | .source] | unique) as $p
       | ($g - $p) | length == 0' "$C" >/dev/null || { echo "    a guaranteed claim has no test edge"; exit 1; }
# and the evidence nodes are shared, not duplicated per claim
[ "$(jq '[.nodes[].id] | length' "$C")" = "$(jq '[.nodes[].id] | unique | length' "$C")" ] || { echo "    duplicate node ids in claims-graph.json"; exit 1; }

# a claim whose status changes moves in the graph without any hand edit
sed -i.bak 's/^    status: guaranteed$/    status: advisory/' "$T/docs/CLAIMS.yaml"; rm -f "$T/docs/CLAIMS.yaml.bak"
expect_exit 0 "$T/scripts/generate-site-data"
[ "$(jq '[.nodes[] | select(.kind=="claim" and .status=="guaranteed")] | length' "$C")" = 0 ] \
  || { echo "    claim status in the graph does not follow docs/CLAIMS.yaml"; exit 1; }

# the why section is data too: the moments name commands, responsibilities and claims, and a
# name that does not resolve must stop the build rather than ship a link to nothing
W="$T/site/data/generated/why.json"
git -C "$T" checkout -q -- docs/CLAIMS.yaml 2>/dev/null || true
expect_exit 0 "$T/scripts/generate-site-data"
[ "$(jq '.moments | length' "$W")" -ge 3 ] || { echo "    no moments were derived from site/content-src/why/"; exit 1; }
# every moment resolves each thing it names, or the detail arrays would be shorter than the ids
jq -e '[.moments[] | select((.claims | length) != (.claim_detail | length)
                         or (.commands | length) != (.command_detail | length)
                         or (.responsibilities | length) != (.responsibility_detail | length))] | length == 0' "$W" >/dev/null \
  || { echo "    a moment names something the generator could not resolve"; exit 1; }

first_why="$(cd "$T/site/content-src/why" && ls ./*.md | sed 's#^\./##' | grep -v '^_index\.md$' | head -1)"
cp "$T/site/content-src/why/$first_why" "$T/why.bak"
sed -i.bak 's/^claims = \[/claims = ["no-such-claim-id", /' "$T/site/content-src/why/$first_why"; rm -f "$T/site/content-src/why/$first_why.bak"
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep 'names claims that do not exist'
# and the previous generation is intact: a refused build publishes nothing
[ "$(jq '.moments | length' "$W")" -ge 3 ]
cp "$T/why.bak" "$T/site/content-src/why/$first_why"
expect_exit 0 "$T/scripts/generate-site-data"

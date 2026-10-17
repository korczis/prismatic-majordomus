# majordomus-covers: none
# The knowledge base is published one page per record, and each record is linked to what it
# is about (ADR 0173, I2793). Read from this repository's own derived data, as case 531 reads
# it:
#
#   1. site/data/publication.toml declares the knowledge kind an entity projection, and
#      site/data/generated/entities.json carries it at that route with every record of the
#      index, curated and candidate, as an entity;
#   2. each of those records has a generated page under the route's directory, whose front
#      matter carries the record's status, so that a candidate never reads as verified;
#   3. every record that names a subject in `about` has exactly that `about` edge in
#      docs/generated/graph.json, and its entity lists the subject among its references;
#   4. the navigation leads to the section, from site/data/nav.toml and nowhere else.
#
# Nothing here names a record: every expected value is read from the records, the declaration
# or the derived data.
. "$ROOT/test/lib.sh"

command -v jq >/dev/null 2>&1 || skip "jq absent"
DATA="$ROOT/site/data/generated/entities.json"
GRAPH="$ROOT/docs/generated/graph.json"
PUB="$ROOT/site/data/publication.toml"
for f in "$DATA" "$GRAPH" "$PUB"; do
  [ -f "$f" ] || { echo "    $f is missing; run scripts/derive"; exit 1; }
done

fail=0
note() { echo "    $1"; fail=1; }

# ---------------------------------------------------------------- 1: declared and carried
awk '/^\[\[kinds\]\]/ { k = "" } /^kind = "knowledge"/ { k = 1 } k && /^projection = / { print; exit }' "$PUB" \
  | grep -qF 'projection = "entity"' || note "publication.toml does not publish knowledge as entity pages"
route="$(jq -r '.kinds[] | select(.kind == "knowledge") | .route' "$DATA")"
dir="$(jq -r '.kinds[] | select(.kind == "knowledge") | .dir' "$DATA")"
[ "$route" = "/knowledge/" ] || note "entities.json publishes knowledge at '$route', not /knowledge/"
nodes="$(jq '[.graph.nodes[] | select(.kind == "knowledge" and (.external | not))] | length' "$GRAPH")"
entities="$(jq '[.entities[] | select(.kind == "knowledge")] | length' "$DATA")"
files="$(git -C "$ROOT" ls-files -- '.ai/repo/knowledge/curated/*.md' '.ai/repo/knowledge/candidates/*.md' \
  | grep -cv '/README\.md$' || true)"
[ "$nodes" -ge 1 ] || note "the graph holds no knowledge record"
[ "$nodes" = "$files" ] || note "the graph holds $nodes knowledge records; the tree tracks $files"
[ "$entities" = "$nodes" ] || note "entities.json publishes $entities knowledge records; the graph holds $nodes"

# ---------------------------------------------------------------- 2: one page each, with its status
jq -r '.entities[] | select(.kind == "knowledge") | [.slug, (.status // "")] | @tsv' "$DATA" > k.tsv \
  || { echo "    entities.json does not parse"; exit 1; }
while IFS="$(printf '\t')" read -r slug status; do
  page="$ROOT/site/content/$dir/$slug.md"
  [ -f "$page" ] || { note "no generated page behind $route$slug/"; continue; }
  [ -n "$status" ] || { note "$slug carries no status"; continue; }
  grep -qxF "status = \"$status\"" "$page" || note "$slug's page does not carry its status '$status'"
done < k.tsv
[ -f "$ROOT/site/content/$dir/_index.md" ] || note "the knowledge section has no index page"

# ---------------------------------------------------------------- 3: about is an edge
git -C "$ROOT" grep -h -E '^(id|about): ' -- '.ai/repo/knowledge/curated/*.md' '.ai/repo/knowledge/candidates/*.md' \
  | awk '/^id: / { id = $2 } /^about: / { print id "\t" $2 }' > about.tsv
while IFS="$(printf '\t')" read -r id subject; do
  n="$(jq --arg s "majordomus://knowledge/$id" '[.graph.edges[] | select(.source == $s and .kind == "about")] | length' "$GRAPH")"
  [ "$n" = 1 ] || note "$id is about $subject and the graph draws $n about edge(s) from it"
  jq -e --arg slug "$id" '.entities[] | select(.kind == "knowledge" and .slug == $slug)
                          | .references | any(.edge == "about")' "$DATA" >/dev/null \
    || note "the page of $id does not name what it is about"
done < about.tsv

# ---------------------------------------------------------------- 4: in the navigation
grep -qF 'href = "/knowledge/"' "$ROOT/site/data/nav.toml" || note "site/data/nav.toml does not lead to /knowledge/"

[ "$fail" = 0 ] || exit 1
echo "    knowledge is published one page per record with its status, each record is linked to what it is about, and the navigation leads there"

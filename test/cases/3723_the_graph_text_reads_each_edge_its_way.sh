# majordomus-covers: none
# majordomus-skip-runs-in: site
# The milestone graph's text reads each edge in the direction it means (I3750). The roadmap's
# edges are emitted from a milestone's depends_on, so an edge's source is the dependency and its
# target the dependent. They used to carry kind "requires", so the text fallback under the source
# read "foundation requires runtime-adapters", the inverse of the plan, for every one of them. The
# kind is now "unblocks", which reads correctly from the source; and the fallback lists each edge
# under both ends ("unblocks → target" under the source, "source unblocks → this" under the
# target), as site/graph.js's detail panel already did.
#
# The real site/ is rendered by Zola in a copy and /roadmap/'s text fallback is read node by node:
# every edge of roadmap-graph.json is listed under its source and under its target in that
# direction, and the old reading ("<source> requires <target>") appears nowhere.
. "$ROOT/test/lib.sh"
mj_zola_ready "$ROOT" || skip "$(mj_zola_why "$ROOT")"
command -v jq >/dev/null || skip "jq absent"
mj_runs python3 || skip "$(mj_runs_why python3)"

G="$ROOT/site/data/generated/roadmap-graph.json"
# the producer names the kind by the direction it is drawn in
[ "$(jq -r '[.edges[].kind] | unique | join(",")' "$G")" = "unblocks" ] \
  || { echo "    roadmap-graph.json carries edge kinds other than unblocks: $(jq -c '[.edges[].kind] | unique' "$G")"; exit 1; }
jq -e '.edge_kinds.unblocks' "$G" >/dev/null || { echo "    edge_kinds does not define unblocks"; exit 1; }
# and an edge's source is the dependency: the target's depends_on in roadmap.json, the graph's own
# source, names it
P="$ROOT/site/data/generated/roadmap.json"
bad="$(jq -r --slurpfile p "$P" '.edges[] | . as $e | ($p[0].milestones[] | select(.id == $e.target) | (.depends_on // [])) as $d | select(($d | index($e.source)) == null) | "\($e.source) -> \($e.target)"' "$G")"
[ -z "$bad" ] || { echo "    edges whose target does not depend on their source: $bad"; exit 1; }

mkdir -p S
( cd "$ROOT/site" && tar cf - --exclude ./public . ) | ( cd S && tar xf - )
mkdir -p S/static/js S/static/graphs
: > S/static/app.css; : > S/static/kit.css
for js in "$ROOT"/site/*.js; do cp "$js" S/static/js/; done
printf '{"schema":1,"commit":"0123456789abcdef0123456789abcdef01234567","commit_time":"2026-10-20T12:00:00+00:00","dirty":false,"surfaces":[],"snapshots":[],"release":null}\n' > S/data/build.json
( cd S && zola build --force --output-dir ../out ) > zola.log 2>&1 \
  || { tail -20 zola.log; echo "    zola could not render the site copy"; exit 1; }
R="out/roadmap/index.html"
[ -f "$R" ] || { echo "    no /roadmap/ was rendered"; exit 1; }

python3 -I - "$R" "$G" <<'EOF' || exit 1
import html, json, re, sys
page = open(sys.argv[1], encoding="utf-8").read()
graph = json.load(open(sys.argv[2], encoding="utf-8"))
m = re.search(r'<ul[^>]*data-graph-fallback[^>]*>(.*?)</ul>\s*</details>', page, re.S)
if not m:
    print("    /roadmap/ carries no text fallback of the graph"); sys.exit(1)
fallback = m.group(1)
# one block per node: from its data-graph-node anchor to the next node's
nodes = {}
for b in re.split(r'(?=<li class="px-4 py-3">)', fallback):
    n = re.search(r'data-graph-node="([^"]+)"', b)
    if n:
        nodes[n.group(1)] = b
fail = []
for e in graph["edges"]:
    s, t, k = e["source"], e["target"], e["kind"]
    key = f'data-graph-edge="{s} {k} {t}"'
    src, tgt = nodes.get(s, ""), nodes.get(t, "")
    if key not in src or not re.search(re.escape(key) + r'>\s*<span[^>]*>' + re.escape(k) + r'</span> &rarr; ' + re.escape(t) + '<', src):
        fail.append(f"{s} does not read '{k} -> {t}'")
    if key not in tgt or not re.search(re.escape(key) + r'>' + re.escape(s) + r' <span[^>]*>' + re.escape(k) + r'</span> &rarr; this<', tgt):
        fail.append(f"{t} does not read '{s} {k} -> this'")
# the inverted reading is gone everywhere: no listed edge carries the old kind
listed = re.findall(r'data-graph-edge="([^"]+)"', fallback)
if not listed:
    fail.append("the fallback lists no edge at all")
for le in listed:
    if le.split(" ")[1] == "requires":
        fail.append(f"an edge still reads '{html.unescape(le)}'")
if fail:
    print("    " + "\n    ".join(sorted(set(fail))[:20])); sys.exit(1)
print(f"    {len(graph['edges'])} edge(s) read source -> target from both ends")
EOF

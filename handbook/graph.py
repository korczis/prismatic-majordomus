"""Compact the repository's own generated graph (docs/generated/graph.json) for the handbook."""
import base64, gzip, json, os, sys

import shutil, subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.environ.get("HANDBOOK_ROOT") or os.path.dirname(HERE)
OUT = os.environ.get("HANDBOOK_DERIVED") or os.path.join(ROOT, "target", "handbook")
SOURCE = os.path.join(ROOT, "docs", "generated", "graph.json")
src = json.load(open(SOURCE, encoding="utf-8"))
shutil.copyfile(SOURCE, os.path.join(OUT, "graph-src.json"))
g = src["graph"]
commit = subprocess.run(("git", "-C", ROOT, "rev-parse", "--short=10", "HEAD"), check=True, capture_output=True, text=True).stdout.strip()

# `is_a` only repeats a node's kind, and a `kind` node is that repetition's other end.
nodes = [n for n in g["nodes"] if n["kind"] != "kind"]
kinds = sorted({n["kind"] for n in nodes})
kpos = {k: i for i, k in enumerate(kinds)}
npos = {n["id"]: i for i, n in enumerate(nodes)}
edges = [e for e in g["edges"] if e["kind"] != "is_a" and e["source"] in npos and e["target"] in npos]
ekinds = sorted({e["kind"] for e in edges})
epos = {k: i for i, k in enumerate(ekinds)}
dropped = len(g["edges"]) - len(edges) - sum(1 for e in g["edges"] if e["kind"] == "is_a")
if dropped:
    sys.exit("graph: %d edge(s) name a node the graph does not carry" % dropped)

def clip(s, n=180):
    s = " ".join(str(s or "").split())
    return s if len(s) <= n else s[: n - 1].rstrip() + "…"

out = {
    "commit": commit,
    "generator": src.get("generator", ""),
    "title": g.get("title", ""),
    "kinds": [[k, g["node_kinds"].get(k, "")] for k in kinds],
    "edgeKinds": [[k, g["edge_kinds"].get(k, "")] for k in ekinds],
    "nodes": [[kpos[n["kind"]], n.get("label", n["id"]), clip(n.get("summary")), n.get("source", ""), n.get("status", "")] for n in nodes],
    "edges": [[npos[e["source"]], npos[e["target"]], epos[e["kind"]]] for e in edges],
}
blob = json.dumps(out, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
gz = gzip.compress(blob, 9, mtime=0)
open(os.path.join(OUT, "graph.b64"), "w").write(base64.b64encode(gz).decode("ascii"))
print("graph", commit, "nodes", len(nodes), "edges", len(edges), "kinds", len(kinds), "edge kinds", len(ekinds), "raw", len(blob), "gz", len(gz))

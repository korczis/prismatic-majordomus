"""Build the embedded Markdown corpus of the handbook from the tree this file is in."""
import base64, gzip, json, os, posixpath, re, sys

import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.environ.get("HANDBOOK_ROOT") or os.path.dirname(HERE)
OUT = os.environ.get("HANDBOOK_DERIVED") or os.path.join(ROOT, "target", "handbook")

def git(*args):
    return subprocess.run(("git", "-C", ROOT) + args, check=True, capture_output=True, text=True).stdout

COMMIT = git("rev-parse", "--short=10", "HEAD").strip()
TRACKED = set(p for p in git("ls-files").split("\n") if p)
DIRS = set()
for p in TRACKED:
    while "/" in p:
        p = p.rsplit("/", 1)[0]
        DIRS.add(p)

SKIP = {"docs/PLAN_STATUS.md", "docs/PAGES_STATUS.md"}


def kind_of(path):
    if path.startswith("docs/claims/"): return "claim"
    if path.startswith("docs/generated/"): return None
    if path.startswith("docs/"): return "doc" if path.count("/") == 1 else None
    if path.startswith(".ai/repo/adrs/"): return "adr"
    if path.startswith(".ai/repo/rules/"): return "guide" if path.endswith("README.md") else "rule"
    if path.startswith(".ai/repo/use-cases/"): return "usecase"
    if path.startswith(".ai/repo/workflows/") or path.startswith(".ai/repo/skills/"): return "guide"
    if path in ("README.md", "AGENTS.md", ".ai/README.md"): return "guide"
    return None


docs = {}
for rel in sorted(TRACKED):                      # what git tracks, read as the work tree has it
    if not rel.endswith(".md") or rel in SKIP: continue
    k = kind_of(rel)
    if k and os.path.isfile(os.path.join(ROOT, rel)):
        docs[rel] = {"kind": k, "text": open(os.path.join(ROOT, rel), encoding="utf-8").read()}

FM = re.compile(r"\A---\n([\s\S]*?)\n---\n")
FENCE = re.compile(r"^(```|~~~)[\s\S]*?^\1[^\n]*$", re.M)
LINK = re.compile(r"\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
rule_ids, adr_nums = {}, {}
for path, d in docs.items():
    text = d["text"]
    m = FM.match(text)
    fm = m.group(1) if m else ""
    body = text[m.end():] if m else text
    title = None
    t = re.search(r"^title:\s*(.+)$", fm, re.M)
    if t: title = t.group(1).strip().strip("'\"")
    if not title:
        h = re.search(r"^#\s+(.+)$", body, re.M)
        title = h.group(1).strip() if h else posixpath.basename(path)
    d["title"] = re.sub(r"[`*]", "", title)
    i = re.search(r"^id:\s*(\S+)$", fm, re.M)
    if d["kind"] == "rule" and i: rule_ids[i.group(1)] = path; d["rid"] = i.group(1)
    if d["kind"] == "adr":
        n = re.match(r"(\d{4})-", posixpath.basename(path))
        if n: adr_nums[n.group(1)] = path; d["num"] = n.group(1)
    s = re.search(r"^status:\s*(\S+)$", fm, re.M)
    if s: d["status"] = s.group(1)
    c = re.search(r"^class:\s*(\S+)$", fm, re.M)
    if c: d["cls"] = c.group(1)
    if d["kind"] == "rule":
        d["pf"] = [k for k in ("tests", "validator", "enforced_by", "reviewed_because")
                   if re.search(r"^  %s:\s*\S" % k, fm, re.M) and not re.search(r"^  %s:\s*\[\s*\]" % k, fm, re.M)]

unresolved, stats = [], {"in": 0, "gh": 0, "ext": 0, "site": 0}
for path, d in docs.items():
    prose = FENCE.sub("", d["text"])
    out = set()
    for target in LINK.findall(prose):
        if re.match(r"[a-z][a-z0-9+.-]*:", target): stats["ext"] += 1; continue
        if target.startswith("#"): continue
        if target.startswith("/"): stats["site"] += 1; continue
        t = target.split("#")[0].split("?")[0]
        if not t: continue
        res = posixpath.normpath(posixpath.join(posixpath.dirname(path), t))
        if res in docs: out.add(res); stats["in"] += 1
        elif res in TRACKED or res.rstrip("/") in DIRS or res == ".": stats["gh"] += 1
        else: unresolved.append((path, target))
    for num in set(re.findall(r"\bADR[ -](\d{1,4})\b", prose)):
        p = adr_nums.get(num.zfill(4))
        if p and p != path: out.add(p)
    for rid in set(re.findall(r"\b(?:project|majordomus|ai)\.[a-z0-9][a-z0-9-]+(?:\.[a-z0-9-]+)*", prose)):
        p = rule_ids.get(rid)
        if p and p != path: out.add(p)
    d["out"] = sorted(out)

order = sorted(docs)
pos = {p: i for i, p in enumerate(order)}
inc = {p: [] for p in order}
for p in order:
    for o in docs[p]["out"]: inc[o].append(pos[p])
index = {"commit": COMMIT, "docs": [], "rules": {k: pos[v] for k, v in rule_ids.items()},
         "adrs": {k: pos[v] for k, v in adr_nums.items()}}
for p in order:
    d = docs[p]
    e = {"p": p, "t": d["title"], "k": d["kind"], "b": len(d["text"].encode("utf-8")),
         "o": [pos[o] for o in d["out"]], "i": inc[p]}
    for k in ("status", "cls", "num", "rid", "pf"):
        if k in d: e[k] = d[k]
    index["docs"].append(e)
blob = json.dumps([docs[p]["text"] for p in order], ensure_ascii=False).encode("utf-8")
gz = gzip.compress(blob, 9, mtime=0)
open(os.path.join(OUT, "corpus-index.json"), "w", encoding="utf-8").write(json.dumps(index, ensure_ascii=False))
open(os.path.join(OUT, "corpus.b64"), "w").write(base64.b64encode(gz).decode("ascii"))
kinds = {}
for d in docs.values(): kinds[d["kind"]] = kinds.get(d["kind"], 0) + 1
open(os.path.join(OUT, "tracked-new.txt"), "w", encoding="utf-8").write("\n".join(sorted(TRACKED)) + "\n")
print("corpus", COMMIT, "docs", len(docs), kinds, "raw", len(blob), "gz", len(gz), "links", stats,
      "edges", sum(len(d["out"]) for d in docs.values()), "unresolved", len(unresolved))
for u in unresolved[:15]: print("  unresolved", u)
if unresolved and "--strict" in sys.argv: sys.exit(1)

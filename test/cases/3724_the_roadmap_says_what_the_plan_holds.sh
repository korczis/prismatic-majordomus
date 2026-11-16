# majordomus-covers: none
# majordomus-skip-runs-in: site
# /roadmap/ says only what the plan and the history hold, with a bar and a date on every
# milestone (I3751; the owner's decision of 2026-10-20: dates and progress bars on every
# surface). It used to say that each milestone depends on the ones before it (28 of 100 do),
# that there is no progress bar because a full one would be a lie, and that foundation, accepted
# with no issues, had "no issues written yet"; and no milestone said when anything happened.
#
# The real site/ is rendered by Zola in a copy, and the page is held to the data it reads:
# one bar per milestone with required issues, valued at its done count; the release date from
# the release record, or the last merged_at from delivery.json; the dependency count from
# roadmap.json; the issueless DONE milestone read as delivered; the withdrawn sentences absent.
# The Czech memory carries each new sentence, keyed without a number or a date in it.
. "$ROOT/test/lib.sh"
mj_zola_ready "$ROOT" || skip "$(mj_zola_why "$ROOT")"
mj_runs python3 || skip "$(mj_runs_why python3)"
mj_runs node || skip "$(mj_runs_why node)"

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

python3 -I - "$R" "$ROOT/site/data/generated" "$ROOT/site/static/releases" <<'EOF' || exit 1
import json, os, re, sys
page = open(sys.argv[1], encoding="utf-8").read()
gen, rel = sys.argv[2], sys.argv[3]
roadmap = json.load(open(os.path.join(gen, "roadmap.json"), encoding="utf-8"))
dpath = os.path.join(gen, "delivery.json")
delivery = json.load(open(dpath, encoding="utf-8")) if os.path.exists(dpath) else None
fail = []
cards = {}
for m in re.finditer(r'<li id="milestone-([^"]+)" data-milestone="[^"]+"(.*?)(?=<li id="milestone-|</ol>)', page, re.S):
    cards[m.group(1)] = m.group(2)
for ms in roadmap["milestones"]:
    mid, card = ms["id"], cards.get(ms["id"], "")
    if not card:
        fail.append(f"{mid}: no card"); continue
    req, done = ms["issues"]["required"], ms["issues"]["by_status"].get("DONE", 0)
    bars = re.findall(r'<progress[^>]*value="(\d+)" max="(\d+)"', card)
    if req > 0 and bars != [(str(done), str(req))]:
        fail.append(f"{mid}: bar {bars}, expected one of {done}/{req}")
    if req == 0 and bars:
        fail.append(f"{mid}: a bar on a milestone with no required issue")
    when = re.search(r'data-milestone-when>(.*?)</p>', card, re.S)
    when = when.group(1) if when else ""
    if ms["released_in"] != "unreleased":
        rec = os.path.join(rel, ms["released_in"] + ".json")
        if os.path.exists(rec):
            pub = json.load(open(rec, encoding="utf-8"))["published_at"]
            if f'datetime="{pub}"' not in when or not when.strip().startswith("released"):
                fail.append(f"{mid}: does not read 'released' at {pub}: {when.strip()[:80]}")
    elif delivery:
        dates = sorted(d["merged"]["merged_at"] for d in delivery["issues"]
                       if d.get("milestone") == mid and d.get("merged") and d["merged"].get("merged_at"))
        if dates and f'datetime="{dates[-1]}"' not in when:
            fail.append(f"{mid}: does not read its last delivery {dates[-1]}: {when.strip()[:80]}")
    if ms["status"] == "DONE" and ms["issues"]["total"] == 0 and "data-milestone-issueless" not in card:
        fail.append(f"{mid}: accepted with no issues, not read as delivered")
dependent = sum(1 for ms in roadmap["milestones"] if ms["depends_on"])
if f'data-roadmap-dependents="{dependent}"' not in page:
    fail.append(f"the dependency count is not the {dependent} roadmap.json holds")
# the prose only: an issue's own title may quote a withdrawn sentence (I3773's does)
prose = re.sub(r'<li[^>]*data-issue="[^"]*".*?</li>', '', page, flags=re.S)
for gone in ("depends on the ones before it", "no progress bar", "would be a lie"):
    if gone in prose:
        fail.append(f"the withdrawn sentence is still there: '{gone}'")
if fail:
    print("    " + "\n    ".join(fail[:20])); sys.exit(1)
print(f"    {len(cards)} milestone card(s) hold their bar, their date and the counted dependencies")
EOF

# the Czech memory carries the new sentences, and none of their keys holds a number or a date:
# a count or a date is an opaque element (translate="no"), so a new count reuses the translation
node --input-type=module - "$ROOT/scripts/lib/site-i18n.mjs" "$R" "$ROOT/site/i18n/cs/memory" <<'EOF' || exit 1
const [lib, page, memdir] = process.argv.slice(2);
const fs = await import('node:fs');
const m = await import(lib);
const mem = m.loadMemory(memdir);
const segs = m.segments(fs.readFileSync(page, 'utf8'));
// exactly the sentences this page adds, not any title that happens to share a word with them
const mine = /^(released <t1\/?>|last delivery <t1\/?>|nothing delivered yet$|no delivery date: |<t1\/> of <t2\/> milestones depend on another|delivered — accepted before its work|A milestone here is an outcome)/;
let bad = [];
for (const s of segs) {
  const en = s.source ?? s.en ?? '';
  if (!mine.test(en)) continue;
  if (/\d/.test(en.replace(/<\/?t\d+\/?>/g, ''))) bad.push(`a number or date in the segment: ${en.slice(0, 80)}`);
  if (!mem.has(s.key ?? s.k)) bad.push(`no Czech for: ${en.slice(0, 80)}`);
}
if (bad.length) { console.log('    ' + [...new Set(bad)].slice(0, 12).join('\n    ')); process.exit(1); }
console.log('    every new sentence has its Czech, keyed without a number');
EOF

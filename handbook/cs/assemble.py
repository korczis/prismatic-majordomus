import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
GEN = os.environ.get("HANDBOOK_DERIVED", HERE)        # what handbook/build.py derived from the tree
harness_path, memory_path, mesh_path = (os.path.join(HERE, "sources", n) for n in ("harness.html", "pamet.html", "mesh.html"))
new_path, mem_css_path = os.path.join(HERE, "new-sections.html"), os.path.join(HERE, "mem.css")
out_path = sys.argv[1]
template_path = os.path.join(HERE, "prirucka-majordomusu.html.tmpl")
sys.path.insert(0, HERE)
from tldr import TLDR


def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


SECTION_RE = re.compile(r'<section id="([^"]+)">(.*?)</section>', re.S)


def sections(html, prefix=None):
    found = {}
    for sid, body in SECTION_RE.findall(html):
        if prefix:
            body = re.sub(r'id="([^"]+)"', lambda m: 'id="%s-%s"' % (prefix, m.group(1)), body)
            body = re.sub(r'href="#([^"]+)"', lambda m: 'href="#%s-%s"' % (prefix, m.group(1)), body)
            body = re.sub(r'url\(#([^)]+)\)', lambda m: 'url(#%s-%s)' % (prefix, m.group(1)), body)
            sid = "%s-%s" % (prefix, sid)
        found[sid] = body
    return found


all_sections = {}
all_sections.update(sections(read(harness_path), "h"))
all_sections.update(sections(read(memory_path), "p"))
all_sections.update(sections(read(mesh_path), "m"))
all_sections.update(sections(read(new_path)))
all_sections.update(sections(read(os.path.join(os.path.dirname(os.path.abspath(__file__)), "sections-ops.html"))))
all_sections.update(sections(read(os.path.join(os.path.dirname(os.path.abspath(__file__)), "sections-stealth.html"))))
all_sections.update(sections(read(os.path.join(os.path.dirname(os.path.abspath(__file__)), "sections-reader.html"))))
all_sections.update(sections(read(os.path.join(os.path.dirname(os.path.abspath(__file__)), "sections-flows.html"))))
all_sections.update(sections(read(os.path.join(os.path.dirname(os.path.abspath(__file__)), "sections-graph.html"))))


def replace(sid, old, new):
    body = all_sections[sid]
    if old not in body:
        sys.exit("missing text in %s: %r" % (sid, old[:60]))
    all_sections[sid] = body.replace(old, new, 1)


DOSAH = """<h3>Dosah serveru</h3>
      <p>Výchozí je loopback. Aby se k serveru dalo připojit z jiného stroje (a aby se k němu dostal <a href="#m-vrstvy">mesh</a>), musí poslouchat na síti: <code>serve --host 0.0.0.0</code> pro jeden proces, nebo <code>export MAJORDOMUS_HTTP_HOST=0.0.0.0</code> v profilu shellu pro všechny servery stroje. Tohle nastavení patří stroji, ne repu: nic trackovaného ho nastavit nemůže.</p>
      <div class="code"><button type="button">Kopírovat</button><pre><code>majordomus serve status          <span class="c"># všechny checkouty repa: lease, adresa, stav</span>
majordomus serve ensure          <span class="c"># nastartuj, pokud neběží (to dělá session start)</span>
majordomus serve --host 0.0.0.0  <span class="c"># dosažitelný z LAN / tailnetu</span>
majordomus serve stop
majordomus mcp                   <span class="c"># MCP klient; první se stane serverem</span></code></pre></div>
      <h3>Board: kdo na čem pracuje</h3>"""

replace("h-server", "<h3>Board: kdo na čem pracuje</h3>", DOSAH)
replace("h-server", "<code>majordomus_mesh_claim</code> a jeho odmítnutí",
        '<a href="#m-claims"><code>majordomus_mesh_claim</code></a> a jeho odmítnutí')
replace("h-mapa", "dává až <code>majordomus_mesh_claim</code>.",
        'dává až <a href="#m-claims"><code>majordomus_mesh_claim</code></a>.')
replace("h-vynuceni", "Podrobněji ve stránce o paměti a pravidlech.",
        'Podrobněji v kapitole <a href="#p-pravidla">Pravidla a doktríny</a>.')
replace("h-finish", "Ostatní výsledky jsou poctivé přiznání",
        'Ostatní výsledky (<a href="#h-finish-outcomes">tabulka níže</a>) jsou poctivé přiznání')
replace("h-finish", "<h3>Výsledky</h3>", '<h3 id="h-finish-outcomes">Výsledky</h3>')
replace("h-prubeh", "<code>majordomus context</code> dá briefing v pořadí autority",
        '<code>majordomus context</code> dá <a href="#p-briefing">briefing</a> v pořadí autority')
replace("m-claims", '<text x="622" y="196" font-size="12" font-weight="600" class="ink">B může claimnout</text>',
        '<text x="619" y="196" font-size="11" font-weight="600" class="ink">B může claimnout</text>')
replace("p-rozhodnuti", "v repu Majordomusu jich je 85 (12. 10. 2026)", 'v repu Majordomusu jich je 95 (12. 10. 2026, <a href="#cisla-adr">graf</a>)')
replace("h-server", "V repu Majordomusu je ze 177 MCP nástrojů jen šest typu <code>repository_mutation</code>.", "Zapisujících nástrojů je hrstka: lokální server verze 0.21.0 jich mezi 177 MCP nástroji hlásil šest.")
replace("h-doctor", "První tři nejdou vyrobit bez důkazu.", 'První tři nejdou vyrobit bez <a href="#x-evidence">důkazu</a>.')
replace("h-finish", "<td>každý závazek z <code>--requires</code> je splněný</td>", '<td>každý <a href="#h-zavazky">závazek</a> z <code>--requires</code> je splněný</td>')
replace("h-git", "Feature branch se commituje jen z jejího kanonického worktree", 'Feature branch se commituje jen z jejího kanonického <a href="#x-worktree">worktree</a>')
replace("h-prubeh", "Kroky 1, 2 a 9 se dějí samy", "Kroky 1, 2 a 10 se dějí samy")
replace("p-stroje", "<h2>Napříč stroji</h2>", "<h2>Kontinuita napříč stroji</h2>")
replace("p-stroje", "(tím samým, který používá mesh)",
        '(tím samým, který používá <a href="#m-identita">mesh</a>)')
replace("p-stroje", "continuity záznam nebo mesh handover",
        'continuity záznam nebo <a href="#m-handovery">mesh handover</a>')
replace("p-hranice", "<h2>Co to nedělá</h2>", "<h2>Co paměť a pravidla nedělají</h2>")
replace("p-prikazy", "<h2>Příkazy</h2>", "<h2>Příkazy: paměť a pravidla</h2>")
replace("m-prikazy", "<h2>Příkazy</h2>", "<h2>Příkazy: mesh</h2>")
replace("m-zaruky", "<h2>Záruky a limity</h2>", "<h2>Záruky a limity meshe</h2>")
replace("m-cesta", "s osmi kontrolami na každé straně (viz níže)",
        's osmi kontrolami na každé straně (<a href="#m-handshake">viz níže</a>)')
replace("m-vrstvy", "<code>majordomus context</code> to ukáže v sekci <code>PEERS</code>",
        '<a href="#p-briefing"><code>majordomus context</code></a> to ukáže v sekci <code>PEERS</code>')
replace("m-handovery", "Lokální handover (<code>majordomus handover</code>)",
        'Lokální <a href="#p-zaznamy">handover</a> (<code>majordomus handover</code>)')
replace("p-pravidla", "<code>majordomus doctor</code> tento řetěz projde",
        '<a href="#h-doctor"><code>majordomus doctor</code></a> tento řetěz projde')

PARTS = [
    ("Úvod", [
        ("uvod", "Co je Majordomus"), ("problemy", "Problémy a root causes"), ("hodnoceni", "Sebehodnocení týmu"),
        ("stavba", "Celá stavba"), ("graf", "Stavba z grafu repozitáře"), ("stav", "Šest druhů stavu"),
        ("projekce", "Jeden fakt, mnoho rozhraní"), ("kudy", "Kudy číst"), ("nasazeni", "Nasazení krok za krokem"), ("osobni", "Osobní režim (BYOD)"),
        ("mapa-prirucky", "Mapa příručky")]),
    ("Harness: vstup do session", [
        ("h-co-je", "Co je harness"), ("h-proc", "Proč nestačí provider"),
        ("h-mapa", "Kontrolní body změny"), ("h-prubeh", "Session krok za krokem"),
        ("h-bootstrap", "Jedna policy, víc providerů"), ("h-hooky", "Hooky providera"),
        ("h-server", "Sdílený server a board"), ("h-scope", "Dva druhy scope")]),
    ("Harness: práce a brány", [
        ("h-profily", "Profily"), ("h-zavazky", "Závazky tasku"),
        ("h-finish", "Finish contract"), ("h-reasoning", "Nejistota a poradci"),
        ("h-git", "Git hooky"), ("h-ci", "CI gates"), ("h-doctor", "Doctor a preflight"),
        ("h-vynuceni", "Pravidlo musí něco volat"), ("h-mereni", "Co harness stojí")]),
    ("Paměť a pravidla", [
        ("p-mapa", "Mapa: kde co žije"), ("p-zaznamy", "Který záznam na co"),
        ("p-pamet", "Co je „paměť“"), ("p-session", "Session a session-context"),
        ("p-briefing", "Briefing a kontext"), ("p-rozhodnuti", "Decision a ADR"),
        ("p-znalosti", "Knowledge base"), ("p-pravidla", "Pravidla a doktríny"),
        ("p-skills", "Skills a prompty"), ("p-stroje", "Kontinuita mezi stroji")]),
    ("Koordinace a mesh", [
        ("koordinace", "Kruhy koordinace"), ("m-vrstvy", "Čtyři vrstvy"),
        ("m-identita", "Identita"), ("m-cesta", "Od objevení ke stavu"),
        ("m-handshake", "Handshake"), ("m-journal", "Journal a fold"),
        ("m-claims", "Claims a leases"), ("m-handovery", "Handovery a reviews"),
        ("m-stroje", "Topologie a nastavení"), ("m-bezpecnost", "Bezpečnost"),
        ("m-diagnostika", "Diagnostika"), ("m-overeni", "Ověřeno na železe")]),
    ("Plán, důkazy a provoz", [
        ("x-plan", "Plán: milníky a vlny"), ("x-intent", "Intents"),
        ("x-worktree", "Worktrees"), ("x-env", "Prostředí repozitáře"),
        ("x-evidence", "Důkazy"), ("x-claims", "Garance a use cases"),
        ("x-registry", "Registr zblízka"), ("x-cockpit", "Cockpit a UI"), ("x-exec", "Executions"),
        ("x-release", "Verze a vydání"), ("x-lessons", "Poučení z provozu")]),
    ("Hranice", [
        ("m-zaruky", "Záruky meshe"), ("p-hranice", "Co paměť nedělá"),
        ("h-hranice", "Co harness nedělá"), ("velka-org", "Pro velkou organizaci"),
        ("h-prenos", "Co se dá přenést")]),
    ("Repozitář v číslech", [
        ("cisla-tempo", "Tempo vývoje"), ("cisla-commity", "Commity"),
        ("cisla-adr", "Rozhodnutí v čase"), ("cisla-pravidla", "Pravidla a brány"), ("toky", "Toky spočítané z repozitáře")]),
    ("Reference", [
        ("pruzkumnik", "Průzkumník repozitáře"), ("citarna", "Čítárna dokumentů"), ("slovnik", "Slovník"), ("prikazy", "Reference příkazů"),
        ("p-prikazy", "Příkazy: paměť"), ("m-prikazy", "Příkazy: mesh")]),
]

LABELS = {sid: label for _, items in PARTS for sid, label in items}

SEE = {
    "uvod": ["problemy", "stavba", "kudy"],
    "problemy": ["hodnoceni", "h-proc", "h-prenos", "velka-org"],
    "stavba": ["graf", "projekce", "stav", "p-mapa"],
    "graf": ["stavba", "toky", "citarna", "pruzkumnik", "x-registry"],
    "stav": ["p-mapa", "p-pamet", "h-server"],
    "projekce": ["h-bootstrap", "h-git", "h-server"],
    "h-co-je": ["stavba", "h-proc"],
    "h-proc": ["problemy", "h-prenos"],
    "h-mapa": ["h-git", "h-ci", "h-finish", "koordinace"],
    "h-prubeh": ["p-session", "p-briefing", "h-server"],
    "h-bootstrap": ["p-briefing", "projekce", "h-doctor"],
    "h-hooky": ["p-session", "p-znalosti", "p-zaznamy"],
    "h-server": ["koordinace", "m-vrstvy", "m-claims", "m-stroje"],
    "h-scope": ["koordinace", "m-vrstvy", "h-finish"],
    "h-profily": ["p-briefing", "h-finish", "h-mereni"],
    "h-zavazky": ["h-finish", "h-doctor"],
    "h-finish": ["h-zavazky", "p-zaznamy", "h-git"],
    "h-reasoning": ["p-rozhodnuti", "p-znalosti"],
    "h-git": ["h-ci", "p-pravidla", "projekce"],
    "h-ci": ["h-doctor", "h-finish"],
    "h-doctor": ["h-vynuceni", "p-pravidla", "h-ci"],
    "h-vynuceni": ["p-pravidla", "h-doctor"],
    "h-mereni": ["p-briefing", "h-profily"],
    "p-mapa": ["stav", "p-zaznamy", "stavba"],
    "p-zaznamy": ["h-finish", "p-session", "m-handovery"],
    "p-pamet": ["p-znalosti", "h-hooky", "stav"],
    "p-session": ["h-hooky", "h-prubeh"],
    "p-briefing": ["h-profily", "h-scope", "h-prubeh"],
    "p-rozhodnuti": ["h-reasoning", "p-znalosti"],
    "p-znalosti": ["h-hooky", "p-pamet", "p-rozhodnuti"],
    "p-pravidla": ["h-vynuceni", "h-doctor", "h-git"],
    "p-skills": ["h-finish", "h-profily"],
    "p-stroje": ["m-handovery", "m-stroje", "m-bezpecnost"],
    "koordinace": ["m-vrstvy", "h-scope", "h-server"],
    "m-vrstvy": ["koordinace", "h-scope", "h-server"],
    "m-identita": ["m-stroje", "p-stroje"],
    "m-cesta": ["m-handshake", "m-journal"],
    "m-handshake": ["m-bezpecnost", "m-diagnostika"],
    "m-journal": ["m-claims", "stav"],
    "m-claims": ["h-server", "koordinace"],
    "m-handovery": ["p-stroje", "p-zaznamy"],
    "m-stroje": ["h-server", "m-diagnostika", "m-identita"],
    "m-bezpecnost": ["m-zaruky", "m-handshake"],
    "m-diagnostika": ["h-doctor", "m-overeni"],
    "m-overeni": ["m-diagnostika", "m-zaruky"],
    "m-zaruky": ["m-bezpecnost", "velka-org"],
    "p-hranice": ["p-pamet", "h-hranice"],
    "h-hranice": ["velka-org", "h-prenos"],
    "velka-org": ["problemy", "h-prenos"],
    "h-prenos": ["problemy", "kudy"],
    "slovnik": ["prikazy", "mapa-prirucky"],
    "x-plan": ["x-intent", "h-finish", "h-zavazky", "x-evidence"],
    "x-intent": ["x-plan", "x-evidence", "h-finish"],
    "x-worktree": ["h-git", "h-server", "koordinace", "x-env"],
    "x-env": ["h-doctor", "h-prubeh", "x-worktree"],
    "x-evidence": ["h-doctor", "x-claims", "h-zavazky", "cisla-pravidla"],
    "x-claims": ["x-evidence", "h-ci", "pruzkumnik"],
    "x-registry": ["projekce", "x-cockpit", "x-release"],
    "x-cockpit": ["h-server", "x-exec", "x-registry"],
    "x-exec": ["x-cockpit", "h-server"],
    "x-release": ["x-registry", "h-ci", "cisla-tempo", "nasazeni"],
    "x-lessons": ["problemy", "h-proc", "h-prenos", "cisla-tempo"],
    "hodnoceni": ["problemy", "h-prenos", "nasazeni"],
    "nasazeni": ["osobni", "h-bootstrap", "h-hooky", "h-git", "h-finish"],
    "osobni": ["nasazeni", "velka-org", "h-bootstrap", "h-server", "h-doctor", "x-cockpit"],
    "pruzkumnik": ["citarna", "toky", "p-rozhodnuti", "p-pravidla", "h-ci"],
    "citarna": ["graf", "pruzkumnik", "toky", "p-rozhodnuti", "p-pravidla"],
    "toky": ["graf", "h-vynuceni", "h-ci", "cisla-pravidla", "citarna"],
    "mapa-prirucky": ["kudy", "slovnik"],
    "cisla-tempo": ["cisla-adr", "h-ci"],
    "cisla-commity": ["h-git", "cisla-tempo"],
    "cisla-adr": ["p-rozhodnuti", "cisla-tempo"],
    "cisla-pravidla": ["h-vynuceni", "h-ci", "p-pravidla"],
    "prikazy": ["p-prikazy", "m-prikazy", "slovnik"],
    "p-prikazy": ["prikazy", "m-prikazy"],
    "m-prikazy": ["prikazy", "p-prikazy"],
}

ordered = [sid for _, items in PARTS for sid, _ in items]
missing = [sid for sid in ordered if sid not in all_sections]
if missing:
    sys.exit("missing sections: %s" % missing)
for sid, targets in SEE.items():
    for t in targets:
        if t not in LABELS:
            sys.exit("see-also target %s from %s is not a chapter" % (t, sid))

WIDGETS = dict(re.findall(r'<!--@([a-z0-9-]+)-->(.*?)<!--@end-->', read(os.path.join(HERE, "widgets.html")) + read(os.path.join(HERE, "widgets-stealth.html")), re.S))
for sid in WIDGETS:
    if sid not in all_sections:
        sys.exit("widget for unknown section " + sid)
missing_tldr = [sid for _, items in PARTS for sid, _ in items if sid not in TLDR]
if missing_tldr:
    sys.exit("missing tldr: %s" % missing_tldr)

def esc(t):
    return t.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")

main_parts = []
nav_parts = []
for n, (title, items) in enumerate(PARTS, 1):
    main_parts.append('<p class="part" id="cast-%d">Část %d · %s</p>' % (n, n, title))
    main_parts.append('<div class="part-grid">%s</div>' % "".join(
        '<a class="pg" href="#%s"><b>%s</b><span>%s</span></a>' % (sid, label, esc(TLDR[sid][0])) for sid, label in items))
    links = []
    for sid, label in items:
        body = all_sections[sid].rstrip()
        t, imp = TLDR[sid]
        bubble = ('<aside class="tldr" aria-label="Shrnutí"><p><span class="k t">TL;DR</span>%s</p>'
                  '<p><span class="k i">Dopad</span>%s</p></aside>' % (esc(t), esc(imp)))
        body = re.sub(r'(</h2>)', r'\1\n      ' + bubble.replace('\\', '\\\\'), body, count=1)
        if sid in WIDGETS:
            body += "\n      " + WIDGETS[sid].strip()
        see = SEE.get(sid)
        if see:
            body += '\n      <aside class="see"><span>Souvisí</span> %s</aside>\n    ' % " · ".join(
                '<a href="#%s">%s</a>' % (t, LABELS[t]) for t in see)
        k = ordered.index(sid)
        prev_a = '<a class="pv" href="#%s">← %s</a>' % (ordered[k - 1], LABELS[ordered[k - 1]]) if k > 0 else ''
        next_a = '<a class="nx" href="#%s">%s →</a>' % (ordered[k + 1], LABELS[ordered[k + 1]]) if k + 1 < len(ordered) else ''
        body += '\n      <nav class="pn" aria-label="Další kapitoly">%s%s</nav>\n    ' % (prev_a, next_a)
        main_parts.append('<section id="%s">%s</section>' % (sid, body))
        links.append('<li><a href="#%s">%s</a></li>' % (sid, label))
    nav_parts.append('<p class="grp"><a href="#cast-%d">%d · %s</a></p>\n<ol>%s</ol>' % (
        n, n, title, "".join(links)))

html = "\n".join(main_parts)
ids = re.findall(r'id="([^"]+)"', html)
dupes = sorted({i for i in ids if ids.count(i) > 1})
if dupes:
    sys.exit("duplicate ids: %s" % dupes)
idset = set(ids)
broken = sorted({h for h in re.findall(r'href="#([^"]+)"', html) if h not in idset})
if broken:
    sys.exit("broken anchors: %s" % broken)
broken_urls = sorted({u for u in re.findall(r'url\(#([^)]+)\)', html) if u not in idset})
if broken_urls:
    sys.exit("broken svg refs: %s" % broken_urls)

GH = "https://github.com/korczis/prismatic-majordomus"
TRACKED = set(read(os.path.join(GEN, "tracked-new.txt")).split())
DIRS = set()
for f in TRACKED:
    parts_ = f.split("/")
    for i in range(1, len(parts_)):
        DIRS.add("/".join(parts_[:i]))
ADRS = {}
for f in TRACKED:
    m = re.match(r"\.ai/repo/adrs/(\d{4})-", f)
    if m:
        ADRS[m.group(1)] = f
link_count = {"src": 0, "adr": 0}

RULE_PATHS = {r["id"]: r["p"] for r in json.loads(read(os.path.join(GEN, "explorer-data.json")))["rules"]}

def link_code(m):
    text = m.group(1)
    rid = text.split("@")[0]
    if rid in RULE_PATHS:
        link_count["src"] += 1
        return '<a class="src" href="%s/blob/master/%s"><code>%s</code></a>' % (GH, RULE_PATHS[rid], text)
    path = text.rstrip("/")
    if path in TRACKED:
        link_count["src"] += 1
        return '<a class="src" href="%s/blob/master/%s"><code>%s</code></a>' % (GH, path, text)
    if path in DIRS and "/" in path or path in (".githooks", ".claude", "docs", "share"):
        if path in DIRS:
            link_count["src"] += 1
            return '<a class="src" href="%s/tree/master/%s"><code>%s</code></a>' % (GH, path, text)
    return m.group(0)

def link_adr(m):
    num = m.group(2)
    if num in ADRS:
        link_count["adr"] += 1
        return '%s<a class="src" href="%s/blob/master/%s">%s</a>' % (m.group(1), GH, ADRS[num], num)
    return m.group(0)

def link_text(seg):
    seg = re.sub(r"<code>([^<>*\s]+)</code>", link_code, seg)
    seg = re.sub(r"(ADR\s|ADR )(\d{4})", link_adr, seg)
    return seg

def link_sources(page):
    head, sep, rest = page.partition("<main>")
    tokens = re.split(r"(<pre[\s\S]*?</pre>|<svg[\s\S]*?</svg>|<a\s[^>]*>[\s\S]*?</a>|<script[\s\S]*?</script>|<textarea[\s\S]*?</textarea>)", rest)
    out = []
    for i, t in enumerate(tokens):
        out.append(t if i % 2 else link_text(t))
    rest = "".join(out)
    def footer_adrs(m):
        plain = re.sub(r'<a class="src" href="[^"]*">(\d{4})</a>', r'\1', m.group(0))
        return re.sub(r"\b(0\d{3})\b", lambda n: ('<a class="src" href="%s/blob/master/%s">%s</a>' % (GH, ADRS[n.group(1)], n.group(1))) if n.group(1) in ADRS else n.group(0), plain)
    rest = re.sub(r"<p>Rozhodnutí: ADR[^<]*(?:<a[^>]*>[^<]*</a>[^<]*)*</p>", footer_adrs, rest)
    return head + sep + rest

short = {"Úvod": "Úvod", "Harness: vstup do session": "Harness I", "Harness: práce a brány": "Harness II",
         "Paměť a pravidla": "Paměť", "Koordinace a mesh": "Mesh", "Hranice": "Hranice", "Plán, důkazy a provoz": "Provoz",
         "Repozitář v číslech": "Čísla", "Reference": "Reference"}
edges = set()
for a, bs in SEE.items():
    for b in bs:
        edges.add(tuple(sorted((a, b))))
graph = {"parts": [{"name": t, "short": short[t], "count": len(items)} for t, items in PARTS],
         "nodes": [{"id": sid, "label": label, "part": n} for n, (_, items) in enumerate(PARTS) for sid, label in items],
         "edges": sorted(edges)}
gates_json = json.loads(read(os.path.join(GEN, "gates-data.json")))
link_candidates = set(g["run"].split(" ")[0] for g in gates_json["gates"] if g.get("run"))
link_candidates.update(["docs/INSTALL.md", "docs/ADOPTION.md", ".ai/repo/policy.yaml", ".githooks/pre-commit",
                        ".claude/settings.json", ".mcp.json", ".ai/repo/workflows/task-lifecycle.md"])
arch_json = json.loads(read(os.path.join(HERE, "arch.json")))
for layer in arch_json:
    for item in layer["items"]:
        link_candidates.update(item.get("paths", []))
DATA = (
    ("data-arch", arch_json),
    ("data-graph", graph),
    ("data-repo", json.loads(read(os.path.join(GEN, "repo-data.json")).replace('"other"', '"ostatní"'))),
    ("data-gates", gates_json),
    ("data-explore", json.loads(read(os.path.join(GEN, "explorer-data.json")))),
    ("data-tracked", sorted(p for p in link_candidates if p in TRACKED or p.rstrip("/") in DIRS)),
)
CORPUS_INDEX = read(os.path.join(GEN, "corpus-index.json"))
REPO_PATHS = [p for p in read(os.path.join(GEN, "tracked-new.txt")).split("\n") if p]
data_scripts = "".join('<script type="application/json" id="%s">%s</script>\n'
                       % (k, json.dumps(v, ensure_ascii=False).replace("</", "<\\/")) for k, v in DATA)
data_scripts += '<script type="application/json" id="data-corpus-index">%s</script>\n' % CORPUS_INDEX.replace("</", "<\\/")
data_scripts += '<script type="application/json" id="data-repo-paths">%s</script>\n' % json.dumps(REPO_PATHS).replace("</", "<\\/")
data_scripts += '<script type="application/octet-stream" id="data-graphdb">%s</script>\n' % read(os.path.join(GEN, "graph.b64"))
data_scripts += '<script type="application/octet-stream" id="data-corpus">%s</script>\n' % read(os.path.join(GEN, "corpus.b64"))
ROOT = os.environ.get("HANDBOOK_ROOT") or os.path.dirname(os.path.dirname(HERE))
# The design is declared once, in share/design/tokens.yaml. The projection that gives its
# roles their values is inlined as generated, and the class that selects the dark theme is
# read from the declaration; this edition chooses neither.
DESIGN = read(os.path.join(ROOT, "share", "design", "surface.css"))
_theme = re.search(r"^theme:\n(?:[ \t]+.*\n)*?[ \t]+class:[ \t]*([A-Za-z0-9_-]+)", read(os.path.join(ROOT, "share", "design", "tokens.yaml")), re.M)
if not _theme:
    sys.exit("share/design/tokens.yaml declares no theme class")
THEME_CLASS = _theme.group(1)
css = DESIGN + "\n" + "\n".join(line for line in read(mem_css_path).splitlines() if not line.startswith("<link"))

template = read(template_path)
page = (template.replace("/*BASE_CSS*/", css)
        .replace("<!--NAV-->", "\n".join(nav_parts))
        .replace("<!--MAIN-->", html)
        .replace("/*WIDGETS_CSS*/", read(os.path.join(HERE, "widgets.css")))
        .replace("<!--DATA-->", data_scripts)
        .replace("/*WIDGETS_JS*/", read(os.path.join(HERE, "widgets.js")))
        .replace("/*EXTRAS_JS*/", read(os.path.join(HERE, "extras.js")) + "\n" + read(os.path.join(HERE, "extras2.js")) + "\n" + read(os.path.join(HERE, "extras3.js")) + "\n" + read(os.path.join(HERE, "extras4.js")) + "\n" + read(os.path.join(HERE, "extras5.js")) + "\n" + read(os.path.join(HERE, "extras6.js")) + "\n" + read(os.path.join(HERE, "extras7.js")) + "\n" + read(os.path.join(HERE, "extras8.js")) + "\n" + read(os.path.join(HERE, "extras9.js")) + "\n" + read(os.path.join(HERE, "extras10.js")) + "\n" + read(os.path.join(HERE, "extras11.js")) + "\n" + read(os.path.join(HERE, "extras12.js")) + "\n" + read(os.path.join(HERE, "extras13.js")) + "\n" + read(os.path.join(HERE, "extras14.js"))))
page = link_sources(page).replace("@@THEME_CLASS@@", THEME_CLASS)
# every number a caption states is a fact of the data the page embeds, never typed
_ex = json.loads(read(os.path.join(GEN, "explorer-data.json")))
_ci = json.loads(CORPUS_INDEX)
FACTS = {"adrs": len(_ex["adrs"]), "rules": len(_ex["rules"]), "caps": len(_ex["caps"]), "claims": len(_ex["claims"]),
         "ucs": len(_ex["ucs"]), "gates": len(gates_json["gates"]), "classes": len(gates_json["classes"]),
         "blocking": sum(1 for r in _ex["rules"] if r.get("c") == "blocking"),
         "advisory": sum(1 for r in _ex["rules"] if r.get("c") == "advisory"),
         "docs": len(_ci["docs"]), "commit": _ci["commit"]}
for k, v in FACTS.items():
    page = page.replace("%%" + k + "%%", str(v))
left = sorted(set(re.findall(r"%%[a-z]+%%", page)))
if left:
    sys.exit("unknown fact placeholder(s): %s" % left)

# ---- what each chapter rests on, found in the repository's own graph ----
_g = json.loads(read(os.path.join(GEN, "graph-src.json")))["graph"]
_nodes = [n for n in _g["nodes"] if n["kind"] != "kind"]          # the order graph.py embeds
_pos = {n["id"]: i for i, n in enumerate(_nodes)}
_by_label, _by_source = {}, {}
for i, n in enumerate(_nodes):
    _by_label.setdefault((n["kind"], n.get("label", "")), i)
    if n.get("source"):
        _by_source.setdefault(n["source"], []).append(i)
_caps = {n["label"].replace(".", "_").replace("-", "_"): i for i, n in enumerate(_nodes) if n["kind"] == "capability"}
_rules = {n["label"].split("@")[0]: i for i, n in enumerate(_nodes) if n["kind"] == "rule"}
_adj = {}
_FOLLOW = {"tested_by", "implemented_by", "put_in_force", "exercises", "evidences", "defined_in", "composes", "runs"}
_NEAR = {"test", "claim", "rule", "adr", "implementation", "capability", "use-case", "command", "module"}
for e in _g["edges"]:
    if e["kind"] in _FOLLOW and e["source"] in _pos and e["target"] in _pos:
        _adj.setdefault(_pos[e["source"]], set()).add(_pos[e["target"]])
        _adj.setdefault(_pos[e["target"]], set()).add(_pos[e["source"]])

def chapter_seeds(body):
    found = set()
    for path in re.findall(r"/(?:blob|tree)/master/([^\"#?]+)", body):
        hits = _by_source.get(path, [])
        if 0 < len(hits) <= 3:
            found.update(hits)
        m = re.match(r"docs/claims/([a-z0-9-]+)\.md$", path)
        if m and ("claim", m.group(1)) in _by_label:
            found.add(_by_label[("claim", m.group(1))])
    for num in re.findall(r"adrs/(\d{4})-", body) + re.findall(r"ADR\s(?:<a[^>]*>)?(\d{4})", body):
        if ("adr", "adr-" + num) in _by_label:
            found.add(_by_label[("adr", "adr-" + num)])
    text = re.sub(r"<[^>]+>", " ", body)
    for rid in re.findall(r"\b((?:project|majordomus|ai)\.[a-z0-9][a-z0-9-]+)", text):
        if rid in _rules:
            found.add(_rules[rid])
    for cmd in re.findall(r"\bmajordomus ([a-z][a-z-]+)", text):
        if ("command", cmd) in _by_label:
            found.add(_by_label[("command", cmd)])
    for tool in re.findall(r"\bmajordomus_([a-z_]+)", text):
        if tool in _caps:
            found.add(_caps[tool])
    for code in re.findall(r"<code>([^<]{2,80})</code>", body):
        words = code.replace("majordomus ", "").split()
        if not words or not re.match(r"^[a-z][a-z0-9_-]*$", words[0]):
            continue
        if len(words) > 1 and re.match(r"^[a-z][a-z0-9_-]*$", words[1]) and ("capability", words[0] + "." + words[1]) in _by_label:
            found.add(_by_label[("capability", words[0] + "." + words[1])])
        elif len(words) > 1 and ("module", words[0]) in _by_label:
            found.add(_by_label[("module", words[0])])
    for code in re.findall(r"<code>([A-Za-z0-9_./-]+\.[a-z]{2,5})</code>", body):
        hits = _by_source.get(code, [])
        if 0 < len(hits) <= 3:
            found.update(hits)
    for code in re.findall(r"<code>([a-z][a-z0-9.-]+)</code>", body):
        for kind in ("capability", "claim", "use-case", "skill"):
            if (kind, code) in _by_label:
                found.add(_by_label[(kind, code)])
    return found

from chapter_sources import SOURCES
_KIND = {"doc": "document", "feature": "feature", "module": "module", "command": "command", "adr": "adr",
         "rule": "rule", "claim": "claim", "usecase": "use-case", "skill": "skill", "capability": "capability"}

def declared(sid):
    out, missing = set(), []
    for ref in SOURCES.get(sid, "").split():
        kind, _, label = ref.partition(":")
        if kind == "adr":
            label = "adr-" + label
        if kind == "rule":
            i = _rules.get(label)
        else:
            i = _by_label.get((_KIND.get(kind, kind), label))
        if i is None:
            missing.append(ref)
        else:
            out.add(i)
    return out, missing

_unknown = sorted(set(SOURCES) - set(sid for _, items in PARTS for sid, _ in items))
if _unknown:
    sys.exit("chapter_sources.py names chapter(s) the handbook does not have: %s" % _unknown)
_gone = {sid: declared(sid)[1] for sid in SOURCES if declared(sid)[1]}
if _gone:
    sys.exit("declared source(s) the repository graph no longer carries: %s" % _gone)

# Chapters that are about the handbook itself, or are computed entirely in the browser.
NO_EVIDENCE_OK = {"kudy", "mapa-prirucky", "hodnoceni", "slovnik", "pruzkumnik", "citarna", "graf", "toky"}
evidence, bare = {}, []
for sid, body in re.findall(r'<section id="([^"]+)">([\s\S]*?)</section>', page):
    decl = declared(sid)[0]
    seeds = chapter_seeds(body) | decl
    if not seeds:
        if sid not in NO_EVIDENCE_OK:
            bare.append(sid)
        continue
    near = set()
    for i in seeds:
        near.update(j for j in _adj.get(i, ()) if _nodes[j]["kind"] in _NEAR)
    near -= seeds
    kinds = {}
    for i in seeds:
        kinds[_nodes[i]["kind"]] = kinds.get(_nodes[i]["kind"], 0) + 1
    evidence[sid] = {"s": sorted(seeds), "d": sorted(decl), "n": sorted(near)[:60], "nt": len(near), "k": kinds}
if bare:
    sys.exit("chapter(s) that cite nothing of the repository: %s" % bare)
page = page.replace('<script type="application/json" id="data-arch">',
                    '<script type="application/json" id="data-evidence">%s</script>\n<script type="application/json" id="data-arch">'
                    % json.dumps(evidence, separators=(",", ":")), 1)
print("evidence: %d chapter(s), %d seed object(s), %d neighbouring" % (
    len(evidence), sum(len(v["s"]) for v in evidence.values()), sum(v["nt"] for v in evidence.values())))
# The built page is a whole document: opened from a file it is read in standards mode, laid
# out at the device's width, and says which language it is in — the one edition.toml declares.
_lang = re.search(r'^\s*lang\s*=\s*"([a-z-]+)"', read(os.path.join(HERE, "edition.toml")), re.M).group(1)
page = ('<!doctype html>\n<html lang="%s">\n<head>\n<meta charset="utf-8">\n'
        '<meta name="viewport" content="width=device-width, initial-scale=1">\n</head>\n<body>\n' % _lang
        + page + "\n</body>\n</html>\n")
with open(out_path, "w", encoding="utf-8") as fh:
    fh.write(page)
print("links", link_count, "sections", len(ordered), "ids", len(ids), "bytes", len(page.encode("utf-8")))

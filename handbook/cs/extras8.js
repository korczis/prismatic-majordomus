(function () {
  'use strict';
  var GH = 'https://github.com/korczis/prismatic-majordomus/blob/master/';
  var SITE = 'https://majordomus.dev/guarantees/';
  function $(id) { return document.getElementById(id); }
  function el(tag, attrs, parent, text) {
    var e = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (text !== undefined) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  function clear(e) { while (e.firstChild) e.removeChild(e.firstChild); }
  function store(key, val) {
    try {
      if (val === undefined) return window.localStorage.getItem(key);
      if (val === null) window.localStorage.removeItem(key); else window.localStorage.setItem(key, val);
    } catch (e) { return null; }
    return null;
  }
  function jump(id) { if (window.mjJump) window.mjJump(id); }
  function chap(parent, id, label) {
    var a = el('a', { href: '#' + id }, parent, label);
    a.addEventListener('click', function (e) { e.preventDefault(); jump(id); });
    return a;
  }
  // evidence kinds: g guarantee page, f file in the repository, r rule file
  function evidence(parent, ev) {
    var a;
    if (ev[0] === 'g') { a = el('a', { href: SITE + ev[1] + '/', 'class': 'ev-l g' }, parent); el('span', { 'class': 'ev-k' }, a, 'garance'); a.appendChild(document.createTextNode(ev[1])); }
    else if (ev[0] === 'r') { a = el('a', { href: GH + ev[2], 'class': 'ev-l r' }, parent); el('span', { 'class': 'ev-k' }, a, 'pravidlo'); a.appendChild(document.createTextNode(ev[1])); }
    else { a = el('a', { href: GH + ev[1], 'class': 'ev-l f' }, parent); el('span', { 'class': 'ev-k' }, a, ev[2] || 'kód'); a.appendChild(document.createTextNode(ev[1])); }
    return a;
  }

  var host = $('lab-osobni-nav');
  if (!host) return;
  // the navigator belongs at the top of the chapter, right under the note on what was measured
  var sec = $('osobni'), firstNote = sec && sec.querySelector(':scope > .note');
  if (firstNote) sec.insertBefore(host, firstNote.nextSibling);

  var WHO = [
    { id: 'eval', t: 'Zkouším, než navrhnu týmu', s: 'Myslíš, že by týmu pomohl pořádek v práci s AI agenty, ale nemáš co ukázat a nechceš otevírat debatu o nástroji naprázdno.',
      gets: ['Dva až čtyři týdny vlastních tasků se scope a finish contractem na skutečném repu.', 'Handovery a rozhodnutí jako ukázka, ne jako slib.', 'Sebehodnocení týmu vyplněné podle zkušenosti.'],
      uses: [['h-finish', 'Finish contract'], ['p-zaznamy', 'Handovery'], ['hodnoceni', 'Sebehodnocení'], ['nasazeni', 'Běžné nasazení']],
      watch: 'Až to navrhneš, řekni rovnou, že jsi to zkoušel lokálně. Je to silnější argument než utajení.' },
    { id: 'contractor', t: 'Kontraktor nebo konzultant u klienta', s: 'Pracuješ v repozitáři klienta. Konvence, soubory s instrukcemi i CI patří jemu a nejsou tvoje k rozšiřování.',
      gets: ['Stejný životní cyklus tasku u každého klienta.', 'Briefing na začátku a handover na konci každé session, takže po týdnu u jiného klienta navážeš.', 'Strom klienta zůstane přesně takový, jaký jsi ho našel.'],
      uses: [['p-briefing', 'Briefing'], ['p-zaznamy', 'Handover'], ['h-scope', 'Scope'], ['p-rozhodnuti', 'Rozhodnutí']],
      watch: 'Smlouva a pravidla klienta pro nástroje a AI platí i pro lokální použití. Při odchodu vrstvu smaž, nebo si ji odnes podle dohody.' },
    { id: 'audit', t: 'Audit, review, due diligence', s: 'Zadání je číst a posoudit. Měnit repozitář se nesmí a někdy se nesmí ani nic přidat.',
      gets: ['Nálezy, rozhodnutí a otevřené otázky jako typované záznamy vázané na commit.', 'Štítek divergence řekne, že se kód pod nálezem mezitím pohnul.', 'Scope repozitáře drží tajemství a generované soubory mimo dosah agenta.'],
      uses: [['p-pamet', 'Štítky divergence'], ['p-rozhodnuti', 'Decision'], ['h-scope', 'Scope repozitáře'], ['p-znalosti', 'Knowledge']],
      watch: 'Znalosti a nálezy existují jen na tvém disku. Výstup auditu z nich musíš sestavit a odevzdat sám.' },
    { id: 'oss', t: 'Přispěvatel do open source', s: 'Upstream nechce soubory tvého nástroje ve svém stromu, a právem. Pull request má obsahovat jen změnu.',
      gets: ['Scope udrží příspěvek u cest, kterých se týká.', 'Finish contract si vyžádá ověřovací příkaz dřív, než otevřeš pull request.', 'Lokální pre-push hook zastaví push nedokončeného tasku.'],
      uses: [['h-scope', 'Scope tasku'], ['h-finish', 'Finish contract'], ['h-git', 'Git hooky']],
      watch: 'Commit zprávy a trailery se řídí pravidly upstreamu. Nepřenášej do nich konvence z jiného repa.' },
    { id: 'onboard', t: 'Nový v cizí kódové bázi', s: 'Přebíráš kód, který jsi nepsal. Co zjistíš dnes, příští týden zapomeneš a agent to neví vůbec.',
      gets: ['Co zjistíš, se zapíše jednou jako rozhodnutí a znalosti.', 'Další session to dostane v briefingu místo toho, aby to zjišťovala znovu.', 'Kontextové dokumenty po adresářích: co platí kde.'],
      uses: [['p-znalosti', 'Knowledge base'], ['p-briefing', 'Scoped context'], ['p-session', 'Session']],
      watch: 'Až kódu porozumíš, zvaž, co z poznámek patří týmu: do dokumentace repa, ne do tvé soukromé vrstvy.' },
    { id: 'frozen', t: 'Regulované nebo zamrzlé repo', s: 'Přidání souboru je změnové řízení: regulovaný kód, zamrzlá release větev, dodávka od vendora.',
      gets: ['Vrstva je vedle kódu, ne v něm, takže není co schvalovat.', 'Scope tasku hlídá, že oprava zůstane v cestách schválené změny.', 'Odstranění je smazání adresáře.'],
      uses: [['h-scope', 'Scope'], ['h-finish', 'Finish contract'], ['h-zavazky', 'Závazky']],
      watch: 'V regulovaném prostředí si ověř, jestli smí k repu jakýkoli další nástroj a jestli smí AI klient kód číst.' },
    { id: 'agent', t: 'Sám s AI agentem', s: 'Pracuješ s Claude Code v týmovém repu. Chceš, aby agent znal tvoje pravidla a nevybíhal mimo zadání, a nechceš měnit týmové instrukce.',
      gets: ['Agent čte tvůj kontext z CLAUDE.local.md vedle týmového CLAUDE.md.', 'Scope tasku odmítne změnu mimo cesty, které jsi určil.', 'Finish odmítne „hotovo“ bez skutečného ověření.'],
      uses: [['h-bootstrap', 'Projekce instrukcí'], ['h-scope', 'Scope'], ['h-finish', 'Finish contract'], ['h-profily', 'Profily']],
      watch: 'Server spusť dřív než klienta a s --discovery filesystem, jinak agent přes MCP uvidí prázdnou vrstvu.' },
    { id: 'lead', t: 'Vlastní přehled nad repem týmu', s: 'Chceš vidět do vlastní práce: co máš rozdělané, co čeká, kde jsou tvoje worktrees a větve.',
      gets: ['Plán a issues, které si napíšeš, s odvozeným stavem v /ui a Cockpitu.', 'Přehled worktrees a branchí z gitu.', 'Historie vlastních sessions a rozhodnutí.'],
      uses: [['x-plan', 'Plán'], ['x-cockpit', 'Cockpit a UI'], ['x-worktree', 'Worktrees']],
      watch: 'Do práce kolegů nevidíš víc, než říká git. Board ukáže jen tvoje sessions. Sdílený přehled potřebuje běžné nasazení.' }
  ];
  var WHY = [
    { t: 'Diff repozitáře je prázdný', d: 'init zapíše adresář .ai/ a dva řádky do .gitignore. Po vrácení .gitignore a jednom řádku v .git/info/exclude je git status čistý.', m: 'změřeno',
      ev: [['f', 'lib/init.sh', 'kód'], ['f', 'test/cases/01_init.sh', 'test'], ['g', 'local-state-ignored'], ['g', 'ai-layer-manifest']] },
    { t: 'Do repa se nic neinstaluje', d: 'init neinstaluje nástroj, hook ani soubor shellu. Hooky jmenuje doctor a zapojuješ je sám, do .git/hooks, které git nikdy nesdílí.', m: 'změřeno',
      ev: [['f', 'lib/init.sh', 'kód'], ['f', 'docs/INSTALL.md', 'dokumentace'], ['r', 'majordomus.enforcement-wiring', '.ai/repo/rules/vendor/majordomus/rules/enforcement-wiring.v1.md']] },
    { t: 'Instrukce pro AI jdou kamkoli určí policy', d: 'Soubory s instrukcemi jsou projekce policy. Cíl projekce je jeden řádek, takže ji lze poslat do CLAUDE.local.md a týmový CLAUDE.md nechat být.', m: 'změřeno',
      ev: [['g', 'projection-generation'], ['g', 'projection-fingerprint'], ['g', 'region-projection'], ['f', 'lib/update.sh', 'kód'], ['r', 'majordomus.projection-integrity', '.ai/repo/rules/vendor/majordomus/rules/projection-integrity.v1.md']] },
    { t: 'Tvoje brány platí i bez týmu', d: 'Scope, finish contract a typované výsledky vynucuje nástroj lokálně. Ve zkoušce finish odmítl jako ověření příkaz true.', m: 'změřeno',
      ev: [['g', 'scope-enforcement'], ['g', 'finish-contract'], ['g', 'typed-outcome'], ['r', 'majordomus.scope-integrity', '.ai/repo/rules/vendor/majordomus/rules/scope-integrity.v1.md'], ['r', 'majordomus.verification-integrity', '.ai/repo/rules/vendor/majordomus/rules/verification-integrity.v1.md']] },
    { t: 'Nic neopouští stroj', d: 'Majordomus nevolá model, nedělá síťová volání a neukládá konverzace. Co odchází, odchází přes AI klienta, ne přes vrstvu.', m: 'garance',
      ev: [['g', 'no-network'], ['r', 'project.no-network-no-eval', '.ai/repo/rules/project/no-network-no-eval.v1.md'], ['r', 'project.never-store-transcripts', '.ai/repo/rules/project/never-store-transcripts.v1.md'], ['f', 'docs/CONCEPTS.md', 'dokumentace']] },
    { t: 'Server umí číst vrstvu, kterou git nezná', d: 'Výchozí zjišťování jde přes index gitu. Přepínač --discovery filesystem projde pracovní strom se stejnými vzory, takže vyloučená vrstva je vidět.', m: 'změřeno',
      ev: [['f', 'apps/majordomus-cli/src/cli.rs', 'kód'], ['f', 'apps/majordomus-cli/src/app.rs', 'kód'], ['f', 'docs/MCP.md', 'dokumentace'], ['r', 'project.the-server-sees-the-current-tree', '.ai/repo/rules/project/the-server-sees-the-current-tree.v1.md']] },
    { t: 'Agent nečte, co číst nemá', d: 'scope.yaml říká, co se nikdy neindexuje ani neservíruje: tajemství, generované soubory, binárky. Platí stejně v soukromé vrstvě.', m: 'garance',
      ev: [['g', 'scope-declared'], ['f', 'docs/SCOPE.md', 'dokumentace'], ['r', 'project.scope-is-declared', '.ai/repo/rules/project/scope-is-declared.v1.md']] },
    { t: 'Jde to vrátit beze stopy', d: 'Smazání .ai/, CLAUDE.local.md a hooku a odstranění řádků z exclude vrátí klon do původního stavu. Nic nebylo commitnuto, takže není co revertovat.', m: 'změřeno',
      ev: [['f', 'docs/ADOPTION.md', 'dokumentace']] }
  ];
  var FIT = [
    ['Task lifecycle: start, check, finish', 'ok', 'Změřeno. check viděl změněný soubor ve scope, finish --check prošel.', 'h-finish'],
    ['Scope tasku', 'ok', 'Změřeno. git status ukazoval jen skutečnou změnu kódu.', 'h-scope'],
    ['Briefing a kontext', 'ok', 'Změřeno. context vypsal git, task a profil.', 'p-briefing'],
    ['Checkpointy, handovery, rozhodnutí', 'ok', 'Lokální záznamy, gitem necestovaly ani v běžném režimu.', 'p-zaznamy'],
    ['Projekce instrukcí', 'ok', 'Změřeno. update vytvořil jen CLAUDE.local.md.', 'h-bootstrap'],
    ['Hook pre-push', 'ok', 'Změřeno. Leží v .git/hooks tohoto klonu.', 'h-git'],
    ['Server, /ui a Cockpit', 'ok', 'Změřeno s --discovery filesystem: /ui, /cockpit i API odpovídaly 200.', 'x-cockpit'],
    ['Plán a issues', 'ok', 'Lokální. /cockpit/plan odpovídal 200.', 'x-plan'],
    ['Doctor', 'part', 'Jedno trvalé selhání: README nejmenuje AGENTS.md. Nezapojuj ho do pre-commit.', 'h-doctor'],
    ['MCP nástroje pro agenta', 'part', 'Server spusť předem s --discovery filesystem. Samotné čtení vrstvy je změřené, připojený klient ne.', 'h-server'],
    ['Board', 'part', 'Funguje, ale uvidíš jen vlastní sessions.', 'h-server'],
    ['Knowledge base', 'part', 'Kandidáti vznikají lokálně a nikdo jiný je nevidí. Odvození na konci session jsem neměřil.', 'p-znalosti'],
    ['Evidence ledger', 'part', 'Lokální. Nepodpírá žádnou týmovou garanci.', 'x-evidence'],
    ['Hooky session v Claude Code', 'unk', 'Neměřeno. capture install zapisuje do adresáře .claude/ v repu.', 'h-hooky'],
    ['Commit a push session recordu', 'no', 'Policy má commit_record_on_end a push_record_on_end zapnuté. V soukromé policy je vypni. Co udělají s vyloučenou vrstvou, jsem nezkoušel.', 'p-session'],
    ['Continuity publish a sync', 'no', 'Pushuje ref refs/majordomus/continuity na sdílený remote. Viděl by ho každý.', 'p-stroje'],
    ['Mesh', 'no', 'Sdílí metadata sessions a claimů s jinými stroji. Pro jednoho člověka v cizím repu nedává smysl.', 'm-vrstvy'],
    ['CI gates', 'na', 'Soukromá vrstva nemá gates.yaml, není co plánovat. Týmové CI běží beze změny.', 'h-ci'],
    ['Worktree guard', 'na', 'Volá ho pre-commit, který tu nezapojuješ.', 'x-worktree']
  ];
  var FITL = { ok: 'funguje', part: 's výhradou', no: 'nepoužívat', unk: 'neměřeno', na: 'netýká se' };
  var HOW = [
    ['Inicializuj vrstvu a vrať .gitignore', 'majordomus init && git checkout .gitignore'],
    ['Vyluč vrstvu lokálně', "printf '.ai/\\nCLAUDE.local.md\\n' >> .git/info/exclude"],
    ['V policy nech jednu projekci a jedno vynucení', '# .ai/repo/policy.yaml: projections → CLAUDE.local.md, enforcement → finish-on-push'],
    ['Vypni commit a push session recordu', '# .ai/repo/policy.yaml: session.commit_record_on_end: false, push_record_on_end: false'],
    ['Vygeneruj instrukce', 'majordomus update'],
    ['Zapoj pre-push hook', "printf '#!/bin/sh\\nmajordomus finish --check || exit $?\\n' > .git/hooks/pre-push && chmod +x .git/hooks/pre-push"],
    ['Spusť server se čtením z disku', 'majordomus serve --discovery filesystem'],
    ['Ověř, že git nic nevidí', 'git status --short && git check-ignore -v .ai/manifest.yaml'],
    ['Zálohuj vrstvu mimo repo', 'rsync -a --delete .ai/ ~/majordomus-backup/"$(basename "$PWD")"/']
  ];

  var tabs = el('div', { 'class': 'seg', role: 'tablist' }, host);
  var body = el('div', { 'class': 'sn-body', 'aria-live': 'polite' }, host);
  var TABS = [['who', 'Pro koho'], ['why', 'Proč to jde'], ['how', 'Jak'], ['fit', 'Jak to lícuje']];
  var cur = store('mj-st-tab') || 'who', who = 0, fitFilter = null;
  var done = {};
  try { done = JSON.parse(store('mj-st-how') || '{}') || {}; } catch (e) { done = {}; }

  function renderWho() {
    var cols = el('div', { 'class': 'sn-cols' }, body);
    var list = el('div', { 'class': 'arch-col', role: 'listbox', 'aria-label': 'Situace' }, cols);
    var det = el('div', { 'class': 'arch-detail' }, cols);
    WHO.forEach(function (w, i) {
      var b = el('button', { type: 'button', role: 'option', 'class': 'arch-b', 'aria-selected': i === who ? 'true' : 'false' }, list);
      el('b', null, b, w.t);
      b.addEventListener('click', function () { who = i; render(); });
    });
    var w = WHO[who];
    el('h3', { 'class': 'h4' }, det, w.t);
    el('p', null, det, w.s);
    el('p', { 'class': 'arch-gh' }, det).appendChild(el('b', null, null, 'Co z toho máš'));
    var ul = el('ul', { 'class': 'sn-ul' }, det);
    w.gets.forEach(function (g) { el('li', null, ul, g); });
    var u = el('p', { 'class': 'sn-links' }, det);
    el('b', { 'class': 'sn-lbl' }, u, 'Co k tomu použiješ');
    w.uses.forEach(function (x, k) { if (k) u.appendChild(document.createTextNode(' · ')); chap(u, x[0], x[1]); });
    var n = el('div', { 'class': 'note' }, det);
    var np = el('p', null, n); el('strong', null, np, 'Na co si dát pozor. '); np.appendChild(document.createTextNode(w.watch));
  }
  function renderWhy() {
    el('p', { 'class': 'lab-note' }, body, 'Každé tvrzení má doklad: veřejnou garanci na majordomus.dev, pravidlo, nebo soubor v repozitáři. Co jsem změřil sám ve zkušebním repu, je označené.');
    WHY.forEach(function (y) {
      var d = el('details', { 'class': 'sn-why' }, body);
      var s = el('summary', null, d);
      el('b', null, s, y.t);
      el('span', { 'class': 'pill ' + (y.m === 'změřeno' ? 'reports' : 'refuses') }, s, y.m);
      el('p', null, d, y.d);
      var ev = el('div', { 'class': 'ev-row' }, d);
      y.ev.forEach(function (e) { evidence(ev, e); });
    });
  }
  function renderHow() {
    var n = HOW.filter(function (_, i) { return done[i]; }).length;
    var bar = el('div', { 'class': 'adopt-bar' }, body); var fill = el('i', null, bar); fill.style.width = (100 * n / HOW.length) + '%';
    el('p', { 'class': 'lab-note' }, body, n + ' z ' + HOW.length + ' kroků hotovo. Podrobný popis je níže v oddílu Postup, přesné příkazy pro tvoje repo sestaví generátor na konci kapitoly.');
    var ol = el('ol', { 'class': 'sn-how' }, body);
    HOW.forEach(function (h, i) {
      var li = el('li', { 'class': done[i] ? 'done' : '' }, ol);
      var lab = el('label', { 'class': 'chk' }, li);
      var cb = el('input', { type: 'checkbox', id: 'st-how-' + i }, lab);
      cb.checked = !!done[i];
      lab.appendChild(document.createTextNode(' ' + h[0]));
      el('code', { 'class': 'sn-cmd' }, li, h[1]);
      cb.addEventListener('change', function () { if (cb.checked) done[i] = 1; else delete done[i]; store('mj-st-how', JSON.stringify(done)); render(); });
    });
  }
  function renderFit() {
    el('p', { 'class': 'lab-note' }, body, 'Co z Majordomusu v osobním režimu funguje, co má výhradu a co nepoužívat, protože by se vrstva dostala ven.');
    var chips = el('div', { 'class': 'chips-btn' }, body);
    var all = el('button', { type: 'button', 'class': 'chip' + (fitFilter ? '' : ' on') }, chips, 'vše · ' + FIT.length);
    all.addEventListener('click', function () { fitFilter = null; render(); });
    Object.keys(FITL).forEach(function (k) {
      var c = FIT.filter(function (f) { return f[1] === k; }).length;
      if (!c) return;
      var b = el('button', { type: 'button', 'class': 'chip' + (fitFilter === k ? ' on' : '') }, chips, FITL[k] + ' · ' + c);
      b.addEventListener('click', function () { fitFilter = fitFilter === k ? null : k; render(); });
    });
    var wrap = el('div', { 'class': 'table-wrap', tabindex: '0' }, body);
    var t = el('table', null, wrap), tr = el('tr', null, el('thead', null, t));
    ['Funkce', 'V osobním režimu', 'Poznámka', 'Kapitola'].forEach(function (h) { el('th', null, tr, h); });
    var tb = el('tbody', null, t);
    FIT.filter(function (f) { return !fitFilter || f[1] === fitFilter; }).forEach(function (f) {
      var r = el('tr', null, tb);
      el('td', null, r, f[0]);
      el('span', { 'class': 'fit fit-' + f[1] }, el('td', null, r), FITL[f[1]]);
      el('td', null, r, f[2]);
      chap(el('td', null, r), f[3], '→');
    });
  }
  function render() {
    clear(body);
    Array.prototype.forEach.call(tabs.children, function (b) { b.setAttribute('aria-selected', b.dataset.k === cur ? 'true' : 'false'); });
    ({ who: renderWho, why: renderWhy, how: renderHow, fit: renderFit })[cur]();
  }
  TABS.forEach(function (t) {
    var b = el('button', { type: 'button', role: 'tab', 'class': 'seg-b' }, tabs, t[1]);
    b.dataset.k = t[0];
    b.addEventListener('click', function () { cur = t[0]; store('mj-st-tab', cur); render(); });
  });
  if (!{ who: 1, why: 1, how: 1, fit: 1 }[cur]) cur = 'who';
  render();
})();

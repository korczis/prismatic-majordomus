(function () {
  'use strict';
  /* Personal mode: the measured conversion to a committed adoption, and what to say to whom. */
  function $(id) { return document.getElementById(id); }
  function el(tag, attrs, parent, text) {
    var e = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (text !== undefined) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  function clear(e) { while (e.firstChild) e.removeChild(e.firstChild); }

  var GRAD_STEPS = [
      { t: 'Odstraň vyloučení', c: 'smaž řádky .ai/ a CLAUDE.local.md z .git/info/exclude', cmd: ['git status --short'], out: ['?? .ai/'],
        note: 'Vrstva je teď jeden netrackovaný adresář. Nic dalšího se nezměnilo.', m: 'změřeno' },
      { t: 'Vrať hranici lokálního stavu', c: 'init --extend doplní, co chybí, a nic nepřepíše', cmd: ['majordomus init --extend', 'git diff .gitignore'], out: ['+.ai/local/', '+.ai/repo/**/.tmp.*'],
        note: 'Přesně dva řádky v .gitignore. Stav checkoutu zůstane mimo git i po commitu vrstvy.', m: 'změřeno' },
      { t: 'Ověř, co by šlo do commitu', c: 'suchý běh, nic se nepřidá', cmd: ['git add -A -n | wc -l', 'git add -A -n | grep -c ".ai/local"'], out: ['97', '0'],
        note: '96 cest pod .ai/ a soubor .gitignore. Žádná z nich není pod .ai/local/.', m: 'změřeno' },
      { t: 'Vygeneruj instrukce pro tým', c: 'update podle policy', cmd: ['majordomus update'],
        out: ['create AGENTS.md', 'create CLAUDE.md', 'create GEMINI.md', 'generated 3 target(s) from policy 450bbf1919d2; each carries its own stamp'],
        note: 'S výchozí policy vzniknou tři soubory. Se zúženou policy nejdřív vrať projekce.', m: 'změřeno s výchozí policy' },
      { t: 'Zeptej se doctora', c: 'co je deklarované a není zapojené', cmd: ['majordomus doctor'],
        out: ['FAIL wiring  doctor-on-commit — hook .git/hooks/pre-commit does not exist', 'FAIL wiring  finish-on-push — hook .git/hooks/pre-push does not exist'],
        note: 'Dvě selhání, obě o hoocích. Doplň dva řádky, které init vypsal, a pokračuj kapitolou', m: 'změřeno', link: ['nasazeni', 'Nasazení krok za krokem'] }
  ];
  var SWITCH_STEPS = [
    { t: 'Založ osobní vrstvu', c: 'init --personal vytvoří vrstvu a vyloučí ji v gitu tohoto klonu', cmd: ['majordomus init --personal', 'git status --short', 'git diff --stat .gitignore', 'grep -v "^#" .git/info/exclude'], out: ['/.ai/'],
      note: 'Kód 0. Stav gitu i .gitignore jsou beze změny; v .git/info/exclude přibyl jeden označený blok.', m: 'změřeno na větvi' },
    { t: 'Vygeneruj svoje instrukce', c: 'jedna projekce do CLAUDE.local.md', cmd: ['majordomus update', 'git status --short'], out: ['create CLAUDE.local.md', 'generated 1 target(s) from policy 0d7ce6b31b8c; each carries its own stamp'],
      note: 'Soubor vznikl a blok vyloučení se o něj sám rozšířil. Stav gitu je pořád prázdný.', m: 'změřeno na větvi' },
    { t: 'Zeptej se doctora', c: 'README projektu se na AGENTS.md neptá', cmd: ['majordomus doctor'], out: ['FAIL wiring  doctor-on-commit — hook .git/hooks/pre-commit does not exist', 'FAIL wiring  finish-on-push — hook .git/hooks/pre-push does not exist', 'INFO bootstrap  README.md — not asked to name AGENTS.md: the layer is personal, and the README is the project\'s'],
      note: 'Dvě selhání, obě o hoocích. Po doplnění dvou lokálních hooků hlásí doctor 0 selhání.', m: 'změřeno na větvi' },
    { t: 'Udělej jeden task', c: 'start, check, commit, handover, finish', cmd: ['majordomus start "change the greeting" --scope app.py', 'majordomus check', 'git commit -am "feat: greeting"', 'majordomus handover < note.md', 'majordomus finish --outcome completed --verify-command "python3 app.py"', 'git ls-files .ai CLAUDE.local.md | wc -l'], out: ['check: 11 finding(s), 0 failing', 'doctor: 0 failure(s)', 'finish: t-20261012210418-b1eb completed', '0'],
      note: 'Pre-commit hook pustil doctora a commit prošel. Finish bez handoveru odmítl, s ním dokončil. Z vrstvy není v gitu nic.', m: 'změřeno na větvi' },
    { t: 'Předej vrstvu týmu', c: 'až se osvědčí: init --extend --shared', cmd: ['majordomus init --extend --shared', 'git status --short', 'git add -A -n | wc -l'], out: ['shared: the cover is removed; these paths are now the repository\'s to commit, when you choose to:', ' M .gitignore', '?? .ai/', '?? CLAUDE.local.md', '98'],
      note: 'Kryt je pryč, v .gitignore jsou dva řádky pro lokální stav, nic se necommitlo za tebe. Žádná z 98 cest není pod .ai/local/. Dál podle kapitoly', m: 'změřeno na větvi', link: ['nasazeni', 'Nasazení krok za krokem'] }
  ];
  function stepper(prefix, STEPS) {
    var ctl = $(prefix + '-ctl'), list = $(prefix + '-steps'), out = $(prefix + '-out'), pos = 0;
    if (!ctl || !list || !out) return;
    var prev = el('button', { type: 'button', 'class': 'btn ghost small' }, ctl, '◀ krok');
    var next = el('button', { type: 'button', 'class': 'btn ghost small' }, ctl, 'krok ▶');
    var where = el('span', { 'class': 'flow-pos' }, ctl);
    STEPS.forEach(function (s, i) {
      var li = el('li', { 'class': 'flow-step' }, list), b = el('button', { type: 'button', 'class': 'flow-step-b' }, li);
      el('span', { 'class': 'flow-step-n' }, b, String(i + 1)); el('b', null, b, s.t);
      el('span', { 'class': 'flow-step-c' }, li, s.c);
      el('span', { 'class': 'pill reports' }, el('span', { 'class': 'flow-step-l' }, li), s.m);
      b.addEventListener('click', function () { draw(i); });
    });
    function draw(k) {
      pos = Math.max(0, Math.min(STEPS.length - 1, k));
      Array.prototype.forEach.call(list.children, function (li, i) {
        li.classList.toggle('todo', i > pos);
        if (i === pos) li.setAttribute('aria-current', 'step'); else li.removeAttribute('aria-current');
      });
      var s = STEPS[pos];
      clear(out);
      s.cmd.forEach(function (c) { el('div', { 'class': 'ln' }, out, '$ ' + c); });
      s.out.forEach(function (o) { el('div', { 'class': 'ln mut' }, out, o); });
      var n = el('div', { 'class': 'ln ok' }, out, '# ' + s.note);
      if (s.link) { n.appendChild(document.createTextNode(' ')); el('a', { href: '#' + s.link[0] }, n, s.link[1]); }
      where.textContent = 'krok ' + (pos + 1) + ' z ' + STEPS.length;
      prev.disabled = pos === 0; next.disabled = pos === STEPS.length - 1;
    }
    prev.addEventListener('click', function () { draw(pos - 1); });
    next.addEventListener('click', function () { draw(pos + 1); });
    draw(0);
  }
  stepper('gr', GRAD_STEPS);
  stepper('sw', SWITCH_STEPS);

  if ($('lab-say')) (function () {
    var WHO = [
      { k: 'Kolega', say: 'Tasky, jejich scope a handovery si vedu v lokálním adresáři, který git ignoruje. V repozitáři z toho není nic.',
        proof: 'git status --short --ignored v mém klonu', links: [['lab-visible', 'Kdo co uvidí']],
        ask: 'Když se zeptá, jestli by to šlo i jemu: ano, a je to pět příkazů.' },
      { k: 'Vedoucí týmu', say: 'Repozitář, review a CI jsou přesně takové, jaké byly. Když se to osvědčí, navrhnu to jako pull request a přinesu k němu záznamy.',
        proof: 'prázdný diff a změřený převod na týmové nasazení', links: [['lab-grad', 'Od osobního k týmovému'], ['nasazeni', 'Nasazení krok za krokem']],
        ask: 'Zajímá ho hlavně, jestli mu to přidělá práci. Nepřidělá, dokud to sám nebude chtít.' },
      { k: 'Bezpečnost', say: 'Nástroj nedělá žádné síťové volání a nevolá žádný model. AI klient, který používám, je ten, který firma už povoluje, a platí pro něj stejná datová pravidla.',
        proof: 'garance no-network a pravidlo no-network-no-eval v repozitáři Majordomusu', links: [['m-bezpecnost', 'Bezpečnost'], ['h-hranice', 'Co harness nedělá']],
        docs: ['docs/claims/no-network.md', '.ai/repo/rules/project/no-network-no-eval.v1.md'],
        ask: 'Jediná poctivá výhrada: pravidla o tom, jaké nástroje smí číst tenhle kód, platí i pro lokální nástroj.' },
      { k: 'Architekt', say: 'Je to stejný životní cyklus, jaký běží v týmovém nasazení, jen se soukromou policy. Žádné pravidlo týmu se nepřepisuje ani neobchází.',
        proof: 'task lifecycle a finish contract jsou tytéž', links: [['h-finish', 'Finish contract'], ['h-scope', 'Dva druhy scope'], ['graf', 'Stavba z grafu repozitáře']],
        ask: 'Bude chtít vědět, co se stane při přechodu na týmové nasazení. Odpověď je změřená.' },
      { k: 'Management', say: 'BYOD pro pracovní kázeň: vlastní nástroj na vlastní pořádek, firemní repozitář beze změny. Nestojí to nic, neschvaluje se nic a jde to kdykoli smazat.',
        proof: 'tři režimy vedle sebe: osobní, týmový, s meshem', links: [['velka-org', 'Pro velkou organizaci'], ['problemy', 'Problémy a root causes'], ['h-mereni', 'Co harness stojí']],
        ask: 'Čísla k rozhodnutí o týmovém nasazení vzniknou z tvých vlastních záznamů, ne z prezentace.' }
    ];
    var seg = $('say-seg'), card = $('say-card');
    function show(i) {
      Array.prototype.forEach.call(seg.children, function (b, k) { b.setAttribute('aria-selected', String(k === i)); });
      var w = WHO[i]; clear(card);
      el('blockquote', { 'class': 'say-q' }, card, w.say);
      var p = el('p', null, card); el('strong', null, p, 'Kde si to ověří: '); p.appendChild(document.createTextNode(w.proof + '.'));
      var q = el('p', null, card); el('strong', null, q, 'Na co se připravit: '); q.appendChild(document.createTextNode(w.ask));
      var l = el('p', { 'class': 'as-links' }, card);
      w.links.forEach(function (x, k) { if (k) l.appendChild(document.createTextNode(' · ')); el('a', { href: '#' + x[0] }, l, '§ ' + x[1]); });
      (w.docs || []).forEach(function (d) {
        if (!(window.mjDocs && window.mjDocs.index.docs.some(function (x) { return x.p === d; }))) return;
        l.appendChild(document.createTextNode(' · '));
        el('a', { href: '#doc.' + d, 'class': 'md-in' }, l, d.split('/').pop());
      });
    }
    WHO.forEach(function (w, i) {
      el('button', { type: 'button', 'class': 'seg-b', role: 'tab', 'aria-selected': 'false' }, seg, w.k).addEventListener('click', function () { show(i); });
    });
    show(0);
  })();
})();

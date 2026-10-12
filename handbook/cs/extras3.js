(function () {
  'use strict';
  var GH = 'https://github.com/korczis/prismatic-majordomus';
  var NS = 'http://www.w3.org/2000/svg';
  function $(id) { return document.getElementById(id); }
  function el(tag, attrs, parent, text) {
    var e = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (text !== undefined) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  function svg(tag, attrs, parent) {
    var e = document.createElementNS(NS, tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (parent) parent.appendChild(e);
    return e;
  }
  function clear(e) { while (e.firstChild) e.removeChild(e.firstChild); }
  function line(t, text, cls) { return el('div', { 'class': 'ln' + (cls ? ' ' + cls : '') }, t, text); }
  function checks(host, defs, onChange) {
    return defs.map(function (d) {
      var lab = el('label', { 'class': 'chk' }, host);
      var cb = el('input', { type: 'checkbox', id: d[0] }, lab);
      cb.checked = !!d[2];
      lab.appendChild(document.createTextNode(' ' + d[1]));
      cb.addEventListener('change', onChange);
      return cb;
    });
  }

  /* ---------- issue status ---------- */
  if ($('lab-issue')) (function () {
    var out = $('is-out');
    var c = checks($('is-checks'), [
      ['is-cancel', 'cancelled: true', false], ['is-deps', 'všechny závislosti jsou DONE', true], ['is-start', 'started_at (s pečetí)', true],
      ['is-verify', 'verified_at: implementace prohlášena za hotovou', false], ['is-done', 'completed_at (s pečetí)', false], ['is-ev', 'každý token evidence_required má důkaz', false]
    ], render);
    function render() {
      clear(out);
      var cancel = c[0].checked, deps = c[1].checked, start = c[2].checked, verify = c[3].checked, done = c[4].checked, ev = c[5].checked;
      var st, why, cls = '';
      if (cancel) { st = 'CANCELLED'; why = 'cancelled: true má přednost před vším'; cls = 'mut'; }
      else if (done && ev) { st = 'DONE'; why = 'completed_at je nastavené a každý požadovaný token má důkaz'; cls = 'ok'; }
      else if (done || verify) { st = 'VERIFY'; why = 'implementace je prohlášená za hotovou, důkazy ještě nestačí'; cls = 'warn'; }
      else if (start) { st = 'ACTIVE'; why = 'started_at je nastavené a nic dalšího'; }
      else if (!deps) { st = 'BLOCKED'; why = 'některá závislost není DONE'; cls = 'bad'; }
      else { st = 'READY'; why = 'nic z předchozího neplatí'; cls = 'ok'; }
      line(out, '$ majordomus plan show I0103', 'mut');
      line(out, 'status: ' + st + '   (odvozeno, v záznamu není)', cls);
      line(out, why, 'mut');
      if (done && !ev) line(out, 'plan done by tu byl odmítnut: chybí důkaz k požadovanému tokenu', 'bad');
      if (start && !deps && !cancel) line(out, 'pozor: stav před svými blokery se hlásí jako nekonzistence', 'warn');
      if (st === 'READY') line(out, 'plan start I0103 projde; u jiných stavů je odmítnut', 'mut');
    }
    render();
  })();

  /* ---------- proof state ---------- */
  if ($('lab-proof')) (function () {
    var out = $('pf-out'), test = $('pf-test'), run = $('pf-run');
    var c = checks($('pf-checks'), [
      ['pf-contain', 'commit běhu je v posuzované historii', true], ['pf-digest', 'test má stejný hash jako při běhu', true],
      ['pf-input', 'změnil se některý vstup (zdroj garance, implementace, test)', false], ['pf-other', 'změnilo se něco jiného', false],
      ['pf-clean-r', 'strom při běhu byl čistý', true], ['pf-clean-p', 'posuzovaný strom je čistý', true]
    ], render);
    [test, run].forEach(function (s) { s.addEventListener('change', render); });
    function render() {
      clear(out);
      var st, why;
      if (test.value === 'none') { st = 'no_test'; why = 'garance nejmenuje test'; }
      else if (test.value === 'unrunnable') { st = 'unrunnable'; why = 'cestu neumí spustit žádný runner'; }
      else if (run.value === 'none') { st = 'not_run'; why = 'nic nezaznamenáno'; }
      else if (run.value === 'skip') { st = 'not_run'; why = 'skip není běh'; }
      else if (run.value === 'fail') { st = 'failing'; why = 'poslední běh neprošel'; }
      else if (!c[0].checked) { st = 'stale'; why = 'běh proběhl na commitu, který posuzovaná revize neobsahuje'; }
      else if (!c[1].checked) { st = 'stale'; why = 'test už nemá zaznamenaný hash'; }
      else if (c[2].checked) { st = 'stale'; why = 'změnil se vstup důkazu'; }
      else if (c[3].checked) { st = 'inputs_unchanged'; why = 'něco se změnilo, ale žádný ze vstupů'; }
      else if (!c[4].checked || !c[5].checked) { st = 'inputs_unchanged'; why = 'nic se nezměnilo, ale některý strom je špinavý nebo neznámý'; }
      else { st = 'proven'; why = 'pass, nic se nezměnilo, oba stromy čisté: jediná cesta k proven'; }
      var good = st === 'proven' || st === 'inputs_unchanged';
      line(out, '$ majordomus evidence claim <id>', 'mut');
      line(out, 'state: ' + st, st === 'proven' ? 'ok' : good ? '' : st === 'failing' ? 'bad' : 'warn');
      line(out, why, 'mut');
      line(out, good ? 'gate evidence-check: garance je podpořená' : 'gate evidence-check: garance NENÍ podpořená', good ? 'ok' : 'bad');
    }
    render();
  })();

  /* ---------- release bump ---------- */
  if ($('lab-bump')) (function () {
    var out = $('bp-out'), ver = $('bp-ver'), surf = $('bp-surf'), com = $('bp-commits');
    function render() {
      clear(out);
      var m = ver.value.trim().match(/^(\d+)\.(\d+)\.(\d+)$/);
      line(out, '$ majordomus release analyze', 'mut');
      if (!m) { line(out, 'verze musí být ve tvaru X.Y.Z', 'bad'); return; }
      var a = +m[1], b = +m[2], p = +m[3], pre = a === 0, bump = null, by;
      if (surf.value === 'unknown') { line(out, 'undecided: baseline nejde změřit', 'warn'); line(out, 'release bump bez --level odmítne · exit 12', 'bad'); return; }
      if (surf.value === 'narrow') { bump = pre ? 'minor' : 'major'; by = 'contract'; }
      else if (surf.value === 'wider') { bump = 'minor'; by = 'contract'; }
      else if (com.checked) { bump = 'patch'; by = 'contract_and_commits'; }
      if (!bump) { line(out, 'nic nedluží: povrch stejný a žádné změny', 'ok'); return; }
      var next = bump === 'major' ? (a + 1) + '.0.0' : bump === 'minor' ? a + '.' + (b + 1) + '.0' : a + '.' + b + '.' + (p + 1);
      line(out, 'contract implies: ' + (surf.value === 'same' ? 'nothing' : bump) + (pre && surf.value === 'narrow' ? '  (pod 1.0.0 je podlahou pro změnu povrchu minor)' : ''));
      line(out, 'commits imply: ' + (com.checked ? 'změny (jen podpůrný důkaz)' : 'nic'), 'mut');
      line(out, 'next: ' + next + '   decided_by: ' + by, 'ok');
      line(out, '--level a --exact smí jít jen výš než ' + next, 'mut');
    }
    [ver, surf, com].forEach(function (x) { x.addEventListener('input', render); x.addEventListener('change', render); });
    render();
  })();

  /* ---------- lessons timeline ---------- */
  var L = [
    { d: '', k: 'vynucení', t: 'Sedmnáct kontrol, které nikdo nevolal', w: 'Nejméně sedmnáct kontrol bylo zdokumentovaných jako blokující, leželo na disku, šlo spustit, a nic je nevolalo. V jednom případě bylo osiřelých 34 z 39 pre-commit skriptů.', c: 'Režim vynucení se odvozuje z kódu a doctor prochází řetěz každé doktríny.', ch: 'h-vynuceni', s: 'docs/DESIGN.md' },
    { d: '2026-09-10', k: 'CI', t: 'Master čekal sedm hodin na runner', w: 'Čtyři validační běhy masteru stály ve frontě, nejstarší sedm hodin, a čekaly na macOS runner. ci neodpovědělo a master přes noc nesl pět vad.', c: 'Gates na vzácných runnerech se plánují jen na vyžádání.', ch: 'h-ci', s: '.ai/repo/adrs/0037', adr: '0037' },
    { d: '2026-09-11', k: 'koordinace', t: 'Sedm serverů, sedm boardů', w: 'Sedm živých serverů nad jedním repozitářem mělo sedm oddělených boardů. Každý worker si myslel, že vidí celé repo.', c: 'majordomus_peers sbírá boardy všech checkoutů a hlásí, když některý nešel přečíst.', ch: 'h-server', s: '.ai/repo/adrs/0044', adr: '0044' },
    { d: '2026-09-11', k: 'disk', t: '64 worktrees, 111 branchí, 192 serverů', w: '16 otevřených PR, 64 worktrees (47 se změnami), 111 branchí a 192 serverů, z toho 14 opuštěných.', c: 'Pravidlo, že hromadění se měří, gate backlog-check, majordomus prs a reap-orphans.', ch: 'x-worktree', s: 'docs/BACKLOG.md' },
    { d: '2026-09-11', k: 'session', t: 'Šest dní bez checkpointu', w: 'Šest dní se nezapsal žádný checkpoint ani handover a doctor přitom hlásil zdraví.', c: 'Životní cyklus se váže na epizody, ne na tasky, a hlídá ho blokující pravidlo.', ch: 'p-session', s: '.ai/repo/adrs/0052', adr: '0052' },
    { d: '2026-09-12', k: 'disk', t: 'Nula bajtů volného místa', w: 'Disk o 926 GiB došel na nulu. Sloučené worktrees držely kolem 145 GB build výstupu a každá session spadla.', c: 'reap-orphans --reclaim a build se odmítne pod 5 120 MB volného místa.', ch: 'x-worktree', s: 'docs/BACKLOG.md', adr: '0055' },
    { d: '2026-09-12', k: 'vydání', t: 'Web 34 commitů pozadu', w: 'Rušení rozběhnutých deployů nechalo web tři hodiny 34 commitů za masterem.', c: 'Deploymenty se řadí za sebe a nikdy neruší.', ch: 'x-release', s: 'docs/CI.md' },
    { d: '2026-09-13', k: 'CI', t: 'Půl výstupu vypadalo jako report', w: 'Průchod umřel na SIGPIPE (exit 141) po hlavičce a jednom řádku a byl přečten jako hotový report.', c: 'Neúplný průchod tiskne INCOMPLETE.', ch: 'h-ci', s: 'docs/BACKLOG.md' },
    { d: '2026-09-14', k: 'koordinace', t: 'Commit spolkl cizí rozdělanou práci', w: 'Jeden commit pohltil napůl hotové přejmenování z jiné session. CI prošlo.', c: 'Necommitnutá práce peera je součást jeho claimu.', ch: 'koordinace', s: '.ai/repo/adrs/0063', adr: '0063' },
    { d: '2026-09-15', k: 'vydání', t: 'Verze, která nikdy nevyšla', w: 'Strom deklaroval 0.7.0, nejnovější vydání bylo v0.6.0 a dělilo je 55 commitů. Web přitom ukazoval verzi stromu.', c: 'Odznak renderuje skutečné vydání a site-check odmítne nabízenou verzi bez vydání.', ch: 'x-release', s: 'docs/RELEASE.md' },
    { d: '2026-09-15', k: 'vynucení', t: 'Příkaz jen v chybových hláškách', w: 'release bump se objevoval jen v chybových hláškách. Nic ho nespouštělo.', c: 'Workflow version.yml, který analyzuje a otevírá PR s bumpem.', ch: 'x-release', s: 'docs/RELEASE.md' },
    { d: '', k: 'vydání', t: 'Čtyři vydání z červených commitů', w: 'Verze v0.3.1, v0.5.0, v0.6.0 a v0.7.0 vyšly z commitů, jejichž ci selhalo.', c: 'release-verdict v plánu vydání.', ch: 'x-release', s: 'docs/DISTRIBUTION.md' },
    { d: '', k: 'vynucení', t: '109 selhání při prvním spuštění', w: 'První doctor v cizím repozitáři hlásil 109 selhání (112 ve v0.5.0). Dnes jedno.', c: 'Důkazy vendorovaného balíčku rozhoduje razítko vydání.', ch: 'nasazeni', s: 'docs/ADOPTION_FIRST_RUN.md' },
    { d: '2026-09-30', k: 'koordinace', t: 'Sedmdesát otevřených PR, žádné připravené', w: 'Sedmdesát pull requestů bylo otevřených a žádné nebylo připravené k merge.', c: 'prs drain: jedno PR po druhém místo hromadného přistávání.', ch: 'x-plan', s: '.ai/repo/adrs/0101', adr: '0101' },
    { d: '2026-10-09', k: 'bezpečnost', t: 'Servery poslouchaly na všech rozhraních', w: 'Tři servery poslouchaly na každém rozhraní a stroje v LAN mohly spouštět měnící capabilities.', c: 'Volající mimo loopback se musí autentizovat.', ch: 'm-bezpecnost', s: '.ai/repo/adrs/0126', adr: '0126' },
    { d: '2026-10-10', k: 'vydání', t: 'Vydání viditelné až po hodině', w: 'Záznamové PR jednoho vydání běželo v ci 106 minut. Další vydání vyšlo v 16:38 a web ho servíroval až v 17:42.', c: 'Vydání publikuje svoje metadata samo, s mezí 15 minut.', ch: 'x-release', s: '.ai/repo/adrs/0131', adr: '0131' },
    { d: '2026-09-08', k: 'disk', t: '104 GB build výstupu za 18 hodin', w: 'Osmnáct hodin po zavedení topologie drželo 15 worktrees 104 GB a na disku zbylo 124 MiB. Tři subsystémy hlásily špatný obsah, ne plný disk.', c: 'Sdílené mezivýsledky buildu se stropem a měření build výstupu.', ch: 'x-worktree', s: 'docs/WORKTREES.md' }
  ];
  if ($('lab-lessons')) (function () {
    var s = $('ls-svg'), panel = $('ls-panel'), list = $('ls-list'), chips = $('ls-chips');
    var kinds = [];
    L.forEach(function (x) { if (kinds.indexOf(x.k) === -1) kinds.push(x.k); });
    var filter = null, cur = -1;
    var t0 = Date.UTC(2026, 8, 3), t1 = Date.UTC(2026, 9, 12);
    function X(d) { var t = Date.UTC(+d.slice(0, 4), +d.slice(5, 7) - 1, +d.slice(8, 10)); return 110 + 590 * (t - t0) / (t1 - t0); }
    function cz(d) { return d ? (+d.slice(8, 10)) + '. ' + (+d.slice(5, 7)) + '. 2026' : 'nedatováno'; }
    function show(i) {
      cur = i; draw();
      var x = L[i];
      clear(panel);
      var h = el('p', { 'class': 'pn-h' }, panel);
      el('b', null, h, x.t);
      el('span', { 'class': 'pill reports' }, h, x.k);
      el('span', { 'class': 'pill reports' }, h, cz(x.d));
      el('p', null, panel, x.w);
      var ch = el('p', null, panel);
      el('strong', null, ch, 'Co se změnilo: ');
      ch.appendChild(document.createTextNode(x.c));
      var lk = el('p', { 'class': 'as-links' }, panel);
      var a = el('a', { href: '#' + x.ch }, lk, 'Kapitola');
      a.addEventListener('click', function (e) { e.preventDefault(); if (window.mjJump) window.mjJump(x.ch); else { var t = $(x.ch); if (t) t.scrollIntoView(); } });
      lk.appendChild(document.createTextNode(' · zdroj: '));
      var src = x.adr ? document.querySelector('a.src[href*="/adrs/' + x.adr + '-"]') : null;
      var href = x.adr && adrHref[x.adr] ? adrHref[x.adr] : (x.s.indexOf('docs/') === 0 ? GH + '/blob/master/' + x.s : null);
      if (href) { var g = el('a', { href: href, 'class': 'src' }, lk); el('code', null, g, x.adr ? 'ADR ' + x.adr : x.s); }
      else el('code', null, lk, x.s);
    }
    var adrHref = {};
    var ex = null;
    try { ex = JSON.parse($('data-explore').textContent); } catch (e) { ex = null; }
    if (ex) ex.adrs.forEach(function (a) { adrHref[a.n] = GH + '/blob/master/' + a.p; });
    function draw() {
      clear(s);
      svg('line', { x1: 110, x2: 700, y1: 70, y2: 70, 'class': 'axis' }, s);
      [['2026-09-05', '5. 9.'], ['2026-09-15', '15. 9.'], ['2026-09-25', '25. 9.'], ['2026-10-05', '5. 10.']].forEach(function (t) {
        var tx = svg('text', { x: X(t[0]), y: 100, 'text-anchor': 'middle', 'class': 'ax' }, s); tx.textContent = t[1];
        svg('line', { x1: X(t[0]), x2: X(t[0]), y1: 66, y2: 74, 'class': 'axis' }, s);
      });
      var und = svg('text', { x: 16, y: 44, 'class': 'ax' }, s); und.textContent = 'nedatováno';
      var stack = {}, uy = 0;
      L.forEach(function (x, i) {
        var hidden = filter && x.k !== filter;
        var cx, cy;
        if (x.d) { var n = stack[x.d] = (stack[x.d] || 0) + 1; cx = X(x.d); cy = 70 - (n - 1) * 20 - (n > 1 ? 0 : 0); if (n > 1) cy = 70 - (n - 1) * 20; }
        else { cx = 30 + uy * 24; cy = 70; uy++; }
        var g = svg('g', { 'class': 'mark', tabindex: hidden ? '-1' : '0', role: 'button', 'aria-label': x.t + ', ' + cz(x.d) }, s);
        svg('circle', { cx: cx, cy: cy, r: 13, fill: 'transparent' }, g);
        svg('circle', { cx: cx, cy: cy, r: i === cur ? 9 : 6.5, 'class': (i === cur ? 'st-gate' : 'st-agent') + ' ring', opacity: hidden ? 0.15 : 1 }, g);
        if (!hidden) {
          g.addEventListener('click', function () { show(i); });
          g.addEventListener('keydown', function (e) { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); show(i); } });
        }
      });
    }
    function renderList() {
      clear(list);
      L.map(function (x, i) { return [x, i]; }).filter(function (p) { return !filter || p[0].k === filter; })
        .sort(function (a, b) { return (a[0].d || '0').localeCompare(b[0].d || '0'); })
        .forEach(function (p) {
          var li = el('li', null, list);
          var b = el('button', { type: 'button', 'class': 'ls-b' }, li);
          el('span', { 'class': 'ls-d' }, b, p[0].d ? cz(p[0].d).replace(' 2026', '') : '—');
          el('span', null, b, p[0].t);
          b.addEventListener('click', function () { show(p[1]); });
        });
    }
    function renderChips() {
      clear(chips);
      var all = el('button', { type: 'button', 'class': 'chip' + (filter ? '' : ' on') }, chips, 'vše · ' + L.length);
      all.addEventListener('click', function () { filter = null; renderChips(); draw(); renderList(); });
      kinds.forEach(function (k) {
        var n = L.filter(function (x) { return x.k === k; }).length;
        var b = el('button', { type: 'button', 'class': 'chip' + (filter === k ? ' on' : '') }, chips, k + ' · ' + n);
        b.addEventListener('click', function () { filter = filter === k ? null : k; renderChips(); draw(); renderList(); });
      });
    }
    renderChips(); renderList(); show(1);
  })();
})();

(function () {
  'use strict';
  var NS = 'http://www.w3.org/2000/svg';
  function $(id) { return document.getElementById(id); }
  function el(tag, attrs, parent, text) {
    var e = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (text !== undefined) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  function svg(tag, attrs, parent, text) {
    var e = document.createElementNS(NS, tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (text !== undefined) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  function clear(e) { while (e.firstChild) e.removeChild(e.firstChild); }
  function line(t, text, cls) { return el('div', { 'class': 'ln' + (cls ? ' ' + cls : '') }, t, text); }
  function seg(host, items, onPick, initial) {
    var btns = items.map(function (it, i) {
      var b = el('button', { type: 'button', 'class': 'seg-b', 'aria-pressed': 'false' }, host, it);
      b.addEventListener('click', function () { pick(i); });
      return b;
    });
    function pick(i) { btns.forEach(function (b, k) { b.setAttribute('aria-pressed', k === i ? 'true' : 'false'); b.setAttribute('aria-selected', k === i ? 'true' : 'false'); }); onPick(i); }
    pick(initial || 0);
  }

  /* ---------- who sees what ---------- */
  if ($('lab-visible')) (function () {
    var s = $('vis-svg'), note = $('vis-note');
    var ART = [
      ['airepo', '.ai/repo/ · pravidla, ADR, plán'], ['ailocal', '.ai/local/ · task, handovery'], ['claude', 'CLAUDE.local.md'],
      ['excl', '.git/info/exclude'], ['hook', '.git/hooks/pre-push'], ['commits', 'tvoje commity s kódem'],
      ['names', 'jména branchí, commit zprávy'], ['server', 'server na 127.0.0.1'], ['banner', 'banner v terminálu']
    ];
    var ACT = [
      ['Ty', ['airepo', 'ailocal', 'claude', 'excl', 'hook', 'commits', 'names', 'server', 'banner'], 'Vidíš všechno. Je to tvůj disk a tvůj klon.'],
      ['Kolega s vlastním klonem', ['commits', 'names'], 'Dostane jen to, co pushneš: commity a jejich jména. Vrstva, soubor s instrukcemi, vyloučení ani hook v jeho klonu neexistují.'],
      ['CI', ['commits', 'names'], 'CI checkoutuje commit. V něm z Majordomusu nic není, takže běží přesně to, co běželo předtím.'],
      ['Vzdálený repozitář', ['commits', 'names'], 'Git posílá objekty a refy. .git/info/exclude ani .git/hooks se nikdy nepushují.'],
      ['AI klient na tvém stroji', ['airepo', 'claude', 'commits', 'server'], 'Čte instrukce z CLAUDE.local.md a vrstvu přes server. Stav v .ai/local/ mu Majordomus neservíruje, ale klient s přístupem k disku si soubory přečíst může. Co klient odesílá ven, řídí jeho vlastní pravidla.'],
      ['Kdo vidí tvou obrazovku', ['banner', 'server'], 'Banner při vstupu do adresáře a otevřené /ui v prohlížeči. Banner vypíná MAJORDOMUS_BANNER=off.']
    ];
    var ay = function (i) { return 34 + i * 48; }, ry = function (i) { return 22 + i * 33; };
    var edges = [], artEls = {}, actEls = [];
    ART.forEach(function (a, i) {
      var g = svg('g', { 'class': 'vis-n' }, s);
      svg('rect', { x: 440, y: ry(i) - 13, width: 264, height: 26, rx: 5, 'class': 'box', 'stroke-width': 1.25 }, g);
      svg('text', { x: 452, y: ry(i) + 4, 'class': 'ax mono vis-t' }, g, a[1]);
      artEls[a[0]] = { g: g, y: ry(i) };
    });
    ACT.forEach(function (a, i) {
      var g = svg('g', { 'class': 'vis-n vis-a', tabindex: '0', role: 'button', 'aria-label': a[0] }, s);
      svg('rect', { x: 16, y: ay(i) - 17, width: 214, height: 34, rx: 6, 'class': 'box', 'stroke-width': 1.25 }, g);
      svg('text', { x: 28, y: ay(i) + 5, 'class': 'ax strong vis-t' }, g, a[0]);
      a[1].forEach(function (k) {
        var y2 = artEls[k].y;
        edges.push({ a: i, k: k, el: svg('path', { d: 'M230 ' + ay(i) + 'C330 ' + ay(i) + ' 340 ' + y2 + ' 440 ' + y2, 'class': 'vis-e' }, s) });
      });
      actEls.push(g);
      g.addEventListener('click', function () { pick(i); });
      g.addEventListener('keydown', function (e) { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pick(i); } });
    });
    var segHost = $('vis-seg'), segBtns = [];
    ACT.forEach(function (a, i) {
      var b = el('button', { type: 'button', 'class': 'seg-b', 'aria-pressed': 'false' }, segHost, a[0]);
      b.addEventListener('click', function () { pick(i); });
      segBtns.push(b);
    });
    function pick(i) {
      var seen = {};
      ACT[i][1].forEach(function (k) { seen[k] = true; });
      segBtns.forEach(function (b, k) { b.setAttribute('aria-pressed', k === i ? 'true' : 'false'); b.setAttribute('aria-selected', k === i ? 'true' : 'false'); });
      actEls.forEach(function (g, k) { g.setAttribute('class', 'vis-n vis-a' + (k === i ? ' on' : ' dim')); });
      ART.forEach(function (a) { artEls[a[0]].g.setAttribute('class', 'vis-n' + (seen[a[0]] ? ' on' : ' dim')); });
      edges.forEach(function (e) { e.el.setAttribute('class', 'vis-e' + (e.a === i ? ' on' : ' off')); });
      clear(note);
      el('b', null, note, ACT[i][0] + ': ' + ACT[i][1].length + ' z ' + ART.length + '. ');
      note.appendChild(document.createTextNode(ACT[i][2]));
    }
    pick(1);
  })();

  /* ---------- command flow ---------- */
  if ($('lab-stflow')) (function () {
    var s = $('sf-svg'), note = $('sf-note');
    var N = {
      you: [16, 100, 96, 50, 'Ty'], mj: [146, 100, 130, 50, 'majordomus'],
      layer: [316, 22, 160, 50, '.ai/ · vyloučeno'], index: [316, 100, 160, 50, 'index gitu'], hook: [316, 178, 160, 50, '.git/hooks/pre-push'],
      server: [516, 22, 188, 50, 'server · /ui'], disk: [516, 100, 188, 50, 'pracovní strom'], remote: [516, 178, 188, 50, 'vzdálený repozitář']
    };
    var els = {};
    var lines = svg('g', null, s);
    Object.keys(N).forEach(function (k) {
      var n = N[k], g = svg('g', { 'class': 'sf-n' }, s);
      svg('rect', { x: n[0], y: n[1], width: n[2], height: n[3], rx: 6, 'class': 'box', 'stroke-width': 1.25 }, g);
      svg('text', { x: n[0] + n[2] / 2, y: n[1] + 30, 'text-anchor': 'middle', 'class': 'ax strong' + (k === 'layer' || k === 'hook' ? ' mono' : '') }, g, n[4]);
      els[k] = g;
    });
    var layer = svg('g', null, s);
    var FL = [
      ['start · check · finish', ['you', 'mj', 'layer'], null, 'Majordomus čte a zapisuje jen do .ai/. Index gitu se nezmění, takže git status zůstane čistý.'],
      ['git commit', ['you', 'index', 'disk'], null, 'Git staguje jen to, co vidí. Vyloučenou vrstvu nenabídne ani git add -A, takže commit nese jen kód.'],
      ['git push', ['you', 'hook', 'mj', 'layer', 'remote'], null, 'Lokální hook spustí finish --check, ten čte vrstvu. Na remote odejdou jen commity. Hook ani exclude se nepushují.'],
      ['serve --discovery filesystem', ['you', 'mj', 'disk', 'layer', 'server'], null, 'Zjišťování projde pracovní strom, vrstvu najde a /ui i Cockpit odpovídají. Změřeno: stav ok.'],
      ['serve, výchozí režim', ['you', 'mj', 'index'], 'index', 'Výchozí zjišťování se ptá indexu gitu. Ten vyloučenou vrstvu nezná, povinné třídy nenajdou nic a index vyjde degraded.'],
      ['git clean -fdx', ['you', 'disk', 'layer'], 'layer', 'Přepínač -x maže i vyloučené soubory. Vrstva zmizí i s rozhodnutími a handovery. Nejdřív git clean -ndx a záloha.']
    ];
    function center(k) { var n = N[k]; return [n[0] + n[2] / 2, n[1] + n[3] / 2]; }
    function show(i) {
      var f = FL[i];
      clear(layer); clear(lines);
      Object.keys(els).forEach(function (k) { els[k].setAttribute('class', 'sf-n dim'); });
      f[1].forEach(function (k, n) {
        els[k].setAttribute('class', 'sf-n on' + (f[2] === k ? ' fail' : ''));
        var c = center(k);
        if (n > 0) { var p = center(f[1][n - 1]); svg('path', { d: 'M' + p[0] + ' ' + p[1] + 'L' + c[0] + ' ' + c[1], 'class': 'flow-line' }, lines); }
      });
      f[1].forEach(function (k, n) {
        var b = N[k];
        svg('circle', { cx: b[0] + 4, cy: b[1] + 4, r: 9, 'class': 'flow-badge' + (f[2] === k ? ' fail' : '') }, layer);
        svg('text', { x: b[0] + 4, y: b[1] + 8, 'text-anchor': 'middle', 'class': 'flow-num' }, layer, f[2] === k ? '✕' : String(n + 1));
      });
      clear(note);
      el('b', null, note, f[0] + (f[2] ? ' končí špatně. ' : '. '));
      note.appendChild(document.createTextNode(f[3]));
    }
    seg($('sf-seg'), FL.map(function (f) { return f[0]; }), show, 2);
  })();

  /* ---------- measured numbers ---------- */
  function bars(host, title, sub, rows, unit) {
    if (!host) return;
    el('p', { 'class': 'chart-title' }, host, title);
    el('p', { 'class': 'chart-sub' }, host, sub);
    var max = Math.max.apply(null, rows.map(function (r) { return r[1]; }));
    var box = el('div', { 'class': 'st-bars', role: 'img', 'aria-label': title + ': ' + rows.map(function (r) { return r[0] + ' ' + r[1]; }).join(', ') }, host);
    rows.forEach(function (r) {
      var row = el('div', { 'class': 'st-row' }, box);
      el('span', { 'class': 'st-l' }, row, r[0]);
      var track = el('span', { 'class': 'st-t' }, row);
      var b = el('i', { 'class': r[2] || '' }, track);
      b.style.width = Math.max(1.5, 100 * r[1] / max) + '%';
      el('b', { 'class': 'st-v' }, row, r[1] + (unit ? ' ' + unit : ''));
    });
  }
  bars($('st-doctor'), 'Selhání doctora v osobním režimu', 'tři běhy ve zkušebním repu, 12. 10. 2026', [
    ['čerstvý init, nic neupraveno', 6],
    ['prázdné enforcement, žádná always_loaded projekce', 4],
    ['jedna projekce, jedno vynucení, hook pre-push', 1, 'good']
  ], '');
  bars($('st-objects'), 'Co server z vyloučené vrstvy vidí', 'počet objektů v indexu podle režimu zjišťování', [
    ['--discovery vcs (výchozí), stav degraded', 1],
    ['--discovery filesystem, stav ok', 101, 'good'],
    ['z toho pravidla', 59], ['kontextové dokumenty', 19], ['taxonomie', 8], ['dokumenty', 5], ['profily', 4], ['prompty', 4], ['policy a scope', 2]
  ], '');

  /* ---------- which mechanism ---------- */
  if ($('lab-sttech')) (function () {
    var host = $('tq-checks'), out = $('tq-out');
    var Q = [
      ['Nový soubor nebo adresář, jen v tomhle repu', ['.git/info/exclude', "printf '.ai/\\n' >> .git/info/exclude", 'git check-ignore -v .ai/manifest.yaml', 'To je osobní režim. Nikdo jiný vzor nevidí, platí pro všechny worktrees klonu.'], 'ok'],
      ['Soubor, který tým trackuje, chci mít lokálně jinak', ['git update-index --skip-worktree', 'git update-index --skip-worktree <soubor>', 'git ls-files -v | grep ^S        # které soubory mají příznak', 'Git přestane hlásit tvoje lokální změny v tom souboru. Když ho někdo změní na remote, pull skončí konfliktem a příznak musíš sundat: --no-skip-worktree.'], 'warn'],
      ['Stejný druh souboru ve všech mých repozitářích', ['core.excludesFile', 'git config --global core.excludesFile ~/.gitignore_global', "printf '.DS_Store\\n*.swp\\n' >> ~/.gitignore_global", 'Dobré pro soubory editoru a systému. Řádek .ai/ sem nedávej: vyloučil by vrstvu i v repu, kde ji tým commituje.'], 'warn'],
      ['Něco, co nemá trackovat nikdo z týmu', ['.gitignore', "printf 'dist/\\n' >> .gitignore && git add .gitignore", '', 'Commituje se a vidí ho všichni. Správně pro build výstup, špatně pro cokoli, co má zůstat tvoje.'], 'bad']
    ];
    Q.forEach(function (q, i) {
      var lab = el('label', { 'class': 'chk' }, host);
      var r = el('input', { type: 'radio', name: 'tq', id: 'tq-' + i }, lab);
      if (i === 0) r.checked = true;
      lab.appendChild(document.createTextNode(' ' + q[0]));
      r.addEventListener('change', function () { show(i); });
    });
    function show(i) {
      var a = Q[i][1];
      clear(out);
      line(out, 'použij: ' + a[0], Q[i][2] === 'ok' ? 'ok' : Q[i][2] === 'bad' ? 'bad' : 'warn');
      line(out, a[1]);
      if (a[2]) line(out, a[2]);
      line(out, a[3], 'mut');
    }
    show(0);
  })();
})();

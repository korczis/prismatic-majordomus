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
  function jump(id) { if (window.mjJump) window.mjJump(id); }
  function chapterLink(id, label, parent) {
    var a = el('a', { href: '#' + id }, parent, label);
    a.addEventListener('click', function (e) { e.preventDefault(); jump(id); });
    return a;
  }
  var reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;

  /* A flow player over an existing SVG: numbered badges on a sequence of nodes, a connecting line, the rest dimmed.
     Every step is documented from the diagram itself: the node's own title and caption, and the chapter it links to. */
  function flowPlayer(opts) {
    var s = opts.svg, host = opts.host;
    var bar = el('div', { 'class': 'flow-bar' });
    el('span', { 'class': 'lbl', style: 'display:inline;margin:0 4px 0 0' }, bar, opts.title || 'Zvýraznit tok');
    var note = el('p', { 'class': 'flow-note', 'aria-live': 'polite' });
    var panel = el('div', { 'class': 'flow-panel', hidden: '' });
    var ctl = el('div', { 'class': 'flow-ctl' }, panel);
    var steps = el('ol', { 'class': 'flow-steps' }, panel);
    host.parentNode.insertBefore(bar, host);
    host.parentNode.insertBefore(note, host.nextSibling);
    host.parentNode.insertBefore(panel, note.nextSibling);
    var layer = svg('g', { 'class': 'flow-layer' }, s);
    var timer = null, active = null, seq = [], pos = 0;
    var bPrev = el('button', { type: 'button', 'class': 'btn ghost small' }, ctl, '◀ krok');
    var bPlay = el('button', { type: 'button', 'class': 'btn ghost small' }, ctl, 'Přehrát');
    var bNext = el('button', { type: 'button', 'class': 'btn ghost small' }, ctl, 'krok ▶');
    var where = el('span', { 'class': 'flow-pos', 'aria-live': 'polite' }, ctl);
    function texts(n) {
      var g = n.el.querySelectorAll ? n.el.querySelectorAll('text') : [];
      return Array.prototype.map.call(g, function (t) { return t.textContent.trim(); }).filter(Boolean);
    }
    function label(n) { return n.label || texts(n)[0] || n.key; }
    function caption(n) { return n.desc || texts(n).slice(1).join(' '); }
    function target(n) {
      var a = n.el.closest ? n.el.closest('a') : null;
      var h = a && (a.getAttribute('href') || a.getAttribute('xlink:href'));
      return h && h.charAt(0) === '#' ? h.slice(1) : null;
    }
    function chapterName(id) {
      var h = document.querySelector('#' + (window.CSS && CSS.escape ? CSS.escape(id) : id) + ' > h2');
      return h && h.firstChild ? h.firstChild.textContent.trim() : null;
    }
    function halt() { if (timer) { clearInterval(timer); timer = null; } bPlay.textContent = 'Přehrát'; }
    function stop() {
      halt();
      clear(layer); clear(steps);
      s.classList.remove('flowing');
      opts.nodes.forEach(function (n) { n.el.classList.remove('flow-on', 'flow-fail', 'flow-cur'); });
      Array.prototype.forEach.call(bar.querySelectorAll('button'), function (b) { b.setAttribute('aria-pressed', 'false'); });
      clear(note); panel.hidden = true; active = null; seq = [];
    }
    function center(n) { var b = n.box.getBBox(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, b: b }; }
    function draw(k) {
      pos = Math.max(0, Math.min(seq.length - 1, k));
      clear(layer);
      opts.nodes.forEach(function (n) { n.el.classList.remove('flow-on', 'flow-fail', 'flow-cur'); });
      for (var i = 0; i <= pos; i++) {
        var n = seq[i], c = center(n), failed = active.fail === active.seq[i];
        n.el.classList.add('flow-on');
        if (i === pos) n.el.classList.add('flow-cur');
        if (failed) n.el.classList.add('flow-fail');
        if (i > 0) {
          var p = center(seq[i - 1]);
          svg('path', { d: 'M' + p.x + ' ' + p.y + 'L' + c.x + ' ' + c.y, 'class': 'flow-line' + (i === pos ? ' cur' : '') }, layer);
        }
        var bx = opts.badge === 'center' ? c.x + 12 : c.b.x + 2, by = opts.badge === 'center' ? c.y - 12 : c.b.y + 2;
        svg('circle', { cx: bx, cy: by, r: i === pos ? 11 : 9, 'class': failed ? 'flow-badge fail' : 'flow-badge' }, layer);
        var t = svg('text', { x: bx, y: by + 4, 'text-anchor': 'middle', 'class': 'flow-num' }, layer);
        t.textContent = failed ? '✕' : String(i + 1);
      }
      Array.prototype.forEach.call(steps.children, function (li, i) {
        li.classList.toggle('done', i < pos);
        li.classList.toggle('todo', i > pos);
        if (i === pos) li.setAttribute('aria-current', 'step'); else li.removeAttribute('aria-current');
      });
      where.textContent = 'krok ' + (pos + 1) + ' z ' + seq.length;
      bPrev.disabled = pos === 0; bNext.disabled = pos === seq.length - 1;
    }
    function run() {
      if (timer) { halt(); return; }
      if (pos >= seq.length - 1) draw(0);
      bPlay.textContent = 'Zastavit';
      timer = setInterval(function () { if (pos >= seq.length - 1) { halt(); return; } draw(pos + 1); }, 900);
    }
    function play(flow, btn) {
      if (active === flow) { stop(); return; }
      stop(); active = flow;
      btn.setAttribute('aria-pressed', 'true');
      s.classList.add('flowing');
      seq = flow.seq.map(function (k) { return opts.byKey[k]; }).filter(Boolean);
      panel.hidden = false;
      seq.forEach(function (n, i) {
        var li = el('li', { 'class': 'flow-step' + (flow.fail === flow.seq[i] ? ' fail' : '') }, steps);
        var b = el('button', { type: 'button', 'class': 'flow-step-b' }, li);
        el('span', { 'class': 'flow-step-n' }, b, flow.fail === flow.seq[i] ? '✕' : String(i + 1));
        el('b', null, b, label(n));
        b.addEventListener('click', function () { halt(); draw(i); });
        var cap = caption(n);
        if (cap) el('span', { 'class': 'flow-step-c' }, li, cap);
        var id = target(n) || (opts.chapters && opts.chapters[n.key]);
        var name = id && chapterName(id);
        if (name) { var w = el('span', { 'class': 'flow-step-l' }, li); chapterLink(id, '§ ' + name, w); }
      });
      el('b', null, note, flow.label + '. ');
      note.appendChild(document.createTextNode(flow.text + ' '));
      (flow.links || []).forEach(function (l, k) { if (k) note.appendChild(document.createTextNode(' · ')); chapterLink(l[0], l[1], note); });
      if (reduce) draw(seq.length - 1); else { draw(0); run(); }
    }
    bPrev.addEventListener('click', function () { halt(); draw(pos - 1); });
    bNext.addEventListener('click', function () { halt(); draw(pos + 1); });
    bPlay.addEventListener('click', run);
    opts.flows.forEach(function (f) {
      var b = el('button', { type: 'button', 'class': 'seg-b', 'aria-pressed': 'false' }, bar, f.label);
      b.addEventListener('click', function () { play(f, b); });
    });
    var x = el('button', { type: 'button', 'class': 'btn ghost small', style: 'margin:0' }, bar, 'Zrušit');
    x.addEventListener('click', stop);
    return { stop: stop };
  }

  /* ---------- architecture diagram ---------- */
  (function () {
    var sec = $('stavba');
    if (!sec) return;
    var s = sec.querySelector('.fig svg');
    if (!s) return;
    var names = ['provider', 'hooky', 'server', 'task', 'context', 'registry', 'doctor', 'repo', 'local', 'policy', 'rules', 'adr', 'knowledge', 'sessions', 'skills', 'gates', 'meshdecl', 'taskstate', 'handovers', 'ledger', 'prompts', 'git', 'ci', 'mesh'];
    var nodes = [], byKey = {};
    Array.prototype.forEach.call(s.querySelectorAll('a'), function (a, i) {
      var g = a.querySelector('.hit'), box = g && g.querySelector('rect');
      if (!g || !box || !names[i]) return;
      var n = { key: names[i], el: g, box: box };
      nodes.push(n); byKey[names[i]] = n;
    });
    flowPlayer({
      svg: s, host: sec.querySelector('.fig'), nodes: nodes, byKey: byKey, title: 'Zvýraznit tok',
      flows: [
        { label: 'Session začíná', seq: ['provider', 'hooky', 'prompts', 'context', 'rules', 'adr', 'knowledge'],
          text: 'Provider vystřelí hook, ten otevře epizodu a archivuje prompt. Briefing pak složí kontext z pravidel, rozhodnutí a znalostí a provider ho vloží agentovi.',
          links: [['h-hooky', 'Hooky providera'], ['p-briefing', 'Briefing']] },
        { label: 'Změna jde do masteru', seq: ['provider', 'task', 'taskstate', 'git', 'gates', 'ci'],
          text: 'Agent založí task se scope, pracuje, commit a push projdou git hooky a CI vybere gates podle změněných cest. Finish zapíše hotovo až po zelených gates.',
          links: [['h-finish', 'Finish contract'], ['h-git', 'Git hooky'], ['h-ci', 'CI gates']] },
        { label: 'Co se session naučila', seq: ['hooky', 'ledger', 'knowledge', 'sessions', 'git'],
          text: 'Na konci session hook odvodí z ledgeru kandidáty znalostí a zavře epizodu do obálky. Obojí je trackované a k dalšímu workerovi dojde gitem.',
          links: [['p-znalosti', 'Knowledge base'], ['p-session', 'Session']] },
        { label: 'Dva agenti, dva stroje', seq: ['provider', 'server', 'mesh', 'meshdecl', 'handovers'],
          text: 'Agent se přes MCP ohlásí na boardu sdíleného serveru. Mesh podle deklarace propojí runtimes na jiných strojích, hlídá exkluzivní claims a přenese handover.',
          links: [['h-server', 'Sdílený server'], ['m-claims', 'Claims'], ['m-handovery', 'Handovery']] },
        { label: 'Je to zapojené?', seq: ['doctor', 'policy', 'rules', 'gates', 'registry'],
          text: 'Doctor projde policy, razítka instrukcí, řetěz každého pravidla až po gate v CI a shodu registru s jeho projekcemi.',
          links: [['h-doctor', 'Doctor a preflight'], ['h-vynuceni', 'Vynucení'], ['x-registry', 'Registr']] }
      ]
    });
  })();

  /* ---------- control-point lanes ---------- */
  (function () {
    var sec = $('h-mapa');
    if (!sec) return;
    var s = sec.querySelector('.fig svg');
    if (!s) return;
    var LANE = ['vystřeluje provider', 'volá agent', 'vystřeluje git', 'runner na pull requestu'];
    var keys = ['SessionStart', 'UserPromptSubmit', 'PreCompact', 'SessionEnd', 'announce', 'start', 'check', 'finish', 'pre-commit', 'commit-msg', 'pre-push', 'gates'];
    var circles = Array.prototype.filter.call(s.querySelectorAll('circle'), function (c) { return c.getAttribute('r') === '8'; });
    var nodes = [], byKey = {};
    circles.slice(0, 12).forEach(function (c, i) {
      var n = { key: keys[i], el: c, box: c, label: keys[i], desc: LANE[i < 4 ? 0 : i < 8 ? 1 : i < 11 ? 2 : 3] };
      nodes.push(n); byKey[keys[i]] = n;
    });
    flowPlayer({
      svg: s, host: sec.querySelector('.fig'), nodes: nodes, byKey: byKey, badge: 'center', title: 'Přehrát scénář',
      chapters: { SessionStart: 'h-hooky', UserPromptSubmit: 'h-hooky', PreCompact: 'h-hooky', SessionEnd: 'h-hooky', announce: 'h-server',
                  start: 'h-scope', check: 'h-zavazky', finish: 'h-finish', 'pre-commit': 'h-git', 'commit-msg': 'h-git', 'pre-push': 'h-git', gates: 'h-ci' },
      flows: [
        { label: 'Úspěšná změna', seq: ['SessionStart', 'UserPromptSubmit', 'announce', 'start', 'check', 'pre-commit', 'commit-msg', 'pre-push', 'gates', 'finish', 'SessionEnd'],
          text: 'Jedenáct bodů v pořadí, ve kterém je změna potká. Finish přichází až po gates, protože kontrakt vyžaduje gates_passed.',
          links: [['h-prubeh', 'Session krok za krokem']] },
        { label: 'Soubor mimo scope', seq: ['SessionStart', 'start', 'check'], fail: 'check',
          text: 'Agent sáhl mimo cesty deklarované při start. Check skončí s exit 10 a totéž by odmítl finish i pre-push.',
          links: [['h-scope', 'Dva druhy scope']] },
        { label: 'Fix bez testu', seq: ['start', 'check', 'pre-commit', 'commit-msg'], fail: 'commit-msg',
          text: 'Commit typu fix bez testu mezi staged soubory dostane nález commit.fix_without_test. Podle policy je to varování, nebo chyba s exit 10.',
          links: [['h-git', 'Git hooky']] },
        { label: 'Compaction uprostřed', seq: ['SessionStart', 'start', 'PreCompact', 'check', 'finish', 'SessionEnd'],
          text: 'Provider zkrátí konverzaci. Hook těsně předtím zapíše checkpoint a odvodí znalosti, takže práce pokračuje se stejným stavem.',
          links: [['h-hooky', 'Hooky providera'], ['p-zaznamy', 'Záznamy']] },
        { label: 'Gate neodpověděl', seq: ['start', 'check', 'pre-commit', 'commit-msg', 'pre-push', 'gates', 'finish'], fail: 'finish',
          text: 'CI gate je queued. To není pass, takže řádek gates_passed selže a finish nezapíše nic.',
          links: [['h-ci', 'CI gates'], ['h-finish', 'Finish contract']] }
      ]
    });
  })();

  /* ---------- coordination rings scenarios ---------- */
  (function () {
    var sec = $('koordinace');
    if (!sec) return;
    var s = sec.querySelector('.fig svg');
    if (!s) return;
    var rings = Array.prototype.map.call(s.querySelectorAll('a .hit'), function (g) { return g; });
    if (rings.length < 4) return;
    var SC = [
      { label: 'Agent sáhne mimo svůj task', ring: 3, text: 'Zachytí to task scope: check, finish i pre-push změnu odmítnou.', link: ['h-scope', 'Scope'] },
      { label: 'Dva worktrees, stejné cesty', ring: 2, text: 'start projde git worktree list a překryv ohlásí. Neodmítne, rozhodnutí nechá na workerovi.', link: ['x-worktree', 'Worktrees'] },
      { label: 'Dvě sessions v různých checkoutech', ring: 1, text: 'Board sdíleného serveru vrátí peery, jejichž ohlášený scope se potká. Je to claim, ne zámek.', link: ['h-server', 'Board'] },
      { label: 'Dva stroje, stejné cesty', ring: 0, text: 'Mesh odmítne druhý exkluzivní claim: claim_conflict, exit 10. Když se stroje neviděly, vítěze určí fold.', link: ['m-claims', 'Claims'] }
    ];
    var bar = el('div', { 'class': 'flow-bar' });
    el('span', { 'class': 'lbl', style: 'display:inline;margin:0 4px 0 0' }, bar, 'Kdo to zachytí');
    var note = el('p', { 'class': 'flow-note', 'aria-live': 'polite' });
    var fig = sec.querySelector('.fig');
    fig.parentNode.insertBefore(bar, fig);
    fig.parentNode.insertBefore(note, fig.nextSibling);
    SC.forEach(function (sc) {
      var b = el('button', { type: 'button', 'class': 'seg-b', 'aria-pressed': 'false' }, bar, sc.label);
      b.addEventListener('click', function () {
        var was = b.getAttribute('aria-pressed') === 'true';
        Array.prototype.forEach.call(bar.querySelectorAll('button'), function (x) { x.setAttribute('aria-pressed', 'false'); });
        rings.forEach(function (r) { r.classList.remove('flow-on'); });
        s.classList.remove('flowing'); clear(note);
        if (was) return;
        b.setAttribute('aria-pressed', 'true');
        s.classList.add('flowing');
        rings[sc.ring].classList.add('flow-on');
        note.appendChild(document.createTextNode(sc.text + ' '));
        chapterLink(sc.link[0], sc.link[1], note);
      });
    });
  })();

  /* ---------- plan DAG: click to complete issues ---------- */
  (function () {
    var sec = $('x-plan');
    if (!sec) return;
    var s = sec.querySelector('.fig svg');
    if (!s) return;
    var ids = ['I0101', 'I0102', 'I0103', 'I0104', 'I0105'];
    var deps = { I0103: ['I0101'], I0104: ['I0102'], I0105: ['I0103', 'I0104'] };
    var rects = Array.prototype.filter.call(s.querySelectorAll('rect'), function (r) { return r.getAttribute('width') === '148'; });
    if (rects.length < 5) return;
    var done = {}, labels = {};
    var note = el('p', { 'class': 'flow-note', 'aria-live': 'polite' });
    var fig = sec.querySelector('.fig');
    var bar = el('div', { 'class': 'flow-bar' });
    el('span', { 'class': 'lbl', style: 'display:inline;margin:0 4px 0 0' }, bar, 'Klikni na issue a dokonči ho');
    var rs = el('button', { type: 'button', 'class': 'btn ghost small', style: 'margin:0' }, bar, 'Znovu');
    fig.parentNode.insertBefore(bar, fig);
    fig.parentNode.insertBefore(note, fig.nextSibling);
    function status(id) {
      if (done[id]) return 'DONE';
      if ((deps[id] || []).some(function (d) { return !done[d]; })) return 'BLOCKED';
      return 'READY';
    }
    function paint() {
      var ready = ids.filter(function (i) { return status(i) === 'READY'; });
      var conflict = ready.indexOf('I0103') !== -1 && ready.indexOf('I0104') !== -1;
      ids.forEach(function (id, k) {
        var st = status(id), r = rects[k];
        r.setAttribute('class', 'pl-box ' + st.toLowerCase());
        labels[id].textContent = st + (conflict && id === 'I0104' ? ' · čeká na I0103' : '');
      });
      clear(note);
      var next = ready.filter(function (i) { return !(conflict && i === 'I0104'); })[0];
      el('b', null, note, 'plan next: ' + (next || 'nic') + '. ');
      note.appendChild(document.createTextNode(ready.length ? 'Připravené: ' + ready.join(', ') + '.' : (ids.every(function (i) { return done[i]; }) ? 'Všechna issues jsou DONE, milník přejde do VERIFY, dokud nemá vlastní důkazy.' : '')));
      if (conflict) note.appendChild(document.createTextNode(' I0103 a I0104 jsou obě READY ve stejné vlně, ale jejich scope se překrývá, takže se seřadí za sebe.'));
    }
    ids.forEach(function (id, k) {
      var r = rects[k];
      r.setAttribute('tabindex', '0'); r.setAttribute('role', 'button'); r.setAttribute('aria-label', id);
      r.style.cursor = 'pointer';
      var t = svg('text', { x: +r.getAttribute('x') + 140, y: +r.getAttribute('y') + 19, 'text-anchor': 'end', 'class': 'pl-st' }, s);
      labels[id] = t;
      function act() {
        var st = status(id);
        if (st === 'READY') { done[id] = true; paint(); }
        else if (st === 'DONE') { delete done[id]; ids.forEach(function (o) { if ((deps[o] || []).indexOf(id) !== -1) delete done[o]; }); delete done.I0105; paint(); }
        else { clear(note); el('b', null, note, 'plan start ' + id + ' odmítnut. '); note.appendChild(document.createTextNode('Issue je BLOCKED: čeká na ' + (deps[id] || []).filter(function (d) { return !done[d]; }).join(', ') + '.')); }
      }
      r.addEventListener('click', act);
      r.addEventListener('keydown', function (e) { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); act(); } });
    });
    rs.addEventListener('click', function () { done = {}; paint(); });
    paint();
  })();

  /* ---------- handshake diagram follows the failure selector ---------- */
  (function () {
    var sel = $('hs-sel'), sec = $('m-handshake');
    if (!sel || !sec) return;
    var s = sec.querySelector('.fig svg');
    if (!s) return;
    var lines = Array.prototype.filter.call(s.querySelectorAll('text'), function (t) { return /^[1-8]\s{2}/.test(t.textContent); });
    var failAt = [-1, 0, 0, 1, 2, 3, 4, 5, 6, 7];
    function paint() {
      var f = failAt[+sel.value];
      lines.forEach(function (t, i) { t.setAttribute('class', f === -1 || i < f ? 'ink hs-ok' : i === f ? 'hs-bad' : 'mut'); });
    }
    sel.addEventListener('change', paint);
    paint();
  })();

  /* ---------- reading paths on the chapter map ---------- */
  (function () {
    var lab = $('lab-graph');
    if (!lab || !window.mjGraph) return;
    var bar = el('div', { 'class': 'flow-bar' });
    el('span', { 'class': 'lbl', style: 'display:inline;margin:0 4px 0 0' }, bar, 'Trasy čtení');
    lab.insertBefore(bar, lab.firstChild);
    [
      ['Principy', ['problemy', 'stavba', 'h-proc', 'p-pamet', 'koordinace', 'x-lessons', 'h-prenos']],
      ['Nasazení', ['nasazeni', 'h-bootstrap', 'h-hooky', 'h-profily', 'h-finish', 'p-pravidla', 'h-git', 'h-ci']],
      ['Víc agentů a strojů', ['h-server', 'koordinace', 'x-worktree', 'm-claims', 'm-handovery', 'm-stroje', 'm-diagnostika']],
      ['Důkazy a kvalita', ['h-finish', 'h-zavazky', 'x-evidence', 'x-claims', 'h-doctor', 'h-vynuceni', 'x-release']],
      ['Pro vedení', ['problemy', 'hodnoceni', 'h-mereni', 'velka-org', 'x-lessons', 'h-prenos']]
    ].forEach(function (p) {
      var ids = p[1].filter(function (i) { return window.mjGraph.has(i); });
      var b = el('button', { type: 'button', 'class': 'seg-b' }, bar, p[0]);
      b.addEventListener('click', function () { window.mjGraph.showPath(ids, 'Trasa „' + p[0] + '“'); });
    });
  })();

  /* ---------- chart click-throughs ---------- */
  var repo = null;
  try { repo = JSON.parse($('data-repo').textContent); } catch (e) { repo = null; }
  if (repo) {
    var cad = $('chart-cadence');
    if (cad) {
      var info = el('p', { 'class': 'flow-note', 'aria-live': 'polite' }, cad.parentElement, 'Klikni na den a uvidíš jeho commity na GitHubu.');
      var marks = cad.querySelectorAll('svg .mark');
      var rel = {};
      repo.releases.forEach(function (r) { (rel[r[0]] = rel[r[0]] || []).push(r[1]); });
      repo.daily.forEach(function (d, i) {
        var g = marks[i];
        if (!g) return;
        g.style.cursor = 'pointer';
        function act() {
          Array.prototype.forEach.call(marks, function (m) { m.classList.remove('picked'); });
          g.classList.add('picked');
          clear(info);
          var cz = (+d[0].slice(8, 10)) + '. ' + (+d[0].slice(5, 7)) + '. 2026';
          el('b', null, info, cz + ': ' + d[1] + ' commitů');
          info.appendChild(document.createTextNode((rel[d[0]] ? ', vydání ' + rel[d[0]].join(' a ') : '') + '. '));
          el('a', { href: GH + '/commits/master?since=' + d[0] + '&until=' + d[0] }, info, 'Commity toho dne na GitHubu →');
          (rel[d[0]] || []).forEach(function (v) { info.appendChild(document.createTextNode(' · ')); el('a', { href: GH + '/releases/tag/' + v }, info, v); });
        }
        g.addEventListener('click', act);
        g.addEventListener('keydown', function (e) { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); act(); } });
      });
    }
    function barsTo(chartId, items, handler, hint) {
      var c = $(chartId);
      if (!c) return;
      el('p', { 'class': 'flow-note' }, c.parentElement, hint);
      Array.prototype.forEach.call(c.querySelectorAll('svg .mark'), function (g, i) {
        if (!items[i]) return;
        g.style.cursor = 'pointer';
        g.addEventListener('click', function () { handler(items[i]); });
        g.addEventListener('keydown', function (e) { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handler(items[i]); } });
      });
    }
    barsTo('chart-jobs', repo.jobs, function (j) { if (window.mjExplore) window.mjExplore('gate', j[0]); }, 'Klikni na job a otevřu jeho gates v průzkumníku.');
    var adr = $('chart-adr');
    if (adr) {
      var hint = el('p', { 'class': 'flow-note' }, adr.parentElement, 'Všechna rozhodnutí s odkazy jsou v ');
      var a = el('a', { href: '#pruzkumnik' }, hint, 'průzkumníku');
      a.addEventListener('click', function (e) { e.preventDefault(); if (window.mjExplore) window.mjExplore('adr', ''); });
      hint.appendChild(document.createTextNode('.'));
    }
    var pm = $('chart-proof'), pm2 = $('chart-proof-mini');
    [pm, pm2].forEach(function (c) {
      if (!c) return;
      var h = el('p', { 'class': 'flow-note' }, c.parentElement, 'Každé pravidlo se zněním a třídou najdeš v ');
      var l = el('a', { href: '#pruzkumnik' }, h, 'průzkumníku');
      l.addEventListener('click', function (e) { e.preventDefault(); if (window.mjExplore) window.mjExplore('rule', ''); });
      h.appendChild(document.createTextNode('.'));
    });
  }
})();

(function () {
  'use strict';
  var GH = 'https://github.com/korczis/prismatic-majordomus';
  function $(id) { return document.getElementById(id); }
  function el(tag, attrs, parent, text) {
    var e = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (text !== undefined) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  function clear(e) { while (e.firstChild) e.removeChild(e.firstChild); }
  function readJSON(id) { var e = $(id); try { return e ? JSON.parse(e.textContent) : null; } catch (x) { return null; } }
  function store(key, val) {
    try {
      if (val === undefined) return window.localStorage.getItem(key);
      if (val === null) window.localStorage.removeItem(key); else window.localStorage.setItem(key, val);
    } catch (e) { return null; }
    return null;
  }
  function norm(s) { return String(s).toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, ''); }
  function goTo(id) {
    var t = $(id);
    if (t) t.scrollIntoView({ behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth', block: 'start' });
  }
  var tracked = readJSON('data-tracked') || [];
  var trackedSet = {};
  tracked.forEach(function (p) { trackedSet[p] = true; });

  /* ---------- adoption checklist ---------- */
  var STEPS = [
    { t: 'Přečti instalační skript a nainstaluj', cmd: 'curl -fsSL https://majordomus.dev/install.sh | sh', d: 'Zapisuje jen do ~/.local/share/majordomus a dva launchery do ~/.local/bin. Nikdy sudo, žádný zásah do profilu shellu.', ch: ['stavba', 'Celá stavba'], src: 'docs/INSTALL.md' },
    { t: 'Ověř instalaci', cmd: 'majordomus --version\nmajordomus doctor', d: 'doctor ukáže, jestli vrstva .ai/ jde rozlišit a které hooky ještě nejsou zapojené.', ch: ['h-doctor', 'Doctor a preflight'], src: 'docs/INSTALL.md' },
    { t: 'Inicializuj repozitář', cmd: 'majordomus init', d: 'Založí vrstvu .ai/ s manifestem, policy, profily a výchozím scope.yaml. Repo je supervizované, jakmile existuje .ai/manifest.yaml.', ch: ['p-mapa', 'Mapa vrstvy .ai/'], src: 'docs/ADOPTION.md' },
    { t: 'Vyrenderuj instrukce pro providery', cmd: 'majordomus update', d: 'CLAUDE.md, AGENTS.md a další soubory z jedné policy, s razítkem.', ch: ['h-bootstrap', 'Bootstrap'], src: '.ai/repo/policy.yaml' },
    { t: 'Zapoj git hooky', cmd: '# řádky, které vypíše init, do .git/hooks/\n# nebo hooky verzované v repu:\ngit config core.hooksPath .githooks', d: 'pre-commit, commit-msg a pre-push se ptají stejného soudce jako CI.', ch: ['h-git', 'Git hooky'], src: '.githooks/pre-commit' },
    { t: 'Zapoj hooky session v Claude Code', cmd: 'majordomus capture install --provider claude-code', d: 'SessionStart, UserPromptSubmit, PreCompact a SessionEnd zapisují stav, nikdy neblokují.', ch: ['h-hooky', 'Hooky providera'], src: '.claude/settings.json' },
    { t: 'Připoj MCP server', cmd: '# .mcp.json spouští launcher majordomus-mcp\nmajordomus serve status', d: 'První klient se stane sdíleným serverem, další se připojí. Board ukáže, kdo na čem pracuje.', ch: ['h-server', 'Sdílený server'], src: '.mcp.json' },
    { t: 'První task od startu po finish', cmd: 'majordomus start "Opravit X" --scope src/x --requires tests,commit\nmajordomus check\nmajordomus finish --outcome completed --verify-command "make test"', d: 'Scope, závazky a finish contract na jedné malé změně. Odmítnutý finish je informace, ne překážka.', ch: ['h-finish', 'Finish contract'], src: '.ai/repo/workflows/task-lifecycle.md' }
  ];
  if ($('lab-adopt')) (function () {
    var done = {};
    try { done = JSON.parse(store('mj-adopt') || '{}') || {}; } catch (e) { done = {}; }
    var list = $('adopt-list'), fill = $('adopt-fill'), count = $('adopt-count');
    function update() {
      var n = STEPS.filter(function (_, i) { return done[i]; }).length;
      fill.style.width = (100 * n / STEPS.length) + '%';
      count.textContent = n + ' z ' + STEPS.length + ' kroků hotovo' + (n === STEPS.length ? '. Repo je supervizované.' : '');
    }
    STEPS.forEach(function (s, i) {
      var li = el('li', { 'class': 'adopt-i' + (done[i] ? ' done' : '') }, list);
      var head = el('label', { 'class': 'adopt-h' }, li);
      var cb = el('input', { type: 'checkbox', id: 'adopt-' + i }, head);
      cb.checked = !!done[i];
      el('b', null, head, s.t);
      el('p', null, li, s.d);
      var code = el('div', { 'class': 'code' }, li);
      var btn = el('button', { type: 'button' }, code, 'Kopírovat');
      var pre = el('pre', null, code);
      el('code', null, pre, s.cmd);
      btn.addEventListener('click', function () {
        try { navigator.clipboard.writeText(s.cmd).then(function () { btn.textContent = 'Zkopírováno'; setTimeout(function () { btn.textContent = 'Kopírovat'; }, 1500); }, function () {}); } catch (e) { /* no clipboard */ }
      });
      var links = el('p', { 'class': 'adopt-l' }, li);
      var a = el('a', { href: '#' + s.ch[0] }, links, s.ch[1]);
      a.addEventListener('click', function (e) { e.preventDefault(); goTo(s.ch[0]); });
      if (trackedSet[s.src]) {
        links.appendChild(document.createTextNode(' · zdroj: '));
        var g = el('a', { href: GH + '/blob/master/' + s.src, 'class': 'src' }, links);
        el('code', null, g, s.src);
      }
      cb.addEventListener('change', function () {
        if (cb.checked) done[i] = true; else delete done[i];
        li.classList.toggle('done', cb.checked);
        store('mj-adopt', JSON.stringify(done));
        update();
      });
    });
    $('adopt-reset').addEventListener('click', function () {
      done = {}; store('mj-adopt', null);
      list.querySelectorAll('input').forEach(function (c) { c.checked = false; });
      list.querySelectorAll('.adopt-i').forEach(function (l) { l.classList.remove('done'); });
      update();
    });
    update();
  })();

  /* ---------- explorer ---------- */
  var ex = readJSON('data-explore'), gates = readJSON('data-gates');
  if (ex && gates && $('lab-explore')) (function () {
    var tabs = $('ex-tabs'), q = $('ex-q'), chips = $('ex-chips'), list = $('ex-list'), count = $('ex-count'), stats = $('ex-stats');
    var SETS = {
      adr: { label: 'ADR', items: ex.adrs, facet: 's', facetName: 'stavu',
        row: function (a) { return { title: 'ADR ' + a.n + ' · ' + a.t, meta: [a.s, a.d], href: GH + '/blob/master/' + a.p, text: a.t + ' ' + a.n + ' ' + a.s + ' ' + a.d }; } },
      rule: { label: 'Pravidla', items: ex.rules, facet: 'c', facetName: 'třídy',
        row: function (r) { return { title: r.id + (r.v ? '@' + r.v : ''), meta: [r.c, r.ns === 'vendor' ? 'vendorovaný balíček' : 'pravidlo repa', r.st], body: r.s, href: GH + '/blob/master/' + r.p, text: r.id + ' ' + r.s + ' ' + r.c + ' ' + r.ns }; } },
      gate: { label: 'CI gates', items: gates.gates, facet: 'job', facetName: 'jobu',
        row: function (g) {
          var cmd = (g.run || '').split(' ')[0];
          return { title: g.id, meta: [g.job, g.a ? 'always' : 'podle cest', g.o ? 'on-demand' : ''], body: g.s, href: trackedSet[cmd] ? GH + '/blob/master/' + cmd : null, cmd: g.run, text: g.id + ' ' + g.s + ' ' + g.job + ' ' + g.run };
        } }
    };
    SETS.caps = { label: 'Capabilities', items: ex.caps || [], facet: 'm', facetName: 'modulu',
      row: function (c) { return { title: c.id + ' · ' + c.t, meta: [c.k, c.e, c.st], body: c.d, cmd: [c.cli ? 'majordomus ' + c.cli : '', c.tool || '', c.http || ''].filter(Boolean).join('   ·   '), href: GH + '/blob/master/apps/majordomus-cli/src/capability/builtin/' + c.m + '.rs', text: [c.id, c.t, c.d, c.tool, c.http, c.cli, c.m, c.e].join(' ') }; } };
    SETS.claims = { label: 'Garance', items: ex.claims || [], facet: 's', facetName: 'statusu',
      row: function (c) { return { title: c.id, meta: [c.s], body: c.c, cmd: c.t && c.t !== '-' ? c.t : '', href: 'https://majordomus.dev/guarantees/' + c.id + '/', text: c.id + ' ' + c.c + ' ' + c.s + ' ' + c.t }; } };
    SETS.ucs = { label: 'Use cases', items: ex.ucs || [], facet: 'c', facetName: 'kategorie',
      row: function (u) { return { title: u.t, meta: [u.c, u.tg], body: u.s, cmd: u.cmd ? 'příkazy: ' + u.cmd : '', href: GH + '/blob/master/' + u.p, text: [u.id, u.t, u.s, u.c, u.cmd].join(' ') }; } };
    var cur = 'adr', facet = null;
    window.mjExplore = function (tab, query, facetValue) {
      if (!SETS[tab]) return;
      cur = tab; facet = facetValue || null; q.value = query || ''; render();
      var host = $('lab-explore');
      if (host) host.scrollIntoView({ behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth', block: 'start' });
    };
    window.mjCaps = {};
    (ex.caps || []).forEach(function (c) { window.mjCaps[c.id] = true; if (c.tool) window.mjCaps[c.tool] = true; });
    function facetValues(set) {
      var c = {};
      set.items.forEach(function (it) { var v = it[set.facet] || '—'; c[v] = (c[v] || 0) + 1; });
      return Object.keys(c).sort(function (a, b) { return c[b] - c[a]; }).map(function (k) { return [k, c[k]]; });
    }
    function renderStats(set) {
      clear(stats);
      var vals = facetValues(set), total = set.items.length;
      var head = el('p', { 'class': 'ex-total' }, stats);
      el('b', null, head, String(total));
      head.appendChild(document.createTextNode(' ' + ({ adr: 'rozhodnutí', rule: 'pravidel', gate: 'gates', caps: 'capabilities', claims: 'garancí', ucs: 'use cases' }[cur]) + ' podle ' + set.facetName));
      var bar = el('div', { 'class': 'ex-bar', role: 'img', 'aria-label': vals.map(function (v) { return v[0] + ' ' + v[1]; }).join(', ') }, stats);
      vals.slice(0, 6).forEach(function (v, i) {
        var seg = el('i', { 'class': 'ex-seg s' + Math.min(i, 3), title: v[0] + ': ' + v[1] }, bar);
        seg.style.flexGrow = v[1];
      });
    }
    function renderChips(set) {
      clear(chips);
      var all = el('button', { type: 'button', 'class': 'chip' + (facet === null ? ' on' : '') }, chips, 'vše');
      all.addEventListener('click', function () { facet = null; render(); });
      facetValues(set).forEach(function (v) {
        var b = el('button', { type: 'button', 'class': 'chip' + (facet === v[0] ? ' on' : '') }, chips, v[0] + ' · ' + v[1]);
        b.addEventListener('click', function () { facet = facet === v[0] ? null : v[0]; render(); });
      });
    }
    function render() {
      var set = SETS[cur];
      Array.prototype.forEach.call(tabs.children, function (b) { b.setAttribute('aria-selected', b.dataset.k === cur ? 'true' : 'false'); });
      renderStats(set); renderChips(set);
      var toks = norm(q.value).split(/\s+/).filter(Boolean);
      var rows = set.items.map(set.row).filter(function (r, i) {
        var it = set.items[i];
        if (facet !== null && (it[set.facet] || '—') !== facet) return false;
        var t = norm(r.text);
        return toks.every(function (tk) { return t.indexOf(tk) !== -1; });
      });
      if (cur === 'adr') rows.reverse();
      clear(list);
      count.textContent = rows.length + ' z ' + set.items.length + (rows.length > 60 ? ', zobrazeno prvních 60; zúži filtr' : '');
      rows.slice(0, 60).forEach(function (r) {
        var li = el('li', null, list);
        var h = el('p', { 'class': 'ex-t' }, li);
        if (r.href) { var a = el('a', { href: r.href, 'class': 'src' }, h); el('span', null, a, r.title); }
        else el('span', null, h, r.title);
        var meta = el('span', { 'class': 'ex-meta' }, h);
        r.meta.filter(Boolean).forEach(function (m) { el('span', { 'class': 'pill reports' }, meta, m); });
        if (r.body) el('p', { 'class': 'ex-b' }, li, r.body);
        if (r.cmd) { var c = el('p', { 'class': 'ex-b' }, li); el('code', null, c, r.cmd); }
      });
      if (!rows.length) el('li', { 'class': 'lab-note' }, list, 'Nic neodpovídá filtru.');
    }
    Object.keys(SETS).forEach(function (k) {
      var b = el('button', { type: 'button', role: 'tab', 'class': 'seg-b' }, tabs, SETS[k].label + ' · ' + SETS[k].items.length);
      b.dataset.k = k;
      b.addEventListener('click', function () { cur = k; facet = null; render(); });
    });
    q.addEventListener('input', render);
    render();
  })();

  /* ---------- release markers and gate chips link out ---------- */
  document.querySelectorAll('#chart-cadence .mark[aria-label^="vydání"]').forEach(function (g) {
    var label = g.getAttribute('aria-label') || '';
    var m = label.match(/v\d+\.\d+\.\d+/);
    if (!m) return;
    var a = document.createElementNS('http://www.w3.org/2000/svg', 'a');
    a.setAttribute('href', GH + '/releases/tag/' + m[0]);
    a.setAttribute('target', '_blank');
    g.parentNode.insertBefore(a, g);
    a.appendChild(g);
    g.style.cursor = 'pointer';
  });
  if ($('chart-cadence') && $('cisla-tempo')) {
    var rel = readJSON('data-repo');
    if (rel) {
      var box = el('p', { 'class': 'rel-links' }, $('chart-cadence').parentElement);
      el('span', { 'class': 'lbl', style: 'display:inline;margin-right:8px' }, box, 'Vydání na GitHubu');
      rel.releases.forEach(function (r, i) {
        if (i) box.appendChild(document.createTextNode(' '));
        el('a', { href: GH + '/releases/tag/' + r[1], 'class': 'chip rel' }, box, r[1]);
      });
    }
  }
  var ciOut = $('ci-out');
  if (ciOut && gates) {
    var runOf = {};
    gates.gates.forEach(function (g) { var c = (g.run || '').split(' ')[0]; if (trackedSet[c]) runOf[g.id] = c; });
    new MutationObserver(function () {
      ciOut.querySelectorAll('.chip.static:not([data-linked])').forEach(function (c) {
        c.setAttribute('data-linked', '1');
        var p = runOf[c.textContent];
        if (!p) return;
        var a = document.createElement('a');
        a.className = c.className + ' gl';
        a.setAttribute('data-linked', '1');
        a.href = GH + '/blob/master/' + p;
        a.textContent = c.textContent;
        a.setAttribute('aria-label', c.getAttribute('aria-label') || c.textContent);
        ['pointermove', 'pointerleave', 'focus', 'blur'].forEach(function (ev) { a.addEventListener(ev, function (e) { c.dispatchEvent(new e.constructor(e.type, e)); }); });
        c.style.display = 'none';
        c.parentNode.insertBefore(a, c);
      });
    }).observe(ciOut, { childList: true, subtree: true });
    var ta = $('ci-paths');
    if (ta) ta.dispatchEvent(new Event('input'));
  }

  /* ---------- glossary popovers in running text ---------- */
  var TERMS = [
    ['finish contract', /finish contract\p{L}*/iu], ['handover', /handover\p{L}*/iu], ['claim', /claim(?:y|u|em|ů|ech|s)?(?![\p{L}_-])/iu],
    ['checkpoint', /checkpoint\p{L}*/iu], ['briefing', /briefing\p{L}*/iu], ['ledger', /ledger\p{L}*/iu],
    ['doktrína', /doktrín\p{L}*/iu], ['epizoda', /epizod\p{L}*/iu], ['razítko', /razítk\p{L}*/iu],
    ['handshake', /handshake\p{L}*/iu], ['journal', /journal\p{L}*/iu], ['preflight', /preflight\p{L}*/iu],
    ['runtime', /runtime(?![\p{L}_-])/iu], ['harness', /harness(?![\p{L}_-])/iu], ['capability', /capabilit\p{L}*/iu],
    ['projekce', /projekc\p{L}*/iu], ['worktree', /worktree(?:s|u|y|e)?(?![\p{L}_-])/iu], ['milník', /milník\p{L}*/iu], ['intent', /intent(?:u|y|ů|em)?(?![\p{L}_.-])/iu], ['garance', /garanc\p{L}*/iu], ['závazek', /závaz\p{L}*/iu], ['trust.allow', /trust\.allow/u],
    ['kandidát znalosti', /kandidát\p{L}* znalost\p{L}*/iu], ['ADR', /ADR(?![\p{L}_-])/u], ['fold', /fold(?:em|u)?(?![\p{L}_-])/u]
  ];
  var gl = {};
  document.querySelectorAll('#gloss > div').forEach(function (row) {
    var a = row.querySelector('dt a');
    if (a) gl[a.textContent.trim()] = { def: row.querySelector('dd').textContent.trim(), href: a.getAttribute('href').slice(1) };
  });
  var pop = el('div', { 'class': 'term-pop', role: 'tooltip', id: 'term-pop' }, document.body);
  pop.hidden = true;
  var popTimer = null, popFor = null;
  function showPop(span) {
    clearTimeout(popTimer);
    var g = gl[span.dataset.term];
    if (!g) return;
    clear(pop);
    el('b', null, pop, span.dataset.term);
    el('p', null, pop, g.def);
    var a = el('a', { href: '#' + g.href }, pop, 'Víc v kapitole →');
    a.addEventListener('click', function (e) { e.preventDefault(); pop.hidden = true; goTo(g.href); });
    pop.hidden = false;
    var r = span.getBoundingClientRect();
    var w = pop.offsetWidth, h = pop.offsetHeight;
    pop.style.left = Math.min(Math.max(8, r.left + r.width / 2 - w / 2), window.innerWidth - w - 8) + 'px';
    pop.style.top = (r.top - h - 10 < 8 ? r.bottom + 10 : r.top - h - 10) + 'px';
    popFor = span;
  }
  function hidePop() { popTimer = setTimeout(function () { pop.hidden = true; popFor = null; }, 220); }
  pop.addEventListener('pointerenter', function () { clearTimeout(popTimer); });
  pop.addEventListener('pointerleave', hidePop);
  window.addEventListener('scroll', function () { if (!pop.hidden) { pop.hidden = true; popFor = null; } }, { passive: true });
  var SKIP = 'code, pre, a, h1, h2, h3, h4, .tldr, .lab, .see, .pn, svg, table, .term, .axioms, dt, dd, button, label, .facts, .chart';
  var linked = 0;
  document.querySelectorAll('main section[id]').forEach(function (sec) {
    if (sec.id === 'slovnik' || sec.id === 'prikazy' || /-prikazy$/.test(sec.id)) return;
    var used = {}, budget = 5;
    var walker = document.createTreeWalker(sec, NodeFilter.SHOW_TEXT, {
      acceptNode: function (n) {
        if (!n.nodeValue.trim()) return NodeFilter.FILTER_REJECT;
        var p = n.parentElement;
        if (!p || p.closest(SKIP)) return NodeFilter.FILTER_REJECT;
        if (!p.closest('p, li')) return NodeFilter.FILTER_REJECT;
        return NodeFilter.FILTER_ACCEPT;
      }
    });
    var nodes = [];
    while (walker.nextNode()) nodes.push(walker.currentNode);
    nodes.forEach(function (node) {
      if (budget <= 0) return;
      TERMS.forEach(function (t) {
        if (budget <= 0 || used[t[0]] || !gl[t[0]] || !node.parentNode) return;
        if (t[0] === 'razítko' && !/^h-/.test(sec.id)) return;
        var m = t[1].exec(node.nodeValue);
        if (!m) return;
        var before = node.nodeValue.slice(0, m.index);
        if (before && /[\p{L}_.-]$/u.test(before)) return;
        var after = node.splitText(m.index);
        after.splitText(m[0].length);
        var span = document.createElement('span');
        span.className = 'gterm';
        span.tabIndex = 0;
        span.dataset.term = t[0];
        span.setAttribute('aria-describedby', 'term-pop');
        span.textContent = after.nodeValue;
        after.parentNode.replaceChild(span, after);
        span.addEventListener('pointerenter', function () { showPop(span); });
        span.addEventListener('pointerleave', hidePop);
        span.addEventListener('focus', function () { showPop(span); });
        span.addEventListener('blur', hidePop);
        span.addEventListener('click', function () { if (popFor === span && !pop.hidden) { pop.hidden = true; popFor = null; } else showPop(span); });
        used[t[0]] = true; budget--; linked++;
        node = span.nextSibling && span.nextSibling.nodeType === 3 ? span.nextSibling : node;
      });
    });
  });
  document.documentElement.setAttribute('data-terms', String(linked));
})();

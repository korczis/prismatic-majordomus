(function () {
  'use strict';
  /* Reading room: the repository's Markdown, embedded at build time, rendered in place. */
  var GH = 'https://github.com/korczis/prismatic-majordomus';
  var ARTIFACT = 'https://claude.ai/artifact/XgCzwV1MFNDL1ssJHstPQ1';
  var CDN = 'https://cdnjs.cloudflare.com/ajax/libs/';
  var LIB = {
    md: 'markdown-it/13.0.2/markdown-it.min.js',
    pure: 'dompurify/3.1.6/purify.min.js',
    mermaid: 'mermaid/10.9.1/mermaid.min.js',
    echarts: 'echarts/5.5.0/echarts.min.js'
  };
  function $(id) { return document.getElementById(id); }
  function el(tag, attrs, parent, text) {
    var e = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (text !== undefined) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  function clear(e) { while (e.firstChild) e.removeChild(e.firstChild); }
  var host = $('lab-reader'), idxEl = $('data-corpus-index');
  if (!host || !idxEl) return;
  var IDX = JSON.parse(idxEl.textContent);
  var DOCS = IDX.docs, BY = {};
  DOCS.forEach(function (d, i) { d.n = i; BY[d.p] = d; });
  var KINDS = [
    ['doc', 'Dokumentace'], ['adr', 'Rozhodnutí (ADR)'], ['rule', 'Pravidla'],
    ['claim', 'Garance'], ['usecase', 'Use cases'], ['guide', 'Průvodci a skills']
  ];
  var KNAME = {}; KINDS.forEach(function (k) { KNAME[k[0]] = k[1]; });
  var TRACKED = {};
  try { JSON.parse($('data-repo-paths').textContent).forEach(function (p) { TRACKED[p] = 1; }); } catch (e) { /* optional */ }
  var reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
  function dark() { return document.documentElement.classList.contains(document.body.getAttribute('data-theme-class') || ''); }
  function cssv(n) { return getComputedStyle(document.documentElement).getPropertyValue(n).trim(); }
  /* The series of the declared design, read from the page and resolved to red, green and blue: a chart
     library parses colours itself and does not know every notation a stylesheet may use. */
  function paint(name) {
    var c = document.createElement('canvas'); c.width = c.height = 1;
    var x = c.getContext('2d');
    x.fillStyle = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    x.fillRect(0, 0, 1, 1);
    var d = x.getImageData(0, 0, 1, 1).data;
    return '#' + [d[0], d[1], d[2]].map(function (v) { return ('0' + v.toString(16)).slice(-2); }).join('');
  }
  function series() { return [1, 2, 3, 4, 5, 6].map(function (i) { return paint('--mj-series-' + i); }); }
  function palette() { return series(); }

  /* ---------- loading ---------- */
  var scripts = {};
  function load(name) {
    if (scripts[name]) return scripts[name];
    scripts[name] = new Promise(function (ok, fail) {
      var s = document.createElement('script');
      s.src = CDN + LIB[name]; s.async = true;
      s.onload = function () { ok(); };
      s.onerror = function () { scripts[name] = null; fail(new Error('knihovnu ' + name + ' se nepodařilo načíst')); };
      document.head.appendChild(s);
    });
    return scripts[name];
  }
  /* An embedded, gzipped JSON value: the corpus, and its code blocks as the site highlights them. */
  function unpack(id) {
    return new Promise(function (ok, fail) {
      try {
        var b64 = $(id).textContent.trim();
        var bin = atob(b64), n = bin.length, u8 = new Uint8Array(n);
        for (var i = 0; i < n; i++) u8[i] = bin.charCodeAt(i);
        if (typeof DecompressionStream !== 'function') { fail(new Error('prohlížeč neumí rozbalit vložená data')); return; }
        var stream = new Blob([u8]).stream().pipeThrough(new DecompressionStream('gzip'));
        new Response(stream).text().then(function (t) { ok(JSON.parse(t)); }, fail);
      } catch (e) { fail(e); }
    });
  }
  var texts = null, textsP = null;
  function corpus() {
    if (!textsP) textsP = unpack('data-corpus').then(function (t) { texts = t; return t; });
    return textsP;
  }
  /* Every code block of the corpus, highlighted at build time by the site's own engine (Zola's
     giallo, handbook/code.py) and coloured by share/design/code.css: one highlighter on every
     surface (ADR 0166 §7). Keyed by the language and the text markdown-it hands its hook. */
  var CODE = {}, codeP = null;
  function code() {
    if (!codeP) codeP = $('data-code') ? unpack('data-code').then(function (t) { CODE = t; return t; }) : Promise.resolve(CODE);
    return codeP;
  }

  /* ---------- link resolution ---------- */
  function norm(base, target) {
    var parts = (base.indexOf('/') >= 0 ? base.slice(0, base.lastIndexOf('/')).split('/') : []);
    target.split('/').forEach(function (s) {
      if (s === '..') parts.pop(); else if (s && s !== '.') parts.push(s);
    });
    return parts.join('/');
  }
  function slug(s) {
    return String(s).toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
  }
  function docHref(p, frag) { return '#doc.' + p + (frag ? '::' + frag : ''); }
  function ghHref(p) { return GH + '/blob/' + IDX.commit + '/' + p; }
  function ruleDoc(id) { var n = IDX.rules[id.replace(/@\d+$/, '')]; return n === undefined ? null : DOCS[n]; }
  function adrDoc(num) { var n = IDX.adrs[('0000' + num).slice(-4)]; return n === undefined ? null : DOCS[n]; }
  function docLink(d, parent, label) {
    var a = el('a', { href: docHref(d.p), 'class': 'md-in k-' + d.k, title: KNAME[d.k] + ': ' + d.p }, parent, label || d.t);
    return a;
  }

  /* ---------- state ---------- */
  var state = { kind: 'all', q: '', full: false, hot: false, view: 'list', cur: null };
  var trail = [];
  var head = $('rd-head'), body = $('rd-body'), side = $('rd-side'), docBox = $('rd-doc');
  var list = $('rd-list'), count = $('rd-count'), kindsBar = $('rd-kinds'), q = $('rd-q');
  $('rd-commit').textContent = IDX.commit;
  $('rd-total').textContent = DOCS.length + ' dokumentů z commitu ' + IDX.commit;

  /* ---------- list ---------- */
  function kindButtons() {
    clear(kindsBar);
    var counts = { all: DOCS.length };
    DOCS.forEach(function (d) { counts[d.k] = (counts[d.k] || 0) + 1; });
    [['all', 'Vše']].concat(KINDS).forEach(function (k) {
      var b = el('button', { type: 'button', 'class': 'seg-b', role: 'tab', 'data-k': k[0], 'aria-selected': String(state.kind === k[0]) }, kindsBar, k[1] + ' ' + (counts[k[0]] || 0));
      b.addEventListener('click', function () { state.kind = k[0]; kindButtons(); renderList(); });
    });
  }
  function matches(d, needle) {
    if (!needle) return true;
    var hay = (d.t + ' ' + d.p + ' ' + (d.rid || '')).toLowerCase();
    if (hay.indexOf(needle) >= 0) return true;
    return state.full && texts ? texts[d.n].toLowerCase().indexOf(needle) >= 0 : false;
  }
  function renderList() {
    var needle = state.q.trim().toLowerCase();
    var rows = DOCS.filter(function (d) { return (state.kind === 'all' || d.k === state.kind) && matches(d, needle); });
    if (state.hot) rows = rows.slice().sort(function (a, b) { return b.i.length - a.i.length || (a.p < b.p ? -1 : 1); });
    clear(list);
    rows.forEach(function (d) {
      var li = el('li', { 'class': 'rd-item' + (state.cur === d ? ' on' : '') }, list);
      var a = docLink(d, li);
      a.classList.add('rd-t');
      el('span', { 'class': 'rd-k k-' + d.k }, li, KNAME[d.k]);
      el('code', { 'class': 'rd-p' }, li, d.p);
      el('span', { 'class': 'rd-n', title: 'odkazuje na / odkazují sem' }, li, '→ ' + d.o.length + '  ← ' + d.i.length);
    });
    count.textContent = rows.length + ' z ' + DOCS.length + (state.full && !texts ? ' · text se načítá' : '');
  }
  q.addEventListener('input', function () { state.q = q.value; renderList(); });
  $('rd-full').addEventListener('change', function (e) {
    state.full = e.target.checked;
    if (state.full) corpus().then(renderList, renderList);
    renderList();
  });
  $('rd-hot').addEventListener('change', function (e) { state.hot = e.target.checked; renderList(); });

  /* ---------- chapters that mention a document ---------- */
  var chapters = {};
  function decorate(root) {
    Array.prototype.forEach.call(root.querySelectorAll('a[href^="' + GH + '/blob/"]'), function (a) {
      if (a.dataset.md || a.classList.contains('gh-x') || host.contains(a)) return;
      var m = a.getAttribute('href').slice(GH.length).match(/^\/blob\/[^/]+\/([^#?]+)/);
      a.dataset.md = '1';
      if (!m || !BY[m[1]]) return;
      var d = BY[m[1]], gh = a.getAttribute('href');
      a.setAttribute('href', docHref(d.p));
      a.classList.add('md-open');
      a.setAttribute('title', 'Otevřít v čítárně: ' + d.t);
      var x = el('a', { href: gh, 'class': 'gh-x', title: 'Zdroj na GitHubu', 'aria-label': d.p + ' na GitHubu', 'data-md': '1' }, null, '↗');
      a.parentNode.insertBefore(x, a.nextSibling);
      var sec = a.closest('main section[id]');
      if (sec && sec.id !== 'citarna') {
        (chapters[d.p] = chapters[d.p] || {})[sec.id] = (sec.querySelector('h2') || {}).firstChild ? sec.querySelector('h2').firstChild.textContent.trim() : sec.id;
      }
    });
  }
  var main = document.querySelector('main');
  decorate(main);
  var pend = false;
  new MutationObserver(function () {
    if (pend) return;
    pend = true;
    requestAnimationFrame(function () { pend = false; decorate(main); });
  }).observe(main, { childList: true, subtree: true });

  /* ---------- rendering ---------- */
  var mdi = null;
  function engine() {
    if (mdi) return mdi;
    function esc(s) { return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;'); }
    mdi = window.markdownit({
      html: true, linkify: true, typographer: false,
      highlight: function (str, lang) {
        if (lang === 'mermaid') return '<pre class="mermaid-src">' + esc(str) + '</pre>';
        var lit = CODE[lang + '\u0000' + str];
        if (lit) return lit;
        /* a block the build read differently: plain text, in the same frame */
        return '<pre class="giallo z-code"><code' + (lang ? ' data-lang="' + esc(lang).replace(/"/g, '&quot;') + '"' : '') + '>' + esc(str) + '</code></pre>';
      }
    });
    return mdi;
  }
  function frontmatter(text) {
    var m = text.match(/^---\n([\s\S]*?)\n---\n/);
    if (!m) return { meta: [], body: text };
    var meta = [], cur = null;
    m[1].split('\n').forEach(function (ln) {
      var kv = ln.match(/^([A-Za-z_][\w-]*):\s*(.*)$/);
      if (kv) { cur = [kv[1], kv[2]]; meta.push(cur); }
      else if (cur && ln.trim()) cur[1] += (cur[1] ? ' ' : '') + ln.trim();
    });
    return { meta: meta, body: text.slice(m[0].length) };
  }
  function linkValue(parent, value) {
    /* identifiers and paths inside a metadata value become links */
    var re = /((?:project|majordomus|ai)\.[a-z0-9][a-z0-9.-]*[a-z0-9](?:@\d+)?)|((?:[\w.-]+\/)+[\w.-]+\.\w+)|(adr-\d{4})/g;
    var last = 0, m;
    while ((m = re.exec(value))) {
      if (m.index > last) parent.appendChild(document.createTextNode(value.slice(last, m.index)));
      var d = m[1] ? ruleDoc(m[1]) : m[3] ? adrDoc(m[3].slice(4)) : BY[m[2]];
      if (d) docLink(d, parent, m[0]);
      else if (m[2] && TRACKED[m[2]]) el('a', { href: ghHref(m[2]), 'class': 'md-gh' }, parent, m[0]);
      else parent.appendChild(document.createTextNode(m[0]));
      last = m.index + m[0].length;
    }
    if (last < value.length) parent.appendChild(document.createTextNode(value.slice(last)));
  }
  function rewriteLinks(d) {
    Array.prototype.forEach.call(body.querySelectorAll('a[href]'), function (a) {
      var h = a.getAttribute('href');
      if (/^[a-z][a-z0-9+.-]*:/i.test(h)) { a.classList.add('md-ext'); a.setAttribute('rel', 'noopener'); return; }
      if (h.charAt(0) === '#') { a.setAttribute('href', docHref(d.p, slug(decodeURIComponent(h.slice(1))))); a.classList.add('md-frag'); return; }
      if (h.charAt(0) === '/') { a.setAttribute('href', 'https://majordomus.dev' + h); a.classList.add('md-ext'); return; }
      var parts = h.split('#'), res = norm(d.p, decodeURIComponent(parts[0].split('?')[0]));
      if (BY[res]) {
        a.setAttribute('href', docHref(res, parts[1] ? slug(decodeURIComponent(parts[1])) : ''));
        a.classList.add('md-in', 'k-' + BY[res].k);
        a.setAttribute('title', KNAME[BY[res].k] + ': ' + BY[res].t);
      } else {
        a.setAttribute('href', GH + '/' + (/\.[\w]+$/.test(res) ? 'blob' : 'tree') + '/' + IDX.commit + '/' + res + (parts[1] ? '#' + parts[1] : ''));
        a.classList.add('md-gh');
        a.setAttribute('title', 'Na GitHubu: ' + res);
      }
    });
    /* code spans that name a rule, a document or a tracked file */
    Array.prototype.forEach.call(body.querySelectorAll('code'), function (c) {
      if (c.closest('a') || c.closest('pre')) return;
      var t = c.textContent.trim(), target = ruleDoc(t) || BY[t] || BY[t.replace(/^\.\//, '')];
      var a = null;
      if (target && target !== d) { a = el('a', { href: docHref(target.p), 'class': 'md-in k-' + target.k, title: KNAME[target.k] + ': ' + target.t }); }
      else if (TRACKED[t]) { a = el('a', { href: ghHref(t), 'class': 'md-gh', title: 'Na GitHubu: ' + t }); }
      if (!a) return;
      c.parentNode.insertBefore(a, c); a.appendChild(c);
    });
    /* "ADR 0044" in running text */
    var walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT, null), nodes = [], n;
    while ((n = walker.nextNode())) {
      if (/ADR[ -]\d{1,4}/.test(n.nodeValue) && !n.parentNode.closest('a, pre, code')) nodes.push(n);
    }
    nodes.forEach(function (node) {
      var frag = document.createDocumentFragment(), s = node.nodeValue, re = /ADR[ -](\d{1,4})\b/g, last = 0, m;
      while ((m = re.exec(s))) {
        var target = adrDoc(m[1]);
        if (!target || target === d) continue;
        frag.appendChild(document.createTextNode(s.slice(last, m.index)));
        docLink(target, frag, m[0]);
        last = m.index + m[0].length;
      }
      if (!last) return;
      frag.appendChild(document.createTextNode(s.slice(last)));
      node.parentNode.replaceChild(frag, node);
    });
  }
  function outline() {
    var out = [], seen = {};
    Array.prototype.forEach.call(body.querySelectorAll('h1, h2, h3, h4'), function (h) {
      var s = slug(h.textContent) || 'oddil', k = s, i = 1;
      while (seen[k]) k = s + '-' + (i++);
      seen[k] = 1;
      h.id = 'md-' + k; h.dataset.slug = k;
      if (h.tagName === 'H2' || h.tagName === 'H3' || h.tagName === 'H1') out.push({ slug: k, text: h.textContent, level: h.tagName === 'H3' ? 3 : 2 });
      el('a', { href: docHref(state.cur.p, k), 'class': 'h-anchor md-ha', 'aria-label': 'Odkaz na oddíl' }, h, '#');
    });
    return out;
  }
  function mermaids() {
    var blocks = body.querySelectorAll('pre.mermaid-src');
    if (!blocks.length) return;
    load('mermaid').then(function () {
      var m = window.mermaid;
      m.initialize({
        startOnLoad: false, securityLevel: 'strict', theme: 'base',
        fontFamily: cssv('--body') || 'sans-serif',
        themeVariables: {
          darkMode: dark(), background: paint('--panel'), primaryColor: paint('--tint') || paint('--code-bg'),
          primaryTextColor: paint('--fg'), primaryBorderColor: paint('--s1'), lineColor: paint('--muted'),
          secondaryColor: paint('--code-bg'), tertiaryColor: paint('--panel'), textColor: paint('--fg'),
          mainBkg: paint('--tint') || paint('--code-bg'), nodeBorder: paint('--s1'), clusterBkg: paint('--panel'),
          clusterBorder: paint('--line'), edgeLabelBackground: paint('--panel'), actorBkg: paint('--tint') || paint('--code-bg'),
          actorBorder: paint('--s1'), actorTextColor: paint('--fg'), signalColor: paint('--fg'), signalTextColor: paint('--fg'),
          noteBkgColor: paint('--tldr-bg'), noteTextColor: paint('--fg'), noteBorderColor: paint('--tldr-line'), fontSize: '14px'
        }
      });
      Array.prototype.forEach.call(blocks, function (pre, i) {
        if (!pre.parentNode) return;
        var src = pre.textContent, fig = el('figure', { 'class': 'md-mermaid' });
        pre.parentNode.replaceChild(fig, pre);
        m.render('mmd-' + Date.now() + '-' + i, src).then(function (r) {
          fig.innerHTML = r.svg;
          var det = el('details', { 'class': 'md-src' }, fig);
          el('summary', null, det, 'Zdroj diagramu');
          el('pre', { 'class': 'md-pre' }, det, src);
        }, function () {
          el('pre', { 'class': 'md-pre' }, fig, src);
        });
      });
    }, function () { /* the source stays readable */ });
  }
  function sideBlock(title, items, more) {
    if (!items.length) return;
    var box = el('div', { 'class': 'rd-box' }, side);
    el('p', { 'class': 'rd-box-h' }, box, title + ' · ' + items.length);
    var ul = el('ul', null, box);
    items.forEach(function (make, i) {
      var li = el('li', null, ul);
      if (i >= more) li.hidden = true;
      make(li);
    });
    if (items.length > more) {
      var b = el('button', { type: 'button', 'class': 'btn ghost small' }, box, 'Ukázat všech ' + items.length);
      b.addEventListener('click', function () {
        Array.prototype.forEach.call(ul.children, function (li) { li.hidden = false; });
        b.remove();
      });
    }
  }
  function renderSide(d, toc) {
    clear(side);
    sideBlock('Obsah', toc.map(function (h) {
      return function (li) { if (h.level === 3) li.className = 'l3'; el('a', { href: docHref(d.p, h.slug), 'class': 'md-frag' }, li, h.text); };
    }), 40);
    var ch = chapters[d.p] || {};
    sideBlock('V příručce', Object.keys(ch).map(function (id) {
      return function (li) { el('a', { href: '#' + id, 'class': 'rd-ch' }, li, ch[id]); };
    }), 12);
    sideBlock('Odkazují sem', d.i.map(function (n) { return function (li) { docLink(DOCS[n], li); }; }), 10);
    sideBlock('Odkazuje na', d.o.map(function (n) { return function (li) { docLink(DOCS[n], li); }; }), 10);
    if (!side.firstChild) el('p', { 'class': 'lab-note' }, side, 'Dokument nemá oddíly ani odkazy na jiné dokumenty.');
  }
  function renderHead(d, meta) {
    clear(head);
    var top = el('div', { 'class': 'rd-top' }, head);
    if (trail.length > 1) {
      var back = el('button', { type: 'button', 'class': 'btn ghost small' }, top, '← Zpět');
      back.addEventListener('click', function () { trail.pop(); var p = trail.pop(); if (p) open(p); });
    }
    var lst = el('button', { type: 'button', 'class': 'btn ghost small' }, top, 'Seznam');
    lst.addEventListener('click', function () { $('rd-kinds').scrollIntoView({ behavior: reduce ? 'auto' : 'smooth', block: 'start' }); });
    var prev = DOCS[d.n - 1], next = DOCS[d.n + 1];
    if (prev && prev.k === d.k) docLink(prev, top, '‹ předchozí').classList.add('btn', 'ghost', 'small');
    if (next && next.k === d.k) docLink(next, top, 'další ›').classList.add('btn', 'ghost', 'small');
    el('a', { href: ghHref(d.p), 'class': 'btn ghost small md-gh' }, top, 'GitHub ↗');
    var cp = el('button', { type: 'button', 'class': 'btn ghost small' }, top, 'Kopírovat odkaz');
    cp.addEventListener('click', function () {
      var url = ARTIFACT + docHref(d.p), old = cp.textContent;
      function ok() { cp.textContent = 'odkaz zkopírován'; setTimeout(function () { cp.textContent = old; }, 1500); }
      try { navigator.clipboard.writeText(url).then(ok, function () { cp.textContent = url; }); } catch (e) { cp.textContent = url; }
    });
    var crumbs = el('p', { 'class': 'rd-crumbs' }, head);
    el('span', { 'class': 'rd-k k-' + d.k }, crumbs, KNAME[d.k]);
    el('code', null, crumbs, d.p);
    el('span', { 'class': 'rd-n' }, crumbs, Math.max(1, Math.round(d.b / 1024)) + ' KiB · commit ' + IDX.commit);
    el('h3', { 'class': 'rd-title' }, head, d.t);
    if (meta.length) {
      var det = el('details', { 'class': 'rd-meta' }, head);
      el('summary', null, det, 'Metadata · ' + meta.map(function (m) { return m[0]; }).slice(0, 6).join(', ') + (meta.length > 6 ? ', …' : ''));
      var dl = el('dl', null, det);
      meta.forEach(function (m) {
        if (m[0] === 'title') return;
        el('dt', null, dl, m[0]);
        linkValue(el('dd', null, dl), m[1]);
      });
      var pills = el('p', { 'class': 'rd-pills' }, head);
      meta.forEach(function (m) {
        if (['status', 'class', 'date', 'version', 'schema'].indexOf(m[0]) >= 0 && m[1]) el('span', { 'class': 'pill reports' }, pills, m[0] + ': ' + m[1]);
      });
    }
  }
  function scrollToFrag(frag) {
    var t = frag ? $('md-' + frag) : docBox;
    if (!t) t = docBox;
    var y = t.getBoundingClientRect().top + window.scrollY - 70;
    window.scrollTo({ top: y, behavior: reduce ? 'auto' : 'smooth' });
  }
  var seq = 0;
  function open(path, frag, opts) {
    var d = BY[path];
    if (!d) return false;
    opts = opts || {};
    var mine = ++seq;
    docBox.hidden = false;
    if (state.cur !== d) {
      clear(head); clear(side); clear(body);
      el('p', { 'class': 'lab-note' }, body, 'Načítám ' + d.p + ' …');
    }
    if (trail[trail.length - 1] !== path) trail.push(path);
    window.mjDocHash = docHref(path, frag).slice(1); window.mjDocUntil = Date.now() + 2500;
    try { history.replaceState(null, '', docHref(path, frag)); } catch (e) { /* sandboxed frame */ }
    Promise.all([corpus(), code(), load('md'), load('pure')]).then(function () {
      if (mine !== seq) return;
      if (state.cur !== d || !body.querySelector('.md-done')) {
        state.cur = d;
        var fm = frontmatter(texts[d.n]);
        var html = window.DOMPurify.sanitize(engine().render(fm.body), { ADD_ATTR: ['target'] });
        body.innerHTML = html;
        el('span', { 'class': 'md-done', hidden: '' }, body);
        var h1 = body.querySelector('h1');
        if (h1 && body.firstElementChild === h1) h1.remove();
        Array.prototype.forEach.call(body.querySelectorAll('table'), function (t) {
          var w = el('div', { 'class': 'table-wrap' });
          t.parentNode.insertBefore(w, t); w.appendChild(t);
        });
        rewriteLinks(d);
        var toc = outline();
        renderHead(d, fm.meta);
        renderSide(d, toc);
        mermaids();
        renderList();
        if (chart && state.view !== 'list') focusChart(d);
      }
      window.mjDocUntil = Date.now() + 1500;
      if (!opts.quiet) requestAnimationFrame(function () { scrollToFrag(frag); });
    }, function (err) {
      if (mine !== seq) return;
      clear(body);
      el('p', { 'class': 'lab-note' }, body, 'Dokument se nepodařilo vykreslit (' + err.message + '). ');
      el('a', { href: ghHref(d.p), 'class': 'md-gh' }, body, 'Otevřít na GitHubu');
    });
    return true;
  }
  window.mjDoc = function (spec, opts) {
    var parts = String(spec).split('::');
    return open(parts[0], parts[1], opts);
  };
  window.mjDocs = { index: IDX, chapters: chapters };

  /* clicks on document links anywhere on the page */
  document.addEventListener('click', function (e) {
    var a = e.target.closest ? e.target.closest('a') : null;
    if (!a) return;
    var h = a.getAttribute('href') || '';
    if (h.indexOf('#doc.') !== 0) return;
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button) return;
    e.preventDefault(); e.stopPropagation();
    window.mjDoc(h.slice(5));
  }, true);
  window.addEventListener('hashchange', function () {
    if (location.hash.indexOf('#doc.') === 0) window.mjDoc(decodeURIComponent(location.hash.slice(5)));
  });
  if (location.hash.indexOf('#doc.') === 0) setTimeout(function () { window.mjDoc(decodeURIComponent(location.hash.slice(5))); }, 120);

  /* ---------- charts ---------- */
  var chart = null, chartView = null;
  function focusChart(d) {
    if (!chart || chartView !== 'graph') return;
    chart.dispatchAction({ type: 'downplay', seriesIndex: 0 });
    chart.dispatchAction({ type: 'highlight', seriesIndex: 0, name: d.p });
  }
  function drawChart(view) {
    var wrap = $('rd-chart-wrap'), box = $('rd-chart'), note = $('rd-chart-note');
    wrap.hidden = view === 'list';
    if (view === 'list') return;
    note.textContent = 'Načítám graf …';
    load('echarts').then(function () {
      if (chart) { chart.dispose(); chart = null; }
      chart = window.echarts.init(box, null, { renderer: 'canvas' });
      chartView = view;
      var pal = palette(), fg = paint('--fg'), muted = paint('--muted'), panel = paint('--panel'), line = paint('--line');
      var tip = { backgroundColor: panel, borderColor: line, textStyle: { color: fg, fontSize: 13 }, confine: true };
      if (view === 'graph') {
        var cats = KINDS.map(function (k) { return { name: k[1] }; }), sel = {};
        KINDS.forEach(function (k) { sel[k[1]] = k[0] === 'doc' || k[0] === 'adr' || k[0] === 'guide'; });
        var kindIndex = {}; KINDS.forEach(function (k, i) { kindIndex[k[0]] = i; });
        var links = [];
        DOCS.forEach(function (d) { d.o.forEach(function (n) { links.push({ source: d.p, target: DOCS[n].p }); }); });
        chart.setOption({
          color: pal, tooltip: Object.assign({
            formatter: function (p) {
              if (p.dataType !== 'node') return '';
              var d = BY[p.name];
              return '<b>' + d.t.replace(/</g, '&lt;') + '</b><br>' + d.p + '<br>→ ' + d.o.length + ' · ← ' + d.i.length;
            }
          }, tip),
          legend: { data: cats.map(function (c) { return c.name; }), selected: sel, textStyle: { color: fg }, top: 0, type: 'scroll' },
          series: [{
            type: 'graph', layout: 'force', roam: true, draggable: true, top: 40,
            categories: cats, animation: !reduce,
            force: { repulsion: 60, edgeLength: [20, 90], gravity: 0.12, friction: 0.25 },
            label: { show: true, position: 'right', color: fg, fontSize: 10, formatter: function (p) { return p.data.big ? p.data.short : ''; } },
            labelLayout: { hideOverlap: true },
            lineStyle: { color: muted, opacity: 0.25, width: 1, curveness: 0.1 },
            emphasis: { focus: 'adjacency', lineStyle: { width: 2, opacity: 0.9 }, label: { show: true } },
            itemStyle: { borderColor: panel, borderWidth: 1 },
            data: DOCS.map(function (d) {
              return { name: d.p, category: kindIndex[d.k], symbolSize: 6 + Math.min(26, Math.sqrt(d.i.length) * 5),
                       big: d.i.length >= 6, short: d.p.split('/').pop().replace(/\.md$/, '').replace(/^(\d{4})-.*/, 'ADR $1') };
            }),
            links: links
          }]
        });
        note.textContent = 'Uzel je dokument, velikost je počet odkazů, které na něj vedou. Klik otevře dokument, najetí zvýrazní jeho sousedy, legenda zapíná a vypíná druhy. Pravidla, garance a use cases jsou zpočátku vypnuté.';
      } else {
        chart.setOption({
          color: pal, tooltip: Object.assign({
            formatter: function (p) { return '<b>' + String(p.data.title || p.name).replace(/</g, '&lt;') + '</b><br>' + (p.data.path || '') + '<br>' + Math.round(p.value / 1024) + ' KiB'; }
          }, tip),
          series: [{
            type: 'treemap', roam: false, nodeClick: false, breadcrumb: { show: false }, top: 4, bottom: 4, left: 4, right: 4,
            label: { show: true, formatter: '{b}', fontSize: 11, color: paint('--mj-on-accent'), overflow: 'truncate' },
            upperLabel: { show: true, height: 22, color: fg, fontWeight: 600 },
            itemStyle: { borderColor: panel, borderWidth: 1, gapWidth: 2 },
            levels: [{ itemStyle: { borderWidth: 0, gapWidth: 4 } }, { itemStyle: { gapWidth: 2 }, upperLabel: { show: true } }, { colorSaturation: [0.35, 0.6], itemStyle: { gapWidth: 1, borderColorSaturation: 0.6 } }],
            data: KINDS.map(function (k, i) {
              var kids = DOCS.filter(function (d) { return d.k === k[0]; }).map(function (d) {
                return { name: d.num ? 'ADR ' + d.num : d.p.split('/').pop().replace(/\.md$/, '').replace(/\.v\d+$/, ''), value: d.b, path: d.p, title: d.t };
              });
              return { name: k[1], itemStyle: { color: pal[i] }, children: kids, value: kids.reduce(function (s, c) { return s + c.value; }, 0) };
            })
          }]
        });
        note.textContent = 'Plocha je velikost souboru. Klik na dlaždici otevře dokument.';
      }
      chart.on('click', function (p) {
        var path = view === 'graph' ? (p.dataType === 'node' ? p.name : null) : (p.data && p.data.path);
        if (path) window.mjDoc(path);
      });
      if (state.cur) focusChart(state.cur);
    }, function (err) { note.textContent = 'Graf se nepodařilo načíst: ' + err.message; });
  }
  Array.prototype.forEach.call($('rd-views').querySelectorAll('button'), function (b) {
    b.addEventListener('click', function () {
      Array.prototype.forEach.call($('rd-views').querySelectorAll('button'), function (o) { o.setAttribute('aria-selected', String(o === b)); });
      state.view = b.dataset.v;
      drawChart(state.view);
    });
  });
  window.addEventListener('resize', function () { if (chart) chart.resize(); });
  matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function () { if (state.view !== 'list') drawChart(state.view); });

  kindButtons();
  renderList();
})();

(function () {
  'use strict';
  var ARTIFACT = 'https://claude.ai/artifact/XgCzwV1MFNDL1ssJHstPQ1';
  function $(id) { return document.getElementById(id); }
  function el(tag, attrs, parent, text) {
    var e = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (text !== undefined) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  function clear(e) { while (e.firstChild) e.removeChild(e.firstChild); }
  function line(t, text, cls) { return el('div', { 'class': 'ln' + (cls ? ' ' + cls : '') }, t, text); }

  /* ---------- anchors: every state gets a plain #token that can be bookmarked ---------- */
  function token(s) {
    return String(s || '').toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').trim()
      .replace(/\s+/g, '~').replace(/[^a-z0-9._~-]/g, '');
  }
  var lastHash = '';
  function setHash(t) {
    if (!t || t === lastHash) return;
    lastHash = t;
    try { history.replaceState(null, '', '#' + t); } catch (e) { /* sandboxed frame */ }
  }
  function copyLink(t, btn) {
    var url = ARTIFACT + '#' + t, old = btn.textContent;
    function ok() { btn.textContent = 'odkaz zkopírován'; setTimeout(function () { btn.textContent = old; }, 1500); }
    function fail() { btn.textContent = url; }
    try { navigator.clipboard.writeText(url).then(ok, fail); } catch (e) { fail(); }
  }
  function route(hash) {
    var h = (hash || '').replace(/^#/, '');
    if (!h) return false;
    var p = h.split('.');
    if (p[0] === 'g' && window.mjGraph) { lastHash = h; window.mjGraph('#' + h); return true; }
    if (p[0] === 'doc' && window.mjDoc) { lastHash = h; return window.mjDoc(decodeURIComponent(h.slice(4))); }
    if (p[0] === 'ex' && window.mjExplore) { rawExplore(p[1] || 'adr', (p.slice(2).join('.') || '').replace(/~/g, ' ')); lastHash = h; return true; }
    if (p[0] === 'arch' && window.mjArch) { rawArch(+p[1] || 0, p[2] === undefined ? undefined : +p[2]); lastHash = h; return true; }
    if (p[0] === 'q' && window.mjSearch) { window.mjSearch(p.slice(1).join('.').replace(/~/g, ' ')); lastHash = h; return true; }
    var t = $(h);
    if (t) { if (window.mjJump) window.mjJump(h); else t.scrollIntoView(); lastHash = h; return true; }
    return false;
  }
  var rawExplore = window.mjExplore, rawArch = window.mjArch, rawJump = window.mjJump;
  var exState = 'ex.adr', archState = 'arch.0';
  if (rawExplore) window.mjExplore = function (tab, query, facet) {
    rawExplore(tab, query, facet);
    exState = 'ex.' + tab + (query ? '.' + token(query) : '');
    setHash(exState);
  };
  if (rawArch) window.mjArch = function (l, i) {
    rawArch(l, i);
    archState = 'arch.' + l + (i === undefined || i < 0 ? '' : '.' + i);
    setHash(archState);
  };
  if (rawJump) window.mjJump = function (id) { rawJump(id); setHash(id); };

  /* explorer and architecture: follow their own controls, offer a copyable link */
  var exHost = $('lab-explore');
  if (exHost) {
    var exBtn = el('button', { type: 'button', 'class': 'btn ghost small link-b' }, exHost, 'Kopírovat odkaz na tento pohled');
    exBtn.addEventListener('click', function () { copyLink(exState, exBtn); });
    var q = $('ex-q'), tabs = $('ex-tabs');
    function sync() {
      var cur = tabs && tabs.querySelector('[aria-selected="true"]');
      exState = 'ex.' + (cur ? cur.dataset.k : 'adr') + (q && q.value.trim() ? '.' + token(q.value) : '');
      setHash(exState);
    }
    if (q) q.addEventListener('input', sync);
    if (tabs) tabs.addEventListener('click', function () { setTimeout(sync, 0); });
  }
  var archHost = $('lab-arch');
  if (archHost) {
    var arBtn = el('button', { type: 'button', 'class': 'btn ghost small link-b' }, archHost, 'Kopírovat odkaz na tuto komponentu');
    arBtn.addEventListener('click', function () { copyLink(archState, arBtn); });
    archHost.addEventListener('click', function (e) {
      if (e.target === arBtn) return;
      setTimeout(function () {
        var L = Array.prototype.findIndex.call(document.querySelectorAll('#arch-l .arch-b'), function (b) { return b.getAttribute('aria-selected') === 'true'; });
        var I = Array.prototype.findIndex.call(document.querySelectorAll('#arch-i .arch-b'), function (b) { return b.getAttribute('aria-selected') === 'true'; });
        if (L < 0) return;
        archState = 'arch.' + L + (I < 0 ? '' : '.' + I);
        setHash(archState);
      }, 0);
    });
  }

  /* sub-headings and labs get their own anchors */
  document.querySelectorAll('main section[id] h3:not(.h4)').forEach(function (h) {
    if (!h.id || h.querySelector('.h-anchor')) return;
    el('a', { 'class': 'h-anchor h3a', href: '#' + h.id, 'aria-label': 'Odkaz na oddíl' }, h, '#').addEventListener('click', function (e) {
      e.preventDefault(); if (window.mjJump) window.mjJump(h.id);
    });
  });
  document.querySelectorAll('main section[id] .lab').forEach(function (lab) {
    var head = lab.querySelector('.lab-h');
    if (!lab.id || !head || head.querySelector('.h-anchor')) return;
    var a = el('a', { 'class': 'h-anchor lab-a', href: '#' + lab.id, 'aria-label': 'Odkaz na interaktivní blok' }, head, '#');
    a.addEventListener('click', function (e) { e.preventDefault(); setHash(lab.id); copyLink(lab.id, a); });
  });
  document.querySelectorAll('main section[id] > h2 .h-anchor').forEach(function (a) {
    a.addEventListener('click', function (e) { if (e.metaKey || e.ctrlKey || e.shiftKey) return; e.preventDefault(); if (window.mjJump) window.mjJump(a.getAttribute('href').split('#').pop()); });
  });

  /* the context bar copies the link of the chapter being read */
  var bar = document.querySelector('.ctx-bar');
  if (bar) {
    var cb = el('button', { type: 'button', 'class': 'ctx-b', 'aria-label': 'Kopírovat odkaz na tuto kapitolu' }, null, '# odkaz');
    bar.insertBefore(cb, bar.querySelector('.ctx-b:last-child'));
    cb.addEventListener('click', function () {
      var c = bar.querySelector('.ctx-c');
      copyLink(c ? c.getAttribute('href').split('#').pop() : '', cb);
    });
  }

  /* the address follows the chapter in view, so a bookmark lands where the reader was */
  var secs = Array.prototype.slice.call(document.querySelectorAll('main section[id]'));
  var tick = false, quiet = 0;
  window.addEventListener('scroll', function () {
    if (tick) return;
    tick = true;
    requestAnimationFrame(function () {
      tick = false;
      if (Date.now() < quiet) return;
      var best = null;
      secs.forEach(function (s) { if (s.getBoundingClientRect().top <= 100) best = s; });
      if (!best) return;
      var inLab = lastHash.indexOf('ex.') === 0 || lastHash.indexOf('arch.') === 0;
      var labSec = inLab ? (lastHash.indexOf('ex.') === 0 ? 'pruzkumnik' : 'stavba') : null;
      if (labSec && best.id === labSec) return;
      if (window.mjDocUntil && Date.now() < window.mjDocUntil) return;
      if (best.id === 'citarna' && window.mjDocHash) { setHash(window.mjDocHash); return; }
      if (best.id === 'graf' && window.mjGraphHash) { setHash(window.mjGraphHash); return; }
      setHash(best.id);
    });
  }, { passive: true });
  window.addEventListener('hashchange', function () { quiet = Date.now() + 800; route(location.hash); });
  if (location.hash) setTimeout(function () { quiet = Date.now() + 1200; route(location.hash); }, 60);
  window.mjRoute = route;

  /* Internal links stay plain #anchors: the artifact frame treats an absolute address as an
     external link and takes the click away. A click on any of them is routed here, so runtime
     ids (sub-headings, labs, explorer states) resolve too. Bookmarkable addresses come from
     the copy-link controls. */
  document.addEventListener('click', function (e) {
    var a = e.target.closest ? e.target.closest('a') : null;
    if (!a) return;
    var h = a.getAttribute('href') || '';
    if (h.charAt(0) !== '#' || h.length < 2) return;
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button) return;
    if (e.defaultPrevented) return;
    e.preventDefault();
    quiet = Date.now() + 800;
    route(h);
  });

  /* ---------- personal mode generator ---------- */
  if ($('lab-osobni')) (function () {
    var out = $('st-out'), host = $('st-checks'), msg = $('st-msg');
    var defs = [
      ['st-claude', 'Používám Claude Code', true],
      ['st-own', 'Repo už má vlastní CLAUDE.md', false],
      ['st-tracked', 'Tým trackuje nastavení Claude Code v repu', false],
      ['st-push', 'Chci, aby pre-push hlídal finish contract', true],
      ['st-share', 'Sdílím obrazovku nebo nahrávám terminál', false],
      ['st-backup', 'Chci zálohu vrstvy mimo repo', true]
    ];
    var cb = {};
    defs.forEach(function (d) {
      var lab = el('label', { 'class': 'chk' }, host);
      var c = el('input', { type: 'checkbox', id: d[0] }, lab);
      c.checked = d[2];
      lab.appendChild(document.createTextNode(' ' + d[1]));
      c.addEventListener('change', render);
      cb[d[0]] = c;
    });
    var plain = '';
    function render() {
      clear(out);
      var L = [];
      function add(t, cls) { L.push([t, cls]); }
      var target = cb['st-claude'].checked ? 'CLAUDE.local.md' : '.ai/local/AGENTS.md';
      add('# 1. vrstva bez stopy v gitu', 'mut');
      add('majordomus init');
      add('git checkout .gitignore');
      add("printf '.ai/\\n" + (cb['st-claude'].checked ? 'CLAUDE.local.md\\n' : '') + "' >> .git/info/exclude");
      add('# 2. v .ai/repo/policy.yaml: jedna projekce, jedno vynucení', 'mut');
      add('#    projections: provider ' + (cb['st-claude'].checked ? 'claude-code' : 'agents') + ', target ' + target + ', always_loaded: true', 'mut');
      if (!cb['st-claude'].checked) add('#    neověřeno: instrukce z ' + target + ' musíš agentovi podat sám', 'warn');
      if (cb['st-own'].checked) add('#    týmový CLAUDE.md zůstává beze změny; soukromý soubor se načte vedle něj', 'mut');
      add('majordomus update');
      if (cb['st-push'].checked) {
        add('# 3. lokální hook', 'mut');
        add("printf '#!/bin/sh\\nmajordomus finish --check || exit $?\\n' > .git/hooks/pre-push && chmod +x .git/hooks/pre-push");
      } else add('# 3. bez hooku: doctor bude hlásit chybějící wiring finish-on-push', 'warn');
      if (cb['st-claude'].checked) {
        add('# 4. hooky session (neověřeno)', 'mut');
        if (cb['st-tracked'].checked) add('#    capture install NEPOUŠTĚJ: změnil by trackovaný soubor. Hooky dej do osobního settings.local.json.', 'warn');
        else { add('majordomus capture install --provider claude-code'); add("printf '.claude/hooks/\\n' >> .git/info/exclude   # a stejně vyluč soubor nastavení v .claude/", ''); }
      }
      add('# 5. server, který vidí vyloučenou vrstvu', 'mut');
      add((cb['st-share'].checked ? 'MAJORDOMUS_BANNER=off ' : '') + 'majordomus serve --discovery filesystem');
      add('#    http://127.0.0.1:8741/ui', 'mut');
      if (cb['st-backup'].checked) { add('# 6. záloha: git clean -x by vrstvu smazal', 'mut'); add('rsync -a --delete .ai/ ~/majordomus-backup/"$(basename "$PWD")"/'); }
      add('# kontrola', 'mut');
      add('git status --short            # prázdné nebo jen tvoje změny kódu');
      add('git check-ignore -v .ai/manifest.yaml');
      L.forEach(function (x) { line(out, x[0], x[1]); });
      plain = L.map(function (x) { return x[0]; }).join('\n');
    }
    $('st-copy').addEventListener('click', function () {
      try { navigator.clipboard.writeText(plain).then(function () { msg.textContent = 'Zkopírováno.'; }, function () { msg.textContent = 'Kopírování prohlížeč nepovolil, označ text ručně.'; }); }
      catch (e) { msg.textContent = 'Kopírování prohlížeč nepovolil, označ text ručně.'; }
    });
    render();
  })();
})();

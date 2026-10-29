/* The site's one search, in its two places: the dialog the navbar's trigger opens on every
   page (partials/site-search.html), and the documentation hub's inline field
   (partials/docs-hub.html). Both read /docs/index.json, which scripts/site-build writes from
   the pages the build published, and both match and rank with the one function below, so the
   hub and the dialog cannot disagree about what a query finds. Copied to static/js/search.js
   by scripts/site-build and loaded, deferred, by the base layout.

   The index is fetched once per page, on first use: the dialog's first open or the hub's
   first keystroke. Everything is an enhancement: the trigger is rendered `hidden` and revealed
   here, so a reader without the script is never offered a control that does nothing.

   Keyboard: `/` and Ctrl+K or Cmd+K open the dialog from anywhere but a field being typed in;
   Escape closes it and focus returns to whatever opened it; the arrows move the active result
   and Enter opens it. Nothing animates, so a reader who asked for reduced motion gets none,
   and the active result is scrolled to without smoothing when they did. */
(function () {
  'use strict';

  var LIMIT = 30;
  var IDLE = 'Searches the titles and descriptions of every page this site publishes.';
  var indexes = {};

  // One fetch per index URL per page, shared by every search on it.
  function load(url) {
    if (!indexes[url]) {
      indexes[url] = fetch(url).then(function (r) {
        if (!r.ok) { throw new Error(String(r.status)); }
        return r.json();
      }).then(function (ix) {
        return ix.pages.map(function (p) {
          return {
            page: p,
            title: String(p.title || '').toLowerCase(),
            text: [p.title, p.description, p.section, p.kind, p.id, p.route]
              .filter(Boolean).join(' ').toLowerCase()
          };
        });
      });
      indexes[url].catch(function () { delete indexes[url]; });
    }
    return indexes[url];
  }

  // Every word of the query is somewhere in the page's title, description, section, kind, id or
  // route. A page whose title holds the query, then whose title holds its words, ranks first;
  // between equals the shallower route wins, so a section's own page comes before its entries.
  function match(entries, query) {
    var words = query.toLowerCase().split(/\s+/).filter(Boolean);
    if (!words.length) { return []; }
    var phrase = words.join(' ');
    var hits = [];
    entries.forEach(function (e) {
      for (var i = 0; i < words.length; i++) {
        if (e.text.indexOf(words[i]) < 0) { return; }
      }
      var score = 0;
      if (e.title === phrase) { score += 8; }
      else if (e.title.indexOf(phrase) === 0) { score += 6; }
      else if (e.title.indexOf(phrase) > 0) { score += 4; }
      words.forEach(function (w) { if (e.title.indexOf(w) >= 0) { score += 1; } });
      hits.push({ score: score, depth: e.page.route.split('/').length, page: e.page });
    });
    hits.sort(function (a, b) {
      return b.score - a.score || a.depth - b.depth ||
        (a.page.route < b.page.route ? -1 : a.page.route > b.page.route ? 1 : 0);
    });
    return hits.map(function (h) { return h.page; });
  }

  // The site's root, from the index's own address: get_url wrote it with the deployment prefix.
  function baseOf(url) { return url.replace(/docs\/index\.json$/, ''); }
  function hrefOf(base, page) { return base + page.route.replace(/^\//, ''); }
  function metaOf(page) {
    var where = page.kind || page.section;
    return where ? where + ' · ' + page.route : page.route;
  }

  // A result, cloned from the <template> its search carries: the markup and its classes are the
  // template's, and only the text and the address are written here.
  function item(template, base, page) {
    var node = template.content.firstElementChild.cloneNode(true);
    var link = node.matches('a') ? node : node.querySelector('a');
    link.href = hrefOf(base, page);
    var fill = function (slot, text) {
      var el = node.querySelector('[data-slot="' + slot + '"]');
      if (el) { el.textContent = text; }
    };
    fill('title', page.title);
    fill('meta', metaOf(page));
    fill('description', page.description || '');
    return { node: node, link: link };
  }

  /* ------------------------------------------------------------- the documentation hub */
  // The hub's field, its status and its list keep the attributes the specs, `scripts/pages
  // verify-docs` and the kit's search field (which submits here as ?q=) read.
  function inline(root) {
    var input = root.querySelector('input');
    var status = root.querySelector('[data-docs-search-status]');
    var list = root.querySelector('[data-docs-search-results]');
    var template = root.querySelector('template[data-search-item]');
    var url = root.getAttribute('data-index');
    var base = baseOf(url);
    if (!input || !status || !list || !template || !url) { return; }
    function render(entries) {
      var q = input.value.trim();
      list.replaceChildren();
      if (!q) { status.textContent = IDLE; return; }
      var hits = match(entries, q);
      status.textContent = hits.length + ' page(s) match.';
      hits.slice(0, 50).forEach(function (p) { list.append(item(template, base, p).node); });
    }
    function run() {
      load(url).then(render, function () { status.textContent = 'The index could not be loaded.'; });
    }
    input.addEventListener('input', run);
    var asked = new URLSearchParams(window.location.search).get('q');
    if (asked) { input.value = asked; run(); }
  }

  /* ------------------------------------------------------------------- the dialog */
  function dialog(box) {
    var input = box.querySelector('#site-search-input');
    var status = box.querySelector('[data-site-search-status]');
    var list = box.querySelector('[data-site-search-results]');
    var template = box.querySelector('template[data-site-search-item]');
    var closer = box.querySelector('[data-site-search-close]');
    var triggers = Array.prototype.slice.call(document.querySelectorAll('[data-site-search-open]'));
    var url = box.getAttribute('data-index');
    var base = baseOf(url);
    if (!input || !status || !list || !template || !url || typeof box.showModal !== 'function') { return; }
    var opener = null;
    var options = [];
    var active = -1;
    var reduced = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;

    function expanded(open) {
      triggers.forEach(function (t) { t.setAttribute('aria-expanded', open ? 'true' : 'false'); });
    }
    function activate(i) {
      if (active >= 0 && options[active]) { options[active].setAttribute('aria-selected', 'false'); }
      active = options.length ? (i + options.length) % options.length : -1;
      if (active < 0) { input.removeAttribute('aria-activedescendant'); return; }
      var el = options[active];
      el.setAttribute('aria-selected', 'true');
      input.setAttribute('aria-activedescendant', el.id);
      el.scrollIntoView({ block: 'nearest', behavior: reduced ? 'auto' : 'smooth' });
    }
    function clear() {
      list.replaceChildren();
      options = [];
      active = -1;
      input.removeAttribute('aria-activedescendant');
      input.setAttribute('aria-expanded', 'false');
    }
    function render(entries) {
      var q = input.value.trim();
      clear();
      if (!q) { status.textContent = IDLE; return; }
      var hits = match(entries, q);
      if (!hits.length) { status.textContent = 'No page matches “' + q + '”.'; return; }
      status.textContent = hits.length > LIMIT
        ? hits.length + ' pages match; the first ' + LIMIT + ' are listed.'
        : hits.length + (hits.length === 1 ? ' page matches.' : ' pages match.');
      hits.slice(0, LIMIT).forEach(function (p, i) {
        var made = item(template, base, p);
        made.link.id = 'site-search-option-' + i;
        made.link.setAttribute('role', 'option');
        made.link.setAttribute('aria-selected', 'false');
        made.link.setAttribute('data-site-search-result', '');
        options.push(made.link);
        list.append(made.node);
      });
      input.setAttribute('aria-expanded', 'true');
      activate(0);
    }
    function run() {
      if (!input.value.trim()) { render([]); return; }
      if (!indexes[url]) { status.textContent = 'Loading the index…'; }
      load(url).then(function (entries) { if (box.open) { render(entries); } },
        function () { status.textContent = 'The index could not be loaded.'; });
    }
    function open(from) {
      if (box.open) { input.focus(); return; }
      opener = from || document.activeElement;
      box.showModal();
      expanded(true);
      input.focus();
      input.select();
      run();
    }
    // What closing owes: no options left in the page, the trigger saying it is collapsed, and
    // focus back where it was. Done as the dialog closes, not on its `close` event, which the
    // browser queues: a reader (or a test) acting in between would see a closed dialog whose
    // trigger still says it is open. The event still settles a close that came another way.
    function settle() {
      clear();
      expanded(false);
      var back = opener;
      opener = null;
      if (back && typeof back.focus === 'function' && document.contains(back)) { back.focus(); }
    }
    function close() {
      if (box.open) { box.close(); settle(); }
    }

    box.addEventListener('close', settle);
    // Escape is answered here rather than left to the browser: in a search field the browser's
    // own Escape clears the text first, and the dialog would then need a second press to close.
    box.addEventListener('cancel', function (event) { event.preventDefault(); close(); });
    box.addEventListener('keydown', function (event) {
      if (event.key === 'Escape') { event.preventDefault(); close(); return; }
      if (event.key === 'Tab') {
        // the modal dialog already makes the page behind it inert; this keeps Tab cycling
        // between the field and the close button instead of leaving for the browser's chrome
        var stops = [input, closer].filter(Boolean);
        var at = stops.indexOf(document.activeElement);
        var next = event.shiftKey ? (at <= 0 ? stops.length - 1 : at - 1) : (at + 1) % stops.length;
        event.preventDefault();
        stops[next].focus();
      }
    });
    input.addEventListener('keydown', function (event) {
      if (event.key === 'ArrowDown') { event.preventDefault(); activate(active + 1); }
      else if (event.key === 'ArrowUp') { event.preventDefault(); activate(active - 1); }
      else if (event.key === 'Enter') {
        if (active >= 0 && options[active]) { event.preventDefault(); window.location.assign(options[active].href); }
      }
    });
    input.addEventListener('input', run);
    if (closer) { closer.addEventListener('click', close); }
    // a press on the backdrop is a press on the dialog itself, whose content fills it
    box.addEventListener('click', function (event) { if (event.target === box) { close(); } });
    list.addEventListener('mousemove', function (event) {
      var el = event.target.closest && event.target.closest('[role="option"]');
      var i = el ? options.indexOf(el) : -1;
      if (i >= 0 && i !== active) {
        if (active >= 0 && options[active]) { options[active].setAttribute('aria-selected', 'false'); }
        active = i;
        el.setAttribute('aria-selected', 'true');
        input.setAttribute('aria-activedescendant', el.id);
      }
    });

    triggers.forEach(function (t) {
      t.hidden = false;
      t.addEventListener('click', function () { open(t); });
    });
    document.addEventListener('keydown', function (event) {
      if (event.defaultPrevented || event.isComposing || event.altKey) { return; }
      var slash = event.key === '/' && !event.ctrlKey && !event.metaKey;
      var k = (event.key === 'k' || event.key === 'K') && (event.ctrlKey || event.metaKey);
      if (!slash && !k) { return; }
      if (box.open) {
        if (k) { event.preventDefault(); input.focus(); input.select(); }
        return;
      }
      var t = event.target;
      var typing = t && (t.isContentEditable || /^(input|textarea|select)$/i.test(t.tagName || ''));
      if (typing) { return; }
      event.preventDefault();
      open(document.activeElement);
    });
  }

  function init() {
    document.querySelectorAll('[data-docs-search]').forEach(inline);
    var box = document.getElementById('site-search');
    if (box && box.hasAttribute('data-site-search')) { dialog(box); }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init, { once: true });
  } else {
    init();
  }
})();

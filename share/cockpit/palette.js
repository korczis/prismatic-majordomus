// The command palette. Every entry in it is something the registry, the graph derivations
// or the index already holds: the capabilities from /api/v1/capabilities, the graphs from
// /api/v1/graphs, the objects from /api/v1/objects, and the Cockpit's own areas read out
// of the page's own navigation. Nothing is listed here by hand, so a capability, a kind or
// a graph added to the backend is in the palette on the next load.
//
// Nothing is fetched until the palette is opened for the first time.

import { api } from './cockpit.js';

const results = document.querySelector('[x-ref="paletteResults"]');
const input = document.querySelector('[x-ref="paletteInput"]');
if (results && input) {
  install(results, input);
}

function install(list, field) {
  /** @type {{kind: string, label: string, detail: string, href: string}[]} */
  let entries = [];
  let shown = [];
  let active = 0;
  // two flags, not one: `started` keeps the fetch from running twice, `ready` says the
  // answers are in. One flag doing both told a reader who typed while the registry was
  // still being read that nothing matched.
  let started = false;
  let ready = false;

  // the navigation the server rendered is the source for the Cockpit's own pages: it is
  // already derived from the registry, and reading it back beats listing the pages twice
  const pages = Array.from(document.querySelectorAll('.mj-sidebar a.mj-nav-link')).map(
    (a) => ({
      kind: 'page',
      label: (a.querySelector('.mj-nav-label') || a).textContent.trim(),
      detail: a.getAttribute('href'),
      href: a.getAttribute('href'),
    }),
  );

  async function load() {
    if (started) return;
    started = true;
    entries = pages.slice();
    render();
    const [capabilities, graphs, objects] = await Promise.all([
      api('/api/v1/capabilities'),
      api('/api/v1/graphs'),
      api('/api/v1/objects'),
    ]);
    if (capabilities.ok && capabilities.body && capabilities.body.capabilities) {
      for (const c of capabilities.body.capabilities) {
        entries.push({
          kind: c.kind,
          label: c.id,
          detail: c.title || '',
          href: '/cockpit/capabilities/' + encodeURIComponent(c.id),
        });
      }
    }
    if (graphs.ok && graphs.body && graphs.body.graphs) {
      for (const g of graphs.body.graphs) {
        entries.push({
          kind: 'graph',
          label: g.id,
          detail: g.title || '',
          href: '/cockpit/graphs/' + encodeURIComponent(g.id),
        });
      }
    }
    if (objects.ok && objects.body && objects.body.objects) {
      for (const o of objects.body.objects) {
        entries.push({
          kind: o.kind,
          label: o.identity,
          detail: o.title || o.path || '',
          href: '/cockpit/object?uri=' + encodeURIComponent(o.uri),
        });
      }
    }
    ready = true;
    render();
  }

  function score(entry, needle) {
    const label = entry.label.toLowerCase();
    if (!needle) return entry.kind === 'page' ? 0 : 1;
    const at = label.indexOf(needle);
    if (at === 0) return 0;
    if (at > 0) return 1;
    if (entry.detail.toLowerCase().includes(needle)) return 2;
    return -1;
  }

  function render() {
    const needle = field.value.trim().toLowerCase();
    shown = entries
      .map((entry) => ({ entry, rank: score(entry, needle) }))
      .filter((r) => r.rank >= 0)
      .sort((a, b) => a.rank - b.rank || a.entry.label.length - b.entry.label.length)
      .slice(0, 40)
      .map((r) => r.entry);
    if (active >= shown.length) active = 0;

    list.dataset.state = ready ? 'ready' : started ? 'loading' : 'idle';
    list.textContent = '';
    for (let i = 0; i < shown.length; i += 1) {
      const entry = shown[i];
      const item = document.createElement('li');
      item.setAttribute('role', 'option');
      item.setAttribute('aria-selected', String(i === active));
      item.dataset.href = entry.href;

      const kind = document.createElement('span');
      kind.className = 'mj-palette-kind';
      kind.textContent = entry.kind;
      const label = document.createElement('span');
      label.className = 'mj-palette-label';
      label.textContent = entry.label;
      const detail = document.createElement('span');
      detail.className = 'mj-palette-detail';
      detail.textContent = entry.detail;

      item.append(kind, label, detail);
      item.addEventListener('click', () => go(entry.href));
      list.appendChild(item);
    }
    if (!shown.length) {
      const item = document.createElement('li');
      item.className = 'mj-palette-detail';
      item.textContent = ready ? 'Nothing matches.' : 'Reading the registry…';
      list.appendChild(item);
    }
  }

  function go(href) {
    if (href) window.location.assign(href);
  }

  window.addEventListener('mj:palette-open', () => {
    load();
    render();
  });
  window.addEventListener('mj:palette-query', render);
  window.addEventListener('mj:palette-move', (event) => {
    if (!shown.length) return;
    active = (active + event.detail + shown.length) % shown.length;
    render();
    const selected = list.children[active];
    if (selected && selected.scrollIntoView) selected.scrollIntoView({ block: 'nearest' });
  });
  window.addEventListener('mj:palette-choose', () => {
    if (shown[active]) go(shown[active].href);
  });
}

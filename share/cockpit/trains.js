// The trains viewer (ADR 0138). The trains come from /api/v1/pull-requests/trains, the
// projection of `integration.trains`; this file translates that answer into Cytoscape's
// shape at the boundary and nowhere else.
//
// It is drawn in the manner of the Neo4j Browser, because that is the picture the owner
// asked for: round nodes with their caption inside, coloured by what they are, labelled
// directed relationships between them, a force layout that can be dragged, zoomed and
// panned, the neighbours of the node under the pointer lit, and a click that opens every
// property of a node in a panel.
//
// Progressive enhancement in the strict sense: the page already lists every train as a
// table of its links with every fact this drawing shows. If Cytoscape is not vendored, or
// fails, the tables stay and nothing is lost.
//
// What this file decides, and what it does not:
//
//   the colours     are not decided here. A disposition's colour is one of the status or
//                   tone roles share/design/tokens.yaml declares, named below by its token;
//                   this file only decides which role a disposition wears. A node is filled
//                   with the role's ground and ringed with its strong colour, and its caption
//                   is the page's own text colour, which is the pairing the declaration
//                   measures for contrast.
//   the vocabulary  is the capability's: dispositions, relation words and prepared-head
//                   states are shown as the answer spells them.
//   the refresh     is a poll of the same HTTP route, every `data-mj-trains-poll` seconds,
//                   paused while the tab is hidden. No stream is invented for it.

import { api, vendor, explainMissing, motionAllowed } from './cockpit.js';

/**
 * Which declared role each disposition wears: `fill` is the ground inside the circle and
 * `ring` the strong colour around it. `landed`, `closed_unmerged` and `unread` are the
 * states of a pull request beneath a train, which is no longer open.
 */
const ROLES = {
  ready: { fill: '--mj-ok-bg', ring: '--mj-ok' },
  waiting_for_checks: { fill: '--mj-info-bg', ring: '--mj-info' },
  needs_refresh: { fill: '--mj-tone-cyan-bg', ring: '--mj-tone-cyan' },
  waiting_for_review: { fill: '--mj-tone-blue-bg', ring: '--mj-tone-blue' },
  waiting_for_dependency: { fill: '--mj-tone-blue-bg', ring: '--mj-tone-blue' },
  draft: { fill: '--mj-neutral-bg', ring: '--mj-neutral' },
  needs_repair: { fill: '--mj-bad-bg', ring: '--mj-bad' },
  conflicting: { fill: '--mj-tone-rose-bg', ring: '--mj-tone-rose' },
  blocked: { fill: '--mj-tone-orange-bg', ring: '--mj-tone-orange' },
  unsafe: { fill: '--mj-tone-orange-bg', ring: '--mj-tone-orange' },
  other_base: { fill: '--mj-tone-orange-bg', ring: '--mj-tone-orange' },
  unknown: { fill: '--mj-neutral-bg', ring: '--mj-neutral' },
  redundant: { fill: '--mj-warn-bg', ring: '--mj-warn' },
  superseded: { fill: '--mj-warn-bg', ring: '--mj-warn' },
  possibly_redundant: { fill: '--mj-warn-bg', ring: '--mj-warn' },
  obsolete: { fill: '--mj-warn-bg', ring: '--mj-warn' },
  landed: { fill: '--mj-tone-violet-bg', ring: '--mj-tone-violet' },
  closed_unmerged: { fill: '--mj-neutral-bg', ring: '--mj-neutral' },
  unread: { fill: '--mj-neutral-bg', ring: '--mj-neutral' },
};
const MASTER_ROLE = { fill: '--mj-accent-soft', ring: '--mj-accent' };

/** A title in a circle is a caption, not a sentence: this many characters, then an ellipsis. */
const CAPTION_CHARS = 16;

const frame = document.querySelector('[data-mj-trains]');
if (frame) {
  start(frame).catch((error) => explainMissing(frame, error));
}

async function start(frame) {
  const [cytoscape, first] = await Promise.all([
    vendor('cytoscape.min.js', 'cytoscape'),
    api(frame.dataset.mjTrainsSrc),
  ]);
  if (!first.ok) throw new Error('the trains endpoint answered ' + first.status);

  const colours = drawable();
  const type = typeScale();
  const state = {
    view: first.body,
    text: JSON.stringify(first.body),
    links: new Map(),
    order: [],
    train: '',
    hidden: new Set(),
    needle: '',
    selected: '',
    layout: 'force',
  };

  const cy = cytoscape({
    container: frame,
    elements: elements(state),
    layout: { name: 'preset' },
    minZoom: 0.15,
    maxZoom: 3,
    style: style(colours, type),
  });
  runLayout(cy, state, true);

  // ------------------------------------------------------------------- the pointer
  cy.on('mouseover', 'node', (event) => {
    if (state.selected) return;
    cy.elements().addClass('mj-dim');
    event.target.closedNeighborhood().removeClass('mj-dim').addClass('mj-near');
  });
  cy.on('mouseout', 'node', () => {
    if (state.selected) return;
    cy.elements().removeClass('mj-dim').removeClass('mj-near');
  });
  cy.on('tap', 'node', (event) => select(cy, state, event.target.id(), false));
  cy.on('dbltap', 'node', (event) => open(state, event.target.id()));
  cy.on('tap', (event) => {
    if (event.target === cy) clear(cy, state);
  });

  // ------------------------------------------------------------------ the keyboard
  frame.addEventListener('keydown', (event) => {
    const key = event.key;
    const visible = state.order.filter((id) => cy.getElementById(id).visible());
    if (key === 'ArrowRight' || key === 'ArrowDown' || key === 'ArrowLeft' || key === 'ArrowUp') {
      if (!visible.length) return;
      const step = key === 'ArrowRight' || key === 'ArrowDown' ? 1 : -1;
      const at = visible.indexOf(state.selected);
      const next = at < 0 ? (step > 0 ? 0 : visible.length - 1) : (at + step + visible.length) % visible.length;
      select(cy, state, visible[next], true);
    } else if (key === 'Enter') {
      if (state.selected) open(state, state.selected);
    } else if (key === '+' || key === '=') {
      zoom(cy, 1.25);
    } else if (key === '-' || key === '_') {
      zoom(cy, 0.8);
    } else if (key === '0') {
      fit(cy);
    } else if (key === 'Escape') {
      clear(cy, state);
    } else {
      return;
    }
    event.preventDefault();
  });

  // ------------------------------------------------------------------ the controls
  const on = (selector, kind, handler) => {
    const element = document.querySelector(selector);
    if (element) element.addEventListener(kind, handler);
    return element;
  };
  on('[data-mj-trains-zoom-in]', 'click', () => zoom(cy, 1.25));
  on('[data-mj-trains-zoom-out]', 'click', () => zoom(cy, 0.8));
  on('[data-mj-trains-fit]', 'click', () => fit(cy));
  const layoutButton = on('[data-mj-trains-layout]', 'click', () => {
    state.layout = state.layout === 'force' ? 'layered' : 'force';
    layoutButton.textContent = state.layout === 'force' ? 'Layered' : 'Force';
    runLayout(cy, state, true);
  });
  on('[data-mj-trains-train]', 'change', (event) => {
    state.train = event.target.value;
    applyFilters(cy, state);
    fit(cy);
  });
  const search = on('[data-mj-trains-search]', 'input', (event) => {
    state.needle = event.target.value.trim().toLowerCase();
    // a search replaces the selection: what is lit is what matches
    state.selected = '';
    cy.elements().removeClass('mj-focus').removeClass('mj-near');
    applyFilters(cy, state);
    const matching = cy.nodes('.mj-match');
    if (matching.length) fitTo(cy, matching, 48);
  });
  if (search) {
    search.addEventListener('keydown', (event) => {
      if (event.key !== 'Enter') return;
      const first = cy.nodes('.mj-match').first();
      if (first.length) {
        select(cy, state, first.id(), true);
        frame.focus();
      }
      event.preventDefault();
    });
  }

  const legendHost = legend(frame, cy, state);

  // --------------------------------------------------------------------- the poll
  const asked = document.querySelector('[data-mj-trains-asked]');
  const observed = document.querySelector('[data-mj-trains-observed]');
  const seconds = Math.max(5, parseInt(frame.dataset.mjTrainsPoll || '30', 10) || 30);
  const say = (message) => {
    if (asked) asked.textContent = message;
  };
  // one ask at a time: each answer decides every link's relation to master again, and a
  // second ask started while the first is out would only queue behind it on the server
  let asking = false;
  const poll = async () => {
    if (asking) return;
    asking = true;
    say('asking now …');
    let answer;
    try {
      answer = await api(frame.dataset.mjTrainsSrc);
    } catch (error) {
      say('the last ask failed (' + error.message + '); asking again in ' + seconds + ' s');
      return;
    } finally {
      asking = false;
    }
    const time = new Date().toLocaleTimeString();
    if (!answer.ok) {
      say('the last ask, at ' + time + ', was answered ' + answer.status + '; asking again in ' + seconds + ' s');
      return;
    }
    say('at ' + time + ', and again every ' + seconds + ' s while this tab is visible');
    if (observed && answer.body.observed_at) observed.textContent = answer.body.observed_at;
    const text = JSON.stringify(answer.body);
    if (text === state.text) return;
    state.view = answer.body;
    state.text = text;
    replace(cy, state);
    legend(frame, cy, state, legendHost);
  };
  on('[data-mj-trains-refresh]', 'click', poll);
  say('when this page was rendered; asking again every ' + seconds + ' s while this tab is visible');
  setInterval(() => {
    if (document.visibilityState === 'visible') poll();
  }, seconds * 1000);

  // What the drawing holds, for a probe that cannot read a canvas's meaning.
  frame.mjTrains = {
    cy,
    nodes: () => cy.nodes().map((n) => n.id()),
    select: (id) => select(cy, state, id, true),
    selected: () => state.selected,
    poll,
  };
  frame.dataset.mjTrainsReady = 'true';
}

// ------------------------------------------------------------------------ the answer

/** The one translation of the capability's answer into the drawing library's elements. */
function elements(state) {
  const view = state.view || {};
  const out = [];
  state.links = new Map();
  state.order = [];
  if (!view.observed) return out;
  const sha = (view.master_sha || '').slice(0, 7);
  out.push({
    data: { id: 'master', kind: 'master', caption: (view.base || 'master') + '\n' + sha, trains: [] },
  });
  state.links.set('master', { kind: 'master', view });
  state.order.push('master');
  const beneath = new Map();
  for (const train of view.trains || []) {
    for (const link of train.links) {
      const id = 'pr-' + link.number;
      state.links.set(id, { kind: 'link', link, train });
      state.order.push(id);
      out.push({
        data: {
          id,
          kind: 'link',
          number: link.number,
          disposition: link.disposition,
          caption: '#' + link.number + '\n' + caption(link.title),
          haystack: ('#' + link.number + ' ' + link.title + ' ' + link.head_ref).toLowerCase(),
          trains: [train.id],
          turn: link.at_turn ? 'yes' : 'no',
        },
      });
      if (!link.on.length) {
        out.push({
          data: { id: 'targets-' + link.number, source: id, target: 'master', label: 'TARGETS', kind: 'targets' },
        });
      }
    }
    for (const b of train.beneath) {
      const id = 'pr-' + b.number;
      const known = beneath.get(id);
      if (known) {
        known.data.trains.push(train.id);
        continue;
      }
      const fate = b.state === 'merged' ? 'landed' : b.state;
      const node = {
        data: {
          id,
          kind: 'beneath',
          number: b.number,
          disposition: fate,
          caption: '#' + b.number + '\n' + fate.replace('_', ' '),
          haystack: ('#' + b.number).toLowerCase(),
          trains: [train.id],
          turn: 'no',
        },
      };
      beneath.set(id, node);
      state.links.set(id, { kind: 'beneath', beneath: b, train });
    }
    for (const e of train.edges) {
      out.push({
        data: {
          id: 'edge-' + e.from + '-' + e.to,
          source: 'pr-' + e.from,
          target: 'pr-' + e.to,
          label: e.kind === 'stacked_on' ? 'STACKED_ON' : 'DEPENDS_ON',
          kind: e.kind,
          satisfied: e.satisfied ? 'yes' : 'no',
        },
      });
    }
  }
  for (const [id, node] of beneath) {
    out.push(node);
    state.order.push(id);
    if (node.data.disposition === 'landed') {
      out.push({
        data: { id: 'merged-' + node.data.number, source: id, target: 'master', label: 'MERGED_INTO', kind: 'merged' },
      });
    }
  }
  return out;
}

/** A title cut to a caption. */
function caption(title) {
  const words = String(title || '').replace(/^[a-z]+(\([^)]*\))?:\s*/, '');
  return words.length > CAPTION_CHARS ? words.slice(0, CAPTION_CHARS - 1) + '…' : words;
}

/** Put a new answer in place of the old one, keeping where the reader left every node. */
function replace(cy, state) {
  const positions = new Map();
  cy.nodes().forEach((n) => positions.set(n.id(), { ...n.position() }));
  const next = elements(state);
  cy.batch(() => {
    cy.elements().remove();
    cy.add(next);
  });
  let unplaced = 0;
  cy.nodes().forEach((n) => {
    const at = positions.get(n.id());
    if (at) n.position(at);
    else unplaced += 1;
  });
  if (unplaced) runLayout(cy, state, false);
  applyFilters(cy, state);
  if (state.selected && cy.getElementById(state.selected).length) {
    select(cy, state, state.selected, false);
  } else {
    clear(cy, state);
  }
}

// ------------------------------------------------------------------------ the layout

/**
 * The force layout is Cytoscape's own `cose`, the one built-in layout that behaves like the
 * Neo4j Browser's: linked nodes pull together, unlinked ones push apart, and each train
 * settles as its own cluster around master. `layered` is `breadthfirst` from master, for a
 * reader who wants the landing order as rows. Animation only for a reader who wants motion.
 */
function runLayout(cy, state, fitAfter) {
  const animate = motionAllowed();
  const common = { animate, animationDuration: animate ? 450 : 0, fit: fitAfter, padding: 32 };
  const options =
    state.layout === 'layered'
      ? { ...common, name: 'breadthfirst', directed: false, roots: '#master', spacingFactor: 1.25 }
      : {
          ...common,
          name: 'cose',
          randomize: false,
          numIter: 1200,
          nodeRepulsion: () => 14000,
          idealEdgeLength: () => 96,
          edgeElasticity: () => 80,
          gravity: 0.3,
          componentSpacing: 80,
          nodeOverlap: 12,
        };
  cy.elements(':visible').layout(options).run();
}

function zoom(cy, factor) {
  const level = Math.min(cy.maxZoom(), Math.max(cy.minZoom(), cy.zoom() * factor));
  cy.zoom({ level, renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 } });
}

function fit(cy) {
  fitTo(cy, cy.elements(':visible'), 32);
}

/** Fit these elements, ending any viewport animation still running so the last ask wins. */
function fitTo(cy, eles, padding) {
  if (!eles.length) return;
  cy.stop(true, false);
  if (motionAllowed()) cy.animate({ fit: { eles, padding }, duration: 300 });
  else cy.fit(eles, padding);
}

// ------------------------------------------------------------------- filter and find

/** The train chosen, the dispositions hidden and the search, applied together. */
function applyFilters(cy, state) {
  cy.batch(() => {
    cy.nodes().forEach((n) => {
      const kind = n.data('kind');
      const inTrain = !state.train || kind === 'master' || (n.data('trains') || []).includes(state.train);
      const shown = inTrain && (kind === 'master' || !state.hidden.has(n.data('disposition')));
      n.toggleClass('mj-hidden', !shown);
      const match = Boolean(state.needle) && shown && (n.data('haystack') || '').includes(state.needle);
      n.toggleClass('mj-match', match);
    });
    if (state.needle) {
      cy.elements().addClass('mj-dim');
      const matching = cy.nodes('.mj-match');
      matching.removeClass('mj-dim');
      matching.connectedEdges().removeClass('mj-dim');
    } else if (!state.selected) {
      cy.elements().removeClass('mj-dim');
    }
  });
}

/**
 * The legend, which is also the filter: one entry per disposition present, with its colour
 * and count, and a press that takes it out of the picture. Rebuilt in place on every answer.
 */
function legend(frame, cy, state, existing) {
  const host = existing || document.createElement('div');
  host.className = 'mj-graph-legend';
  host.setAttribute('data-mj-trains-legend', '');
  host.setAttribute('aria-label', 'Dispositions: press one to hide or show it');
  host.textContent = '';
  const counts = new Map();
  cy.nodes().forEach((n) => {
    const d = n.data('disposition');
    if (d) counts.set(d, (counts.get(d) || 0) + 1);
  });
  const words = [...counts.keys()].sort((a, b) => counts.get(b) - counts.get(a) || (a < b ? -1 : 1));
  for (const word of words) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'mj-graph-legend-entry';
    button.setAttribute('aria-pressed', state.hidden.has(word) ? 'true' : 'false');
    button.classList.toggle('is-hidden', state.hidden.has(word));
    button.title = word + ' — press to hide or show';
    const swatch = document.createElement('span');
    swatch.className = 'mj-graph-swatch';
    // the DOM reads the declaration's own value; only the canvas needs channels
    swatch.style.background = 'var(' + (ROLES[word] || ROLES.unknown).ring + ')';
    const name = document.createElement('span');
    name.className = 'mj-graph-legend-name';
    name.textContent = word;
    const count = document.createElement('span');
    count.className = 'mj-graph-legend-count';
    count.textContent = String(counts.get(word));
    button.append(swatch, name, count);
    button.addEventListener('click', () => {
      if (state.hidden.has(word)) state.hidden.delete(word);
      else state.hidden.add(word);
      button.setAttribute('aria-pressed', state.hidden.has(word) ? 'true' : 'false');
      button.classList.toggle('is-hidden', state.hidden.has(word));
      applyFilters(cy, state);
    });
    host.append(button);
  }
  // under the drawing and its panel, not between them
  if (!existing) (frame.parentElement || frame).insertAdjacentElement('afterend', host);
  return host;
}

// ------------------------------------------------------------------ select and show

function select(cy, state, id, centre) {
  const node = cy.getElementById(id);
  if (!node.length) return;
  state.selected = id;
  cy.batch(() => {
    cy.elements().removeClass('mj-near').addClass('mj-dim').removeClass('mj-focus');
    node.closedNeighborhood().removeClass('mj-dim').addClass('mj-near');
    node.addClass('mj-focus');
  });
  if (centre) {
    cy.stop(true, false);
    if (motionAllowed()) cy.animate({ center: { eles: node }, duration: 250 });
    else cy.center(node);
  }
  describe(state, id);
}

function clear(cy, state) {
  state.selected = '';
  cy.elements().removeClass('mj-focus').removeClass('mj-near');
  applyFilters(cy, state);
  if (!state.needle) cy.elements().removeClass('mj-dim');
  const panel = document.querySelector('[data-mj-trains-panel]');
  if (panel) {
    panel.textContent = '';
    const p = document.createElement('p');
    p.className = 'mj-note';
    p.textContent = 'Select a link to see every field of it here.';
    panel.append(p);
  }
}

/** Open the forge's page of a pull request; master and nodes without a page open nothing. */
function open(state, id) {
  const entry = state.links.get(id);
  const url = entry && (entry.link ? entry.link.url : entry.beneath ? entry.beneath.url : null);
  if (url) window.open(url, '_blank', 'noopener');
}

/** The property panel: every field of the selected node, as the capability answered it. */
function describe(state, id) {
  const panel = document.querySelector('[data-mj-trains-panel]');
  const entry = state.links.get(id);
  if (!panel || !entry) return;
  panel.textContent = '';
  const card = document.createElement('div');
  card.className = 'mj-alert mj-alert--info';
  const heading = document.createElement('strong');
  const list = document.createElement('dl');
  list.className = 'mj-facts';
  const fact = (key, value) => {
    const dt = document.createElement('dt');
    dt.className = 'mj-facts-key';
    dt.textContent = key;
    const dd = document.createElement('dd');
    dd.className = 'mj-facts-value';
    if (value instanceof Node) dd.append(value);
    else dd.textContent = value === undefined || value === null || value === '' ? '—' : String(value);
    list.append(dt, dd);
  };
  const anchor = (href, text) => {
    const a = document.createElement('a');
    a.className = 'mj-link';
    a.href = href;
    a.textContent = text;
    if (/^https?:/.test(href)) a.rel = 'noopener';
    return a;
  };
  const code = (text) => {
    const c = document.createElement('code');
    c.className = 'mj-mono';
    c.textContent = text;
    return c;
  };
  const numbers = (ns) => (ns && ns.length ? ns.map((n) => '#' + n).join(' ') : '');

  if (entry.kind === 'master') {
    const v = entry.view;
    heading.textContent = v.base || 'master';
    fact('Kind', 'the integration base every train lands on');
    fact('Commit', code(v.master_sha || ''));
    fact('Repository', v.repository);
    fact('Observed', v.observed_at);
    fact('Trains', String((v.trains || []).length));
  } else if (entry.kind === 'beneath') {
    const b = entry.beneath;
    heading.textContent = '#' + b.number;
    fact('Kind', 'beneath ' + entry.train.id + ', no longer open');
    fact('State', b.state);
    fact('Pull request', b.url ? anchor(b.url, b.url) : '');
  } else {
    const l = entry.link;
    heading.textContent = '#' + l.number + ' ' + l.title;
    fact('Train', entry.train.id + ' — turn ' + l.position + ' of ' + entry.train.links.length);
    fact('At its turn', l.at_turn ? 'yes: every link below it has landed' : 'no');
    fact('Disposition', l.disposition + ' (' + l.lane + ')');
    fact('Reasons', l.reasons.join(', '));
    fact('Next action', l.next_action);
    fact('Required checks', l.required_checks);
    fact('Checks', l.checks.map((c) => c.name + ': ' + c.state).join(', '));
    const relation = l.relation_kind + (l.behind !== null && l.behind !== undefined ? ' — ' + l.behind + ' behind' : '');
    fact('Relation to master', relation + (l.carries_master ? ', carries master' : ''));
    fact(
      'Prepared head',
      l.prep ? l.prep.reference + ' ' + l.prep.sha.slice(0, 10) + ' — ' + l.prep.state : 'none',
    );
    fact('Head', code(l.head_sha));
    fact('Branch', code(l.head_ref + ' → ' + l.base_ref));
    fact('Stands on', numbers(l.on) || 'master');
    fact('Carries', numbers(l.carries) || 'nothing');
    fact('Depth', String(l.depth));
    fact('Queue rank', String(l.rank));
    fact('In a cycle', l.in_cycle ? 'yes' : 'no');
    fact('Author', l.author);
    const links = document.createElement('span');
    if (l.url) links.append(anchor(l.url, 'the pull request'));
    if (l.checks_url) links.append(' · ', anchor(l.checks_url, 'its checks'));
    links.append(' · ', anchor('/cockpit/capabilities/integration.explain?number=' + l.number, 'explain'));
    fact('Open', links);
  }
  card.append(heading, list);
  panel.append(card);
}

// --------------------------------------------------------------------------- the style

function style(colours, type) {
  const role = (d) => colours.roles[d] || colours.roles.unknown;
  return [
    {
      selector: 'node',
      style: {
        shape: 'ellipse',
        width: 66,
        height: 66,
        'background-color': (n) => role(n.data('disposition')).fill,
        'border-width': 3,
        'border-color': (n) => role(n.data('disposition')).ring,
        label: 'data(caption)',
        color: colours.text,
        'font-size': type.caption,
        'font-family': 'ui-monospace, SFMono-Regular, Menlo, monospace',
        'text-valign': 'center',
        'text-halign': 'center',
        'text-wrap': 'wrap',
        'text-max-width': '60px',
        'overlay-padding': 4,
      },
    },
    { selector: 'node[turn = "yes"]', style: { 'border-width': 5 } },
    { selector: 'node[kind = "beneath"]', style: { 'border-style': 'dashed', width: 50, height: 50 } },
    {
      selector: 'node[kind = "master"]',
      style: {
        width: 84,
        height: 84,
        'background-color': colours.master.fill,
        'border-color': colours.master.ring,
        'border-width': 4,
        'font-weight': 'bold',
      },
    },
    {
      selector: 'edge',
      style: {
        width: 2,
        'curve-style': 'bezier',
        'line-color': colours.muted,
        'target-arrow-color': colours.muted,
        'target-arrow-shape': 'triangle',
        'arrow-scale': 1.1,
        label: 'data(label)',
        'font-size': type.meta,
        'font-family': 'ui-monospace, SFMono-Regular, Menlo, monospace',
        color: colours.muted,
        'text-rotation': 'autorotate',
        'text-background-color': colours.background,
        'text-background-opacity': 0.9,
        'text-background-padding': 2,
      },
    },
    { selector: 'edge[kind = "depends_on"]', style: { 'line-style': 'dashed' } },
    { selector: 'edge[satisfied = "yes"], edge[kind = "merged"]', style: { 'line-style': 'dotted' } },
    { selector: 'edge[kind = "targets"], edge[kind = "merged"]', style: { width: 1, 'arrow-scale': 0.8 } },
    { selector: '.mj-dim', style: { opacity: 0.15 } },
    { selector: '.mj-hidden', style: { display: 'none' } },
    {
      selector: 'edge.mj-near',
      style: { 'line-color': colours.accent, 'target-arrow-color': colours.accent, color: colours.text, width: 3 },
    },
    { selector: 'node.mj-match', style: { 'border-width': 6, 'border-color': colours.accent } },
    {
      selector: 'node.mj-focus',
      style: { 'border-width': 6, 'border-color': colours.accent, 'overlay-color': colours.accent, 'overlay-opacity': 0.12 },
    },
  ];
}

// ----------------------------------------------------------------------- the palette

/**
 * The sRGB channels of whatever a token resolves to, measured by painting it. The tokens
 * are `oklch`, which Cytoscape's colour parser does not read (see graph.js, `channels`,
 * for the measurement that found it); this is colour arithmetic at a library boundary, and
 * the decision is still share/design/tokens.yaml's. A token that does not resolve falls back
 * to the page's own computed colour, never to a literal.
 */
function channels(value, fallback) {
  if (!value) return fallback;
  try {
    const canvas = document.createElement('canvas');
    canvas.width = 1;
    canvas.height = 1;
    const ctx = canvas.getContext('2d', { willReadFrequently: true });
    ctx.clearRect(0, 0, 1, 1);
    ctx.fillStyle = value;
    ctx.fillRect(0, 0, 1, 1);
    const px = ctx.getImageData(0, 0, 1, 1).data;
    if (!px[3]) return fallback;
    return [px[0], px[1], px[2]];
  } catch (e) {
    return fallback;
  }
}

/** Every colour the drawing uses, read from the declaration and turned into channels. */
function drawable() {
  const root = getComputedStyle(document.documentElement);
  const body = getComputedStyle(document.body);
  const text = channels(body.color, [0, 0, 0]);
  const ground = channels(body.backgroundColor, [255, 255, 255]);
  const read = (token, fallback) => channels(root.getPropertyValue(token).trim(), fallback);
  const accent = read('--mj-accent', text);
  const roles = {};
  for (const [word, role] of Object.entries(ROLES)) {
    roles[word] = { fill: read(role.fill, ground), ring: read(role.ring, accent) };
  }
  return {
    roles,
    master: { fill: read(MASTER_ROLE.fill, ground), ring: read(MASTER_ROLE.ring, accent) },
    text: read('--mj-fg', text),
    muted: read('--mj-muted', text),
    accent,
    background: read('--mj-sunken', ground),
  };
}

/** The declared type scale, for a canvas that cannot be given a CSS class. */
function typeScale() {
  const root = getComputedStyle(document.documentElement);
  const step = (name) => {
    const value = parseFloat(root.getPropertyValue(name));
    return Number.isFinite(value) && value > 0 ? value : 0;
  };
  const meta = step('--text-meta') || step('--text-label') || parseFloat(getComputedStyle(document.body).fontSize);
  // a caption inside a circle is read at a glance: the label step when the scale has one
  return { meta, caption: step('--text-label') || meta };
}

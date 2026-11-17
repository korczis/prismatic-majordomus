// The build half of the site's diagrams (ADR 0152): what can be decided about a diagram
// before a browser draws it.
//
// A diagram reaches the built site as <pre class="mermaid">, projected from a ```mermaid
// fence or written by a template from generated data. This pass reads every one of them in
// the pages Zola just wrote and gives it three things nobody typed:
//
//   a name    `id="diagram-<name>"`: the template's own `data-diagram`, else the id of the
//             heading the diagram sits under, else its ordinal on the page. A second diagram
//             under one heading is `<name>-2`. The id is what `#diagram-<name>` lands on,
//             and scripts/ci/link-check resolves it like any other fragment.
//   its nodes `data-nodes`: the node ids its source declares, so that a link to one node,
//             `#diagram-<name>:<node>`, can be refused when the diagram has no such node.
//   controls  the viewer's toolbar, written `hidden` right after it: zoom, fit, full screen,
//             download, copy link. site/diagrams.js reveals it once the diagram is drawn, so a
//             reader without JavaScript never meets a button that does nothing. It is markup
//             and not something the script builds, because a control the markup does not
//             declare is one scripts/ci/interaction-check cannot hold to a behaviour spec.
//   its links a list beside it: every node whose id or label is the identity of an object
//             this site has a page for (a rule, a decision, a command, a capability, an
//             issue, a milestone, a document), linked to that page. The identities and the
//             routes are read from site/data/generated; a route the build did not write is
//             not linked. site/diagrams.js makes the same nodes clickable from this list,
//             and a reader without JavaScript has the list.
//
// Nothing here is a second source. A link exists because an identity the data carries
// appears in a diagram; removing the object removes the link at the next build, and a
// diagram's author writes no href. The list is ordinary markup, so every link in it is
// decided by scripts/ci/link-check with the rest of the site's.
//
//   node scripts/lib/diagrams.mjs <public dir> <generated data dir> [<base url>]
//
// Exit 0 with one summary line; 10 when a page would carry two diagrams with one id or a
// diagram arrives with an id this pass did not derive; 12 when an input cannot be read.

import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

/** What a diagram's anchor starts with. `#diagram-<name>` and `#diagram-<name>:<node>`. */
export const PREFIX = 'diagram-';

const ENTITIES = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", '#39': "'", '#x27': "'", '#x2F': '/', nbsp: ' ' };

/** The text of an HTML text node: what the browser hands Mermaid as the source. */
export function unescapeHtml(text) {
  return text.replace(/&(#x?[0-9a-fA-F]+|[a-z]+);/g, (whole, name) => {
    if (name in ENTITIES) return ENTITIES[name];
    if (name[0] !== '#') return whole;
    const code = name[1] === 'x' || name[1] === 'X' ? parseInt(name.slice(2), 16) : parseInt(name.slice(1), 10);
    return Number.isFinite(code) ? String.fromCodePoint(code) : whole;
  });
}

/** Text as an attribute value or a text node. */
export function escapeHtml(text) {
  return String(text).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

// A flowchart's statements that declare no node.
const FLOW_SKIP = /^(classDef|class|style|linkStyle|click|direction|accTitle|accDescr|subgraph|end)\b/;
// What opens a node's shape, longest first, with what closes it.
const SHAPES = [
  ['(((', ')))'], ['((', '))'], ['([', '])'], ['[[', ']]'], ['[(', ')]'], ['{{', '}}'],
  ['[/', ']'], ['[\\', ']'], ['[', ']'], ['(', ')'], ['{', '}'], ['>', ']'],
];
// `a-b` and `a.b` are ids; `A-->B` is the id `A` and a link
const ID = /[A-Za-z0-9_]+(?:[.-][A-Za-z0-9_]+)*/y;

/**
 * A label as a reader sees it: breaks become spaces, tags and Markdown marks are dropped, and
 * the entities a Mermaid label is written with (`&lt;`, `#quot;`, `#35;`) are what they stand for.
 */
export function plain(label) {
  const text = label.replace(/<br\s*\/?>/gi, ' ').replace(/<[^>]*>/g, '').replace(/[`*"]/g, '');
  return unescapeHtml(text)
    .replace(/#(\w+);/g, (whole, name) => (/^\d+$/.test(name) ? String.fromCodePoint(Number(name)) : ENTITIES[name] ?? whole))
    .replace(/\s+/g, ' ').trim();
}

/** A label as its lines: what stands on each side of a <br>. */
export function segments(label) {
  return label.split(/<br\s*\/?>/i).map(plain).filter(Boolean);
}

/**
 * The nodes a Mermaid source declares, as `{ id, label }`, in first-seen order.
 *
 * Read for flowcharts (`flowchart`, `graph`) and state diagrams, the two kinds whose nodes
 * carry ids a fragment can name. Any other kind declares none here, and the diagram is still
 * named, drawn and linkable as a whole.
 *
 * ```
 * nodes('flowchart LR\n  A[docs/DESIGN.md] --> B')   // [{id:'A',label:'docs/DESIGN.md'},{id:'B',label:'B'}]
 * ```
 */
export function nodes(source) {
  const lines = source.split('\n').map((l) => l.replace(/%%.*$/, '').trim()).filter(Boolean);
  const kind = (lines[0] || '').split(/\s+/)[0];
  const found = new Map();
  const add = (id, label) => {
    if (!found.has(id)) found.set(id, { id, label: label ?? id });
    else if (label != null && found.get(id).label === id) found.get(id).label = label;
  };
  if (kind === 'flowchart' || kind === 'graph') {
    for (const line of lines.slice(1)) {
      if (FLOW_SKIP.test(line)) continue;
      flowLine(line, add);
    }
  } else if (kind === 'stateDiagram' || kind === 'stateDiagram-v2') {
    for (const line of lines.slice(1)) {
      const named = /^state\s+"([^"]*)"\s+as\s+([A-Za-z0-9_]+)/.exec(line);
      if (named) { add(named[2], named[1]); continue; }
      const edge = /^(\[\*\]|[A-Za-z0-9_]+)\s*-->\s*(\[\*\]|[A-Za-z0-9_]+)/.exec(line);
      if (edge) { for (const id of [edge[1], edge[2]]) if (id !== '[*]') add(id); continue; }
      const described = /^([A-Za-z0-9_]+)\s*:\s*(.+)$/.exec(line);
      if (described && !/^(note|direction|state)$/.test(described[1])) add(described[1], described[2]);
    }
  }
  return [...found.values()];
}

// One flowchart statement: ids, each optionally followed by a shape, joined by links whose
// text (`-- text -->`, `-->|text|`, `-. "text" .->`) is not a node.
function flowLine(line, add) {
  let i = 0;
  const n = line.length;
  while (i < n) {
    const c = line[i];
    if (c === '|') { const j = line.indexOf('|', i + 1); i = j < 0 ? n : j + 1; continue; }
    if (c === '"') { const j = line.indexOf('"', i + 1); i = j < 0 ? n : j + 1; continue; }
    if (c === '@' && line[i + 1] === '{') { const j = line.indexOf('}', i); i = j < 0 ? n : j + 1; continue; }
    if (c === '-' || c === '=' || c === '.' || c === '~') {
      const rest = line.slice(i);
      // `-- text -->`, `-. text .->`, `== text ==>`: an opener with no head, text, a closer
      const worded = /^(?:--|-\.|==)(?![-=.>]|[ox]\s)\s*(?:"[^"]*"|[^"]*?)\s*(?:--+[>ox]?|\.-+[>ox]?|==+[>ox]?)/.exec(rest);
      const arrow = worded || /^[-=.~]+(?:>|[ox](?=\s|$))?/.exec(rest);
      i += Math.max(arrow[0].length, 1);
      continue;
    }
    ID.lastIndex = i;
    const m = ID.exec(line);
    if (!m) { i++; continue; }
    const id = m[0];
    i = ID.lastIndex;
    // `A o--o B`, `A x--x B`: a link's tail, not a node
    if ((id === 'o' || id === 'x') && /[-=.]/.test(line[i] || '')) continue;
    let label = null;
    for (const [open, close] of SHAPES) {
      if (!line.startsWith(open, i)) continue;
      const from = i + open.length;
      let to;
      if (line[from] === '"') {
        const q = line.indexOf('"', from + 1);
        to = q < 0 ? -1 : line.indexOf(close, q + 1);
      } else {
        to = line.indexOf(close, from);
      }
      if (to < 0) break;
      label = line.slice(from, to).trim().replace(/^"|"$/g, '');
      i = to + close.length;
      break;
    }
    if (line.startsWith(':::', i)) { i += 3; ID.lastIndex = i; if (ID.exec(line)) i = ID.lastIndex; }
    add(id, label);
  }
}

// The shapes an identity takes inside a longer label, and whether the shape is strict: a
// strict shape is an identity and nothing else (`ADR 0152`, `I2653`, `docs/DESIGN.md`,
// `majordomus plan`), so one that resolves to no page means the label names something this
// site cannot link, and the node is left alone rather than linked to whatever else it names.
// The loose shape (a dotted id) also matches file names and versions, so an unresolved one
// says nothing. A bare file name is not here at all: `design.md` inside a sentence is as
// likely docs/generated/design.md as docs/DESIGN.md, and only a whole label may claim it.
const TOKENS = [
  [/\bmajordomus(?: [a-z][a-z0-9-]*){1,2}/g, true],
  [/\bADR[ -]?\d{4}\b/gi, true],
  [/\b[IM]\d{3,4}\b/g, true],
  [/\bdocs\/(?:[A-Za-z0-9_.-]+\/)*[A-Za-z0-9_.-]+\.md\b/g, true],
  [/\b[a-z][a-z0-9-]*(?:\.[a-z0-9][a-z0-9-]*)+(?:@\d+)?/g, false],
];

const key = (text) => text.trim().toLowerCase().replace(/\s+/g, ' ');

/**
 * The page a node stands for, or null.
 *
 * A node resolves when its id, its whole label or one line of its label is an identity in the
 * index. Failing that, the identity-shaped tokens inside the label are looked up, and the node
 * resolves only when they agree on one page: a label naming two objects links to neither,
 * and so does one naming an object this site has no page for, because a link that picked the
 * one it could resolve would be a guess.
 */
export function resolve(node, index) {
  for (const exact of [node.id, plain(node.label), ...segments(node.label)]) {
    const hit = index.get(key(exact));
    if (hit) return hit;
  }
  const hits = new Map();
  const text = segments(node.label).join(' ');
  for (const [shape, strict] of TOKENS) {
    for (const token of text.match(shape) || []) {
      // `majordomus plan next` names the command `plan`: try the longest form first
      const words = token.split(' ');
      let hit = null;
      for (let n = words.length; n >= (words[0] === 'majordomus' ? 2 : 1) && !hit; n--) hit = index.get(key(words.slice(0, n).join(' ')));
      if (hit && hit.whole) hit = null;
      if (hit) hits.set(hit.href, hit);
      else if (strict) return null;
    }
  }
  return hits.size === 1 ? [...hits.values()][0] : null;
}

function readJson(dir, name) {
  const path = join(dir, name);
  if (!existsSync(path)) return null;
  return JSON.parse(readFileSync(path, 'utf8'));
}

/**
 * Every identity the generated data gives a page to, as `identity -> { href, title, kind }`.
 *
 * `served(route)` says whether the build wrote that route; an identity whose page was not
 * written is left out, so the index never offers a link the site cannot keep. An identity two
 * objects claim is dropped as well: it names neither.
 */
export function buildIndex(dataDir, served, base = '') {
  const index = new Map();
  const dropped = new Set();
  const root = base.replace(/\/+$/, '');
  // `whole` marks an identity that only a whole label (or a whole line of one) may claim
  const offer = (identity, route, title, kind, whole = false) => {
    if (!identity || !route) return;
    const k = key(identity);
    if (dropped.has(k) || !served(route)) return;
    const entry = { href: root + route, title: title || identity, kind, whole };
    const had = index.get(k);
    if (had && had.href !== entry.href) { index.delete(k); dropped.add(k); return; }
    index.set(k, entry);
  };

  const entities = readJson(dataDir, 'entities.json');
  if (entities) {
    const routes = Object.fromEntries((entities.kinds || []).map((k) => [k.kind, k.route]));
    for (const e of entities.entities || []) {
      const route = routes[e.kind] ? `${routes[e.kind]}${e.slug}/` : null;
      offer(e.identity, route, e.title, e.kind);
      if (e.kind === 'adr') {
        const number = /(\d{4})$/.exec(e.identity || '');
        if (number) for (const form of [`ADR ${number[1]}`, `ADR-${number[1]}`, `ADR${number[1]}`]) offer(form, route, e.title, e.kind);
      } else {
        // a rule is cited with and without the version it is at
        offer(String(e.identity || '').replace(/@\d+$/, ''), route, e.title, e.kind);
      }
    }
  }
  const plan = readJson(dataDir, 'plan.json');
  if (plan) {
    for (const i of plan.issues || []) offer(i.id, `/plan/${String(i.id).toLowerCase()}/`, i.title, 'issue');
    for (const m of plan.milestones || []) offer(m.id, `/plan/${String(m.id).toLowerCase()}/`, m.title, 'milestone');
  }
  const commands = readJson(dataDir, 'commands.json');
  if (commands) {
    for (const c of commands.commands || []) offer(`majordomus ${c.name}`, `/commands/${c.name}/`, c.summary, 'command');
  }
  const executable = readJson(dataDir, 'executable.json');
  if (executable) {
    for (const c of executable.capabilities || []) offer(c.id, c.route, c.id, 'capability');
  }
  const doctrines = readJson(dataDir, 'doctrines.json');
  if (doctrines) {
    for (const d of doctrines.doctrines || []) offer(d.id, `/doctrines/${d.slug}/`, d.title, 'doctrine');
  }
  const docs = readJson(dataDir, 'docs.json');
  if (docs) {
    for (const d of docs.docs || []) {
      offer(`docs/${d.path}`, d.route, d.summary, 'document');
      offer(d.path, d.route, d.summary, 'document', true);
    }
  }
  return index;
}

/**
 * The viewer's controls, in toolbar order: the action site/diagrams.js performs, what the
 * button shows, and its accessible name (which contains what it shows).
 */
export const ACTIONS = [
  ['zoom-out', '\u2212', 'Zoom out'],
  ['zoom-in', '+', 'Zoom in'],
  ['fit', 'Fit', 'Fit the diagram to its frame'],
  ['expand', 'Expand', 'Expand to full screen'],
  ['download', 'SVG', 'Download SVG'],
  ['link', 'Link', 'Link: copy the address of this diagram'],
];

function toolbar(id, name) {
  const buttons = ACTIONS.map(([action, text, label]) =>
    `<button type="button" class="mj-button mj-button--quiet" data-diagram-action="${action}" aria-label="${label}" title="${label}">${text}</button>`).join('');
  return `<div class="mj-diagram-bar" role="toolbar" aria-label="Diagram ${escapeHtml(name)}" aria-controls="${id}" data-diagram-bar="${id}" hidden>${buttons}<span class="mj-diagram-status" role="status"></span></div>`;
}

const PRE = /<pre\b([^>]*\bclass="(?:[^"]*\s)?mermaid(?:\s[^"]*)?"[^>]*)>([\s\S]*?)<\/pre>/g;
const HEADING = /<h[1-6]\b[^>]*\sid="([^"]+)"/g;
const attr = (attrs, name) => {
  const m = new RegExp(`\\s${name}="([^"]*)"`).exec(attrs);
  return m ? m[1] : null;
};
const slug = (text) => text.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '');

/**
 * One built page with its diagrams named, described and linked.
 *
 * Returns `{ text, diagrams, links }`: the page (the same string when it carries no diagram),
 * how many diagrams it holds and how many node links were inferred. Running it over its own
 * output changes nothing. Throws when the page would carry one diagram id twice, or a diagram
 * arrives with an id this pass would not have given it.
 */
export function annotate(html, index = new Map()) {
  if (!html.includes('mermaid')) return { text: html, diagrams: 0, links: 0 };
  // what a previous run wrote is derived output: drop it and derive again
  const clean = html.replace(/<details class="mj-diagram-links"[\s\S]*?<\/details>/g, '')
    .replace(/<div class="mj-diagram-bar"[\s\S]*?<\/div>/g, '');
  const headings = [...clean.matchAll(HEADING)].map((m) => ({ at: m.index, id: m[1] }));
  // every id the page already carries, other than the ones this pass gives its diagrams
  const taken = new Set([...clean.replace(PRE, '').matchAll(/\sid="([^"]+)"/g)].map((m) => m[1]));
  const given = new Set();
  let diagrams = 0;
  let links = 0;
  const text = clean.replace(PRE, (whole, attrs, body, at) => {
    diagrams++;
    const had = attr(attrs, 'id');
    let base = attr(attrs, 'data-diagram');
    if (had && !had.startsWith(PREFIX)) {
      throw new Error(`a diagram carries id="${had}"; a diagram's id is ${PREFIX}<name> and is derived, so name it with data-diagram instead`);
    }
    if (base == null || base === '') {
      const above = headings.filter((h) => h.at < at).pop();
      base = above ? above.id : String(diagrams);
    }
    base = slug(base) || String(diagrams);
    let name = base;
    for (let n = 2; given.has(name) || taken.has(PREFIX + name); n++) name = `${base}-${n}`;
    if (attr(attrs, 'data-diagram') != null && attr(attrs, 'data-diagram') !== '' && name !== base) {
      throw new Error(`two diagrams on one page are named "${base}"; a declared name is an address and must be the only one`);
    }
    given.add(name);
    const declared = nodes(unescapeHtml(body));
    let kept = attrs.replace(/\s(?:id|data-diagram|data-nodes)="[^"]*"/g, '');
    if (attr(kept, 'tabindex') == null) kept += ' tabindex="0"';
    const opened = `<pre${kept} id="${PREFIX}${name}" data-diagram="${name}" data-nodes="${escapeHtml(declared.map((n) => n.id).join(' '))}">`;
    const rows = [];
    for (const node of declared) {
      const hit = resolve(node, index);
      if (!hit) continue;
      const label = plain(node.label) || node.id;
      rows.push(`<li><a href="${escapeHtml(hit.href)}" data-node="${escapeHtml(node.id)}" data-kind="${escapeHtml(hit.kind)}">${escapeHtml(label)}</a></li>`);
    }
    links += rows.length;
    const list = rows.length
      ? `<details class="mj-diagram-links" data-diagram-links="${PREFIX}${name}"><summary>Linked from this diagram (${rows.length})</summary><ul>${rows.join('')}</ul></details>`
      : '';
    // after the diagram, never in front of it: a <pre> is judged to scroll by the wrapper that
    // stands directly before it (scripts/lib/pre-coverage.awk), and the viewer moves the
    // toolbar above the frame when it assembles it
    return `${opened}${body}</pre>${toolbar(PREFIX + name, name)}${list}`;
  });
  return { text, diagrams, links };
}

function walk(dir, out = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) walk(path, out);
    else if (entry.name.endsWith('.html')) out.push(path);
  }
  return out;
}

function main(argv) {
  const [pub, dataDir, base = ''] = argv;
  if (!pub || !dataDir) {
    console.error('usage: node scripts/lib/diagrams.mjs <public dir> <generated data dir> [<base url>]');
    process.exit(2);
  }
  if (!existsSync(pub) || !existsSync(dataDir)) {
    console.error(`diagrams: ${existsSync(pub) ? dataDir : pub} does not exist`);
    process.exit(12);
  }
  let index;
  try {
    index = buildIndex(dataDir, (route) => existsSync(join(pub, route, 'index.html')), base);
  } catch (error) {
    console.error(`diagrams: the generated data cannot be read: ${error.message}`);
    process.exit(12);
  }
  let pages = 0, diagrams = 0, links = 0, failed = 0;
  for (const page of walk(pub)) {
    const before = readFileSync(page, 'utf8');
    let result;
    try {
      result = annotate(before, index);
    } catch (error) {
      console.error(`diagrams: ${page.slice(pub.length + 1)}: ${error.message}`);
      failed++;
      continue;
    }
    if (!result.diagrams) continue;
    pages++; diagrams += result.diagrams; links += result.links;
    if (result.text !== before) writeFileSync(page, result.text);
  }
  if (failed) process.exit(10);
  console.log(`  ${diagrams} diagram(s) named on ${pages} page(s), ${links} node link(s) inferred from ${index.size} identities`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main(process.argv.slice(2));

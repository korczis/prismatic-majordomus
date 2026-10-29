# majordomus-covers: none
# A chart on the site is read by pointing at it (I4400, ADR 0152). site/charts.js draws every
# <figure data-chart> from the table it wraps; this case runs it in node over a page shaped
# like the three that carry charts (trends, economics, the plan), with a document small enough
# to write here and an ECharts that records what it is asked to draw, so every answer is the
# component's own and none is the library's. What is decided:
#
#   1. one tooltip at a category: every chart triggers on the axis, under a vertical rule on a
#      line (whose pill says the day, and no horizontal rule whose pill would say a value the
#      table does not hold) and a band on bars; the tooltip names every visible series at the
#      category in the unit, says "not recorded" for a missing value, adds the whole of a
#      stack, and escapes what the table says; where every place beside the rule covers a
#      line it is laid out as one line over the header band, and otherwise it floats;
#   2. the legend: two or more series have a row of toggle buttons in the card's header (one per
#      series, saying with aria-pressed whether it is shown, its value beside its name: a line's
#      latest, a stack's column total) over a selection ECharts keeps but does not draw, and one
#      series has a readout and no button; each series is painted in its column's colour (a status
#      badge's, the neutral gray for a neutral state, the second one hatched, else the series token
#      of its place), and hiding a series repaints none of the others, leaves the value axis free
#      to rescale, and rounds and labels the new outermost segment of a stack; pointing at a key
#      focuses its series (the others fade, its area fills) and letting go gives every line back;
#   3. the zoom: a line chart whose categories exceed its width / 24 has a slider zoom (no
#      `inside` zoom, which would cancel every wheel over the plot) and a Reset button that is
#      disabled at the whole range, enabled once zoomed, and gives the whole range back; a plain
#      wheel is never cancelled, ctrl + wheel zooms; a chart that fits, and every bar chart, has
#      none; the keyboard moves the tooltip along the categories, anchored on a series measured
#      there (or on the axis when none is), and leaves a key held with ctrl, alt or meta alone;
#   4. the marks: 2px straight lines with no symbols of their own, a dot on every line measured
#      at the category pointed at (and none on a line that recorded nothing there); each line's
#      area a silent series of its own, named as its line (so hidden with it), that no tooltip
#      names, fading from the series colour at the stylesheet's --chart-area-opacity under a
#      lone series (and, with more, none until one is focused); the latest value marked, a
#      value with no measured neighbour dotted, a gap bridged by a silent dashed line between
#      the measured values either side of it (and the header's `not recorded` key shown) and
#      never otherwise, the bridges and marks held unseen until ECharts says the line has
#      drawn itself in (its `finished` event) and then lit; a lone line lifted by weight in
#      the light theme and by a glow in the dark one; bars rounded at the data end only,
#      stacked segments parted by the card's colour, a recorded zero drawn as no sliver, the
#      two neutral states one colour told apart by the hatch alone, and a phone's bar names
#      on their own line above their bars; dates labelled compactly on the axis (the year
#      under the first label of a year, a time only between two labels of one day) and in full
#      in the tooltip;
#   5. reduced motion: drawn without animation when it is asked for, and followed when it
#      changes; with it off, the motion is the design's one duration;
#   6. a theme change draws again in the other theme's tokens;
#   7. without the script every figure is its table: each template wraps a table, the Reset
#      control is an inert <template>, and the script leaves every table where it was;
#   8. the interaction specs: the template's button is claimed by the `chart` spec alone and a
#      chart's frame by no scroll-region claim.
. "$ROOT/test/lib.sh"
command -v node >/dev/null 2>&1 || skip "no node"

# ---------------------------------------------------------------- 7. the templates, without JS
for t in trends economics plan-section; do
  f="$ROOT/site/templates/$t.html"
  awk '/<figure[^>]*data-chart=/{open=1} open && /<table/{found=1} /<\/figure>/{if(open && !found){bad=1}; open=0; found=0} END{exit bad}' "$f" \
    || { echo "    a chart figure of $t.html wraps no table"; exit 1; }
  grep -q 'data-chart=' "$f" || { echo "    $t.html declares no chart"; exit 1; }
done
grep -q '<template data-chart-tools>.*data-chart-action="reset"' "$ROOT/site/templates/partials/charts.html" \
  || { echo "    the Reset control is not an inert <template> in partials/charts.html"; exit 1; }
grep -q '<template data-chart-legend>.*data-chart-action="series"' "$ROOT/site/templates/partials/charts.html" \
  || { echo "    the legend's button is not an inert <template> in partials/charts.html"; exit 1; }

# ---------------------------------------------------------------- 8. the specs claim what they should
OUT="$T/claims.out"
node --input-type=module - "$ROOT/scripts/lib" > "$OUT" 2>&1 <<'NODE' \
  || { echo "    the interaction specs do not claim the chart's controls as they should:"; sed 's/^/    | /' "$OUT"; exit 1; }
const lib = process.argv[2];
const { discover, loadSpecs } = await import(`${lib}/interactions.mjs`);
const specs = await loadSpecs(`${lib}/interaction-specs`);
const page = '<template data-chart-tools><div class="mj-chart-tools"><button type="button" data-chart-action="reset">Reset zoom</button></div></template>'
  + '<div class="mj-chart" tabindex="0" role="img" data-chart-frame></div>'
  + '<button type="button" data-chart-action="reset" data-chart-for="chart-x">Reset zoom</button>'
  + '<template data-chart-legend><div class="mj-chart-keys" role="group"><button type="button" data-chart-action="series" aria-pressed="true">series</button></div></template>'
  + '<button type="button" data-chart-action="series" data-chart-for="chart-x" aria-pressed="true">lines</button>';
const found = discover(page);
const claim = (el) => specs.filter((s) => s.claims(el)).map((s) => s.id);
const fail = (why) => { console.error(why); process.exit(1); };
if (found.length !== 5) fail(`discovered ${found.length} control(s), not 5`);
if (!found[0].conditional) fail('the Reset template button is not counted as conditional');
if (claim(found[0]).join() !== 'chart') fail(`the Reset template button is claimed by [${claim(found[0])}], not [chart]`);
if (claim(found[1]).length) fail(`a chart frame is claimed by [${claim(found[1])}]; the chart spec drives it`);
if (claim(found[2]).length) fail(`a chart's own Reset copy is claimed by [${claim(found[2])}]; the chart spec drives it through its chart`);
if (!found[3].conditional) fail('the legend template button is not counted as conditional');
if (claim(found[3]).join() !== 'chart') fail(`the legend template button is claimed by [${claim(found[3])}], not [chart]`);
if (claim(found[4]).length) fail(`a chart's own legend button is claimed by [${claim(found[4])}]; the chart spec drives it through its chart`);
NODE

# ---------------------------------------------------------------- 1-6. the component, in node
OUT="$T/charts.out"
node --input-type=module - "$ROOT/site/charts.js" > "$OUT" 2>&1 <<'NODE' \
  || { echo "    site/charts.js does not draw what I4400 says:"; sed 's/^/    | /' "$OUT"; exit 1; }
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
const source = readFileSync(process.argv[2], 'utf8');
const fail = (why) => { console.error(why); process.exit(1); };
const ok = (cond, why) => { if (!cond) fail(why); };
const eq = (a, b, why) => { if (JSON.stringify(a) !== JSON.stringify(b)) fail(`${why}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`); };

// ---- a document small enough to write here
class El {
  constructor(tag, attrs = {}, kids = []) {
    this.tagName = tag.toUpperCase(); this.attrs = { ...attrs }; this.children = []; this.parentNode = null;
    this.text = ''; this.style = {}; this.listeners = {}; this.hidden = false; this.disabled = false;
    for (const k of kids) { if (typeof k === 'string') this.text += k; else this.appendChild(k); }
    const self = this;
    this.classList = {
      contains: (c) => (self.attrs.class || '').split(/\s+/).includes(c),
      toggle: (c, on) => { const s = new Set((self.attrs.class || '').split(/\s+/).filter(Boolean)); if (on ?? !s.has(c)) s.add(c); else s.delete(c); self.attrs.class = [...s].join(' '); },
      add: (c) => self.classList.toggle(c, true), remove: (c) => self.classList.toggle(c, false),
    };
    this.dataset = {};
  }
  get id() { return this.attrs.id || ''; } set id(v) { this.attrs.id = v; }
  get className() { return this.attrs.class || ''; } set className(v) { this.attrs.class = v; }
  get type() { return this.attrs.type; } set type(v) { this.attrs.type = v; }
  get href() { return this.attrs.href; }
  get textContent() { return this.text + this.children.map((c) => c.textContent).join(''); }
  set textContent(v) { this.text = String(v); this.children = []; }
  getAttribute(n) { return n in this.attrs ? this.attrs[n] : null; }
  setAttribute(n, v) { this.attrs[n] = String(v); }
  hasAttribute(n) { return n in this.attrs; }
  appendChild(c) { return this.insertBefore(c, null); }
  insertBefore(c, ref) {
    if (c.parentNode) c.remove();
    const i = ref ? this.children.indexOf(ref) : -1;
    if (i < 0) this.children.push(c); else this.children.splice(i, 0, c);
    c.parentNode = this; return c;
  }
  remove() { if (this.parentNode) { const p = this.parentNode.children; p.splice(p.indexOf(this), 1); this.parentNode = null; } }
  get firstChild() { return this.children[0] || null; }
  get firstElementChild() { return this.children[0] || null; }
  get nextSibling() { if (!this.parentNode) return null; const p = this.parentNode.children; return p[p.indexOf(this) + 1] || null; }
  get rows() { return this.all().filter((e) => e.tagName === 'TR'); }
  get cells() { return this.children.filter((e) => e.tagName === 'TD' || e.tagName === 'TH'); }
  get tHead() { return this.children.find((e) => e.tagName === 'THEAD') || null; }
  get tBodies() { return this.children.filter((e) => e.tagName === 'TBODY'); }
  get clientWidth() { return widths[this.id] ?? 1280; }
  get clientHeight() { return 320; }
  getBoundingClientRect() { return { left: 0, top: 0, width: this.clientWidth, height: 320 }; }
  addEventListener(n, f) { (this.listeners[n] ||= []).push(f); }
  fire(n, e = {}) { for (const f of this.listeners[n] || []) f({ preventDefault() {}, ...e }); }
  getContext() { return paint; }
  all() { const out = []; const walk = (e) => { for (const c of e.children) { out.push(c); walk(c); } }; walk(this); return out; }
  matches(sel) {
    return sel.split(',').some((one) => {
      const m = /^([a-z]*)((?:\.[\w-]+)*)((?:\[[\w-]+(?:="[^"]*")?\])*)$/.exec(one.trim());
      if (!m) throw new Error(`the stub does not know the selector ${one}`);
      if (m[1] && this.tagName !== m[1].toUpperCase()) return false;
      for (const c of m[2].split('.').filter(Boolean)) if (!this.classList.contains(c)) return false;
      for (const a of m[3].match(/\[[^\]]+\]/g) || []) {
        const [, n, v] = /^\[([\w-]+)(?:="([^"]*)")?\]$/.exec(a);
        if (!(n in this.attrs) || (v !== undefined && this.attrs[n] !== v)) return false;
      }
      return true;
    });
  }
  querySelectorAll(sel) { return this.all().filter((e) => e.matches(sel)); }
  querySelector(sel) { return this.querySelectorAll(sel)[0] || null; }
}
const h = (tag, attrs, ...kids) => new El(tag, attrs || {}, kids);

// the page's tokens, light and dark, as the browser would resolve them
const LIGHT = { '--mj-fg': [17, 24, 39], '--mj-body': [75, 85, 99], '--mj-muted': [107, 114, 128], '--mj-faint': [107, 114, 128],
  '--mj-line': [229, 231, 235], '--mj-bg': [255, 255, 255], '--mj-raised': [255, 255, 254], '--mj-sunken': [249, 250, 251],
  '--mj-accent': [29, 78, 216], '--mj-accent-soft': [239, 246, 255], '--mj-accent-fill': [29, 78, 215], '--mj-neutral': [55, 65, 81],
  '--mj-series-1': [37, 99, 235], '--mj-series-2': [234, 88, 12], '--mj-series-3': [5, 150, 105], '--mj-series-4': [124, 58, 237] };
const DARK = Object.fromEntries(Object.entries(LIGHT).map(([k, v]) => [k, v.map((c) => 255 - c)]));
const STATUS = { done: [4, 120, 87], active: [30, 64, 175], blocked: [159, 18, 57] };
const rgb = (v) => `rgb(${v.join(', ')})`;
const toHex = (v) => '#' + v.map((c) => c.toString(16).padStart(2, '0')).join('');
const tokens = () => (root.classList.contains('dark') ? DARK : LIGHT);
let painted = [0, 0, 0];
const paint = {
  save() {}, restore() {}, clearRect() {}, fillRect() {},
  set fillStyle(v) { const m = /rgb\((\d+), (\d+), (\d+)\)/.exec(v); painted = m ? m.slice(1).map(Number) : [0, 0, 0]; },
  getImageData() { return { data: [...painted, 255] }; },
};
function getComputedStyle(el) {
  return {
    getPropertyValue(n) {
      if (el === root && n in tokens()) return ' x';
      if (el === root && n === '--mj-motion-fast') return ' 150ms';
      if (n === '--chart-area-opacity' && el.classList.contains('mj-chart')) return ' 0.16';
      return '';
    },
    get color() {
      const v = /var\((--[\w-]+)\)/.exec(el.style.color || '');
      if (v) return rgb(tokens()[v[1]]);
      const word = (/mj-badge--([\w-]+)/.exec(el.className) || [])[1];
      if (word) return rgb(STATUS[word] ? (root.classList.contains('dark') ? STATUS[word].map((c) => 255 - c) : STATUS[word]) : tokens()['--mj-neutral']);
      return rgb(tokens()['--mj-fg']);
    },
    fontFamily: 'Inter',
  };
}

// ---- the page: the trends coverage and cost, the economics history, the plan's milestones
const runs = Array.from({ length: 60 }, (_, i) => i);
const cov = (i, k) => (i === 59 && k === 2 ? null : 60 + i / 3 + k);
const coverage = h('figure', { id: 'chart-trends-coverage', 'data-chart': 'line', 'data-chart-unit': '%', 'data-chart-label': 'Coverage' },
  h('figcaption', {}, 'The coverage at each run.'),
  h('div', { class: 'overflow-x-auto', tabindex: '0' }, h('table', {},
    h('thead', {}, h('tr', {}, h('th', {}, 'Run'), h('th', { 'data-series': 'lines' }, 'Lines'), h('th', { 'data-series': 'functions' }, 'Functions'), h('th', { 'data-series': 'regions' }, 'Regions'))),
    h('tbody', {}, ...runs.map((i) => h('tr', { id: `chart-trends-coverage:${i}` }, h('td', {}, h('a', { href: `https://forge.test/runs/${i}` }, `run ${i}`)),
      ...[0, 1, 2].map((k) => (cov(i, k) == null ? h('td', {}, 'not recorded') : h('td', { 'data-value': String(cov(i, k)) }, `${cov(i, k).toFixed(2)}%`)))))))));
// the runs are dated, across a new year and twice in one day
const started = ['2026-12-31 00:00', '2027-01-01 09:30', '2027-01-01 18:00', '2027-01-02 00:00'];
const cost = h('figure', { id: 'chart-trends-cost', 'data-chart': 'line', 'data-chart-unit': 'min' },
  h('table', {}, h('thead', {}, h('tr', {}, h('th', {}, 'Run'), h('th', { 'data-series': 'wall' }, 'Wall'), h('th', { 'data-series': 'queued' }, 'Queued'))),
    // wall is not recorded on the second and fourth runs: the first series has no value there, and the first run stands alone
    h('tbody', {}, ...[0, 1, 2, 3].map((i) => h('tr', {}, h('td', {}, started[i]), h('td', {}, i % 2 ? 'not recorded' : String(10 + i)), h('td', {}, String(i)))))));
const history = h('figure', { id: 'chart-context-history', 'data-chart': 'line', 'data-chart-unit': '%' },
  h('table', {}, h('thead', {}, h('tr', {}, h('th', {}, 'At'), h('th', { 'data-series': 'context reduction' }, 'Reduction'))),
    h('tbody', {}, ...[0, 1, 2, 3, 4].map((i) => h('tr', {}, h('td', {}, i === 0 ? '2026-10-10 <img src=x>' : `2026-10-1${i}`), h('td', { 'data-value': String(70 + i) }, `${70 + i}%`))))));
const states = ['done', 'active', 'ready', 'planned', 'blocked'];
const counts = { M000: [5, 2, 1, 1, 3], M001: [0, 4, 2, 0, 0], M002: [7, 0, 0, 2, 0] };
const milestones = h('figure', { id: 'chart-milestones', 'data-chart': 'bar', 'data-chart-stack': '', 'data-chart-horizontal': '', 'data-chart-unit': 'issues' },
  h('table', {}, h('thead', {}, h('tr', {}, h('th', {}, 'milestone'), ...states.map((s) => h('th', { 'data-series': s }, h('span', { class: `mj-badge mj-badge--${s}` }, s))))),
    h('tbody', {}, ...Object.entries(counts).map(([m, c]) => h('tr', { id: `chart-milestones:${m.toLowerCase()}` },
      h('td', { 'data-category': m }, h('a', { href: `/plan/${m.toLowerCase()}/` }, m)), ...c.map((n) => h('td', {}, String(n))))))));
const widths = { 'chart-trends-coverage-frame': 400, 'chart-context-history-frame': 400, 'chart-milestones-frame': 400, 'chart-trends-cost-frame': 1280 };
const root = h('html', { 'data-theme-class': 'dark' });
root.dataset.themeClass = 'dark';
const body = h('body', {}, coverage, cost, history, milestones);
root.appendChild(body);
const tables = [coverage, cost, history, milestones].map((f) => f.querySelector('table'));

// ---- an ECharts that records what it is asked to draw
const charts = [];
class Gradient { constructor(x, y, x2, y2, stops) { this.to = [x, y, x2, y2]; this.stops = stops; } }
class Chart {
  constructor(box) { this.box = box; this.options = []; this.actions = []; this.handlers = {}; this.zoom = null; this.selected = null; }
  setOption(o) {
    this.options.push(o);
    if (o.dataZoom) this.zoom = { startValue: o.dataZoom[0].startValue, endValue: o.dataZoom[0].endValue };
    if (o.legend && o.legend.selected) this.selected = { ...o.legend.selected };
    if (o.xAxis) this.n = (o.xAxis.data || o.yAxis.data).length;
  }
  getOption() { return { dataZoom: this.zoom ? [{ ...this.zoom }] : [] }; }
  dispatchAction(a) {
    this.actions.push(a);
    if (a.type === 'dataZoom' && this.zoom) {
      this.zoom = a.start === 0 && a.end === 100 ? { startValue: 0, endValue: this.n - 1 } : { startValue: a.startValue, endValue: a.endValue };
      this.emit('datazoom', {});
    }
    if (a.type === 'legendToggleSelect') {
      this.selected[a.name] = !this.selected[a.name];
      this.emit('legendselectchanged', { name: a.name, selected: { ...this.selected } });
    }
  }
  on(n, f) { (this.handlers[n] ||= []).push(f); }
  emit(n, p) { for (const f of this.handlers[n] || []) f(p); }
  getZr() { return { on() {}, setCursorStyle() {} }; }
  // a value on one axis, or a [category, value] point of the grid
  convertToPixel(finder, v) { return Array.isArray(v) ? [20 + v[0] * 10, 300 - v[1] * 3] : 300 - v * 3; }
  convertFromPixel() { return [0, 0]; }
  containPixel() { return true; }
  dispose() { this.disposed = true; }
  resize() {}
  get first() { return this.options[0]; }
  get last() { return this.options[this.options.length - 1]; }
}
const echarts = { init(box) { const c = new Chart(box); charts.push(c); return c; }, graphic: { LinearGradient: Gradient } };

// ---- the window the component runs in
function run(reduced) {
  charts.length = 0;
  const listeners = [];
  const motion = { matches: reduced, addEventListener: (n, f) => listeners.push(f) };
  let observed = null;
  const document = {
    documentElement: root, body, currentScript: null, head: h('head'),
    querySelectorAll: (s) => body.querySelectorAll(s), querySelector: (s) => body.querySelector(s),
    createElement: (t) => h(t),
  };
  const window = {
    document, echarts, Promise, console, getComputedStyle,
    matchMedia: (q) => { if (q !== '(prefers-reduced-motion: reduce)') throw new Error(`asked ${q}`); return motion; },
    location: { hash: '', assign() {} }, addEventListener() {},
    requestAnimationFrame: (f) => f(),
    MutationObserver: class { constructor(f) { observed = f; } observe() {} },
  };
  window.window = window;
  vm.runInNewContext(source, window);
  return { window, motion, change: () => listeners.forEach((f) => f()), theme: () => observed() };
}
const settle = () => new Promise((r) => setImmediate(r));

// the page is rebuilt for each run: charts.js inserts its frames into the figures
function strip() {
  for (const f of [coverage, cost, history, milestones]) {
    for (const c of [...f.children]) if (/mj-chart/.test(c.className)) c.remove();
  }
}

let page = run(false);
await settle();
eq(charts.length, 4, 'every figure is drawn');
const byId = Object.fromEntries(charts.map((c) => [c.box.id.replace(/-frame$/, ''), c]));
const api = page.window.mjCharts;
// the marks over a line are held unseen while it draws itself in: no option has lit them, and
// nothing has been set under the drawing line, until ECharts says the drawing has finished
for (const id of ['chart-trends-coverage', 'chart-trends-cost', 'chart-context-history']) {
  const c = byId[id];
  ok(c.options.every((o) => !o.series || o.series.every((x) => !x.markPoint || !x.markPoint.itemStyle || x.markPoint.itemStyle.opacity === 0)),
    `${id} lit a latest value before its line had drawn itself in`);
  ok(c.options.every((o) => !o.series || o.series.every((x) => !x.markLine || x.markLine.lineStyle.opacity === 0)),
    `${id} lit a bridge before its line had drawn itself in`);
}
for (const c of charts) c.emit('finished', {});

// 7. the script leaves every table where it was, shown
tables.forEach((t, i) => { ok(t.parentNode && !t.hidden, `table ${i} was taken off the page`); });
ok(coverage.children[0].tagName === 'FIGCAPTION' && coverage.children[1].className === 'mj-chart-card'
  && coverage.children[1].querySelector('[data-chart-frame]') === byId['chart-trends-coverage'].box
  && coverage.children.indexOf(coverage.children[1]) < coverage.children.indexOf(tables[0].parentNode), 'the chart is drawn in a card under the caption, above its table');
const head = coverage.children[1].children[0];
ok(head.className === 'mj-chart-head' && head.querySelector('.mj-chart-keys') && head.querySelector('.mj-chart-tools'), 'the legend and the Reset row are not the header of the chart\'s card');

// 1. one tooltip at a category
for (const [id, c] of Object.entries(byId)) {
  const o = c.first;
  eq(o.tooltip.trigger, 'axis', `${id} triggers its tooltip on the axis`);
  eq(o.tooltip.axisPointer.type, o.series[0].type === 'line' ? 'line' : 'shadow', `${id} points with a vertical rule on a line, a band on bars`);
  ok(!o.tooltip.axisPointer.crossStyle, `${id} draws a horizontal rule, whose pill says the pointer's height, a value the table does not hold`);
  eq([o.tooltip.backgroundColor, o.tooltip.borderColor, o.tooltip.textStyle.color], [toHex(LIGHT['--mj-raised']), toHex(LIGHT['--mj-line']), toHex(LIGHT['--mj-fg'])], `${id} draws its tooltip in the page's surface, border and text`);
}
const cv = byId['chart-trends-coverage'].first;
ok(cv.tooltip.axisPointer.label.show === true, 'the rule has no pill saying the day');
// as ECharts hands an axis tooltip its items: every series at the category, a line chart's area
// series among them, each with its id
const at = (c, i) => c.first.series.map((s, k) => ({ seriesIndex: k, seriesId: s.id, dataIndex: i }));
// a line chart's own series, and the area series drawn under them
const own = (o) => o.series.filter((s) => /^series-/.test(s.id));
const fills = (o) => o.series.filter((s) => /^area-/.test(s.id));
const lastWith = (c, f) => [...c.options].reverse().find(f);
let html = cv.tooltip.formatter(at(byId['chart-trends-coverage'], 59));
for (const want of ['lines', 'functions', 'regions', '79.67%', '80.67%', 'not recorded', 'run 59']) ok(html.includes(want), `the coverage tooltip at the last run lacks "${want}": ${html}`);
ok((html.match(/regions/g) || []).length === 1, `the tooltip names an area series as data: ${html}`);
// the formatter's input: the area series are taken out before anything reads the table by them
// (tip() reads each item's series of the table, which an area series has none of), so an
// unfiltered input throws here rather than passing quietly
ok(fills(cv).length === 3 && at(byId['chart-trends-coverage'], 59).filter((x) => /^area-/.test(x.seriesId)).length === 3,
  'the formatter was not handed the area series the way ECharts hands them');
const areaOnly = cv.tooltip.formatter(at(byId['chart-trends-coverage'], 59).filter((x) => /^area-/.test(x.seriesId)));
ok(!/lines|functions|regions/.test(areaOnly.replace(/<[^>]*>/g, ' ')), `a tooltip handed only area series names one: ${areaOnly}`);
// a dot on every line measured at the category pointed at, none on the line that recorded nothing
const dots = lastWith(byId['chart-trends-coverage'], (o) => o.series && o.series[0] && /^area-/.test(o.series[0].id) && o.series[0].markPoint && !o.series[0].areaStyle);
eq(dots && dots.series.map((s) => s.markPoint.data.map((d) => d.coord)), [[[59, cov(59, 0)]], [[59, cov(59, 1)]], []], 'the category pointed at has no dot on each line measured there, or one where nothing was');
html = byId['chart-trends-cost'].first.tooltip.formatter(at(byId['chart-trends-cost'], 2));
ok(html.includes('12 min') && html.includes('2 min'), `the cost tooltip is not in minutes: ${html}`);
html = byId['chart-context-history'].first.tooltip.formatter(at(byId['chart-context-history'], 0));
ok(html.includes('2026-10-10 &lt;img src=x&gt;') && !html.includes('<img'), `the tooltip does not escape the table's text: ${html}`);
html = byId['chart-milestones'].first.tooltip.formatter(at(byId['chart-milestones'], 0));
ok(/total[\s\S]*12 issues/.test(html) && html.includes('1 issue<'), `the stacked tooltip lacks its whole or its singular: ${html}`);

// 2. the legend, and colour that follows the column
eq(byId['chart-context-history'].first.legend, undefined, 'one series has no legend');
for (const id of ['chart-trends-coverage', 'chart-trends-cost', 'chart-milestones']) {
  const l = byId[id].first.legend;
  ok(l && l.selectedMode === true && l.show === false && l.data.length === own(byId[id].first).length, `${id} has no selectable legend of every series, kept off the canvas`);
}
const buttons = (f) => f.querySelectorAll('[data-chart-action="series"]');
ok(!buttons(history).length, 'one series has legend buttons');
for (const [f, n] of [[coverage, 3], [cost, 2], [milestones, 5]]) {
  const b = buttons(f);
  ok(b.length === n && b.every((x) => x.tagName === 'BUTTON' && x.getAttribute('aria-pressed') === 'true' && x.getAttribute('data-chart-for') === f.id),
    `${f.id} has no pressed legend button of each series for its chart`);
}
eq(buttons(coverage).map((b) => b.querySelector('.mj-chart-key-name').textContent), ['lines', 'functions', 'regions'], 'a legend button does not name its series');
const values = (f) => f.querySelectorAll('.mj-chart-key-value').map((v) => v.textContent);
ok(buttons(coverage).every((b) => b.querySelector('.mj-chart-key-value').getAttribute('aria-hidden') === 'true'), 'a key\'s value is read out as part of its name');
// the tooltips above were asked for as a pointer asks; the pointer leaves, and the keys rest
for (const c of Object.values(byId)) c.emit('globalout', {});
eq(values(coverage), ['79.67%', '80.67%', '81.33%'], 'a line\'s key does not show its latest recorded value');
eq(values(milestones), ['12', '6', '3', '3', '3'], 'a stack\'s key does not show its column\'s whole');
eq(values(history), ['74%'], 'a lone series has no readout of its latest value');
ok(coverage.querySelector('.mj-chart-keys').classList.contains('mj-chart-keys--line') && !milestones.querySelector('.mj-chart-keys').classList.contains('mj-chart-keys--line'),
  'a line chart\'s keys are not strokes, or a bar chart\'s are');
const colours = (o) => own(o).map((s) => s.itemStyle.color);
eq(colours(cv), ['--mj-series-1', '--mj-series-2', '--mj-series-3'].map((t) => toHex(LIGHT[t])), 'the coverage series are the series tokens of their places');
const ms = byId['chart-milestones'];
eq(colours(ms.first), [toHex(STATUS.done), toHex(STATUS.active), toHex(LIGHT['--mj-muted']), toHex(LIGHT['--mj-muted']), toHex(STATUS.blocked)],
  'the milestone series are their badges\' colours, the two neutral states the neutral gray');
eq(ms.first.series.map((s) => Boolean(s.itemStyle.decal)), [false, false, false, true, false], 'the second neutral state is not hatched, or another is');
ok(milestones.querySelectorAll('[data-chart-hatch]').length === 1 && buttons(milestones)[3].hasAttribute('data-chart-hatch'), 'the hatched state\'s key is not hatched');
// ready and planned are one colour, so the hatch is all that tells them apart: on the bar, on the
// legend's key and on the tooltip's key, and on that one series alone
ok(colours(ms.first)[2] === colours(ms.first)[3] && api.state('chart-milestones').series.map((x) => x.hatched).join() === 'false,false,false,true,false',
  'the two neutral states are not one colour told apart by the hatch alone');
const msTip = ms.first.tooltip.formatter(at(ms, 0));
ok((msTip.match(/repeating-linear-gradient/g) || []).length === 1, `the tooltip does not hatch the planned key, or hatches another: ${msTip}`);
ok(!colours(ms.first).includes(toHex(LIGHT['--mj-line'])), 'a series is drawn in the border colour, which nobody can see');
ok(cv.yAxis.scale === true && byId['chart-milestones'].first.xAxis.scale === false, 'a line\'s axis is free, a bar\'s anchored at zero');
for (const c of Object.values(byId)) { const v = c.first.series[0].type === 'bar' && c.first.xAxis.type === 'value' ? c.first.xAxis : c.first.yAxis; ok(v.min === undefined && v.max === undefined, 'a value axis is pinned to a range it cannot rescale from'); }
const before = api.state('chart-milestones').series.map((s) => s.colour);
buttons(milestones)[4].fire('click');
const st = api.state('chart-milestones');
ok(!st.series[4].visible, 'pressing the legend button of blocked left it visible');
eq(buttons(milestones).map((b) => b.getAttribute('aria-pressed')), ['true', 'true', 'true', 'true', 'false'], 'the legend buttons do not say which series is hidden');
eq(st.series.map((s) => s.colour), before, 'hiding a series repainted the others');
const restack = ms.last.series;
eq(restack[3].data[0].label && restack[3].data[0].label.formatter, '9', 'the new outermost segment of a stack does not say the new whole');
ok(restack.every((s) => !('itemStyle' in s && s.itemStyle.color)), 'hiding a series sent new colours');
// M000 is [5, 2, 1, 1, 3]: with blocked hidden its outermost visible segment is planned
eq(restack[3].data[0].itemStyle.borderRadius, [0, 4, 4, 0], 'the new outermost segment of a stack is not rounded at its end');
eq(restack[2].data[0].itemStyle.borderRadius, 0, 'an inner segment of a stack is rounded');
api.toggle('chart-milestones', 'blocked');
eq(ms.last.series[3].data[0].itemStyle.borderRadius, 0, 'showing blocked again left planned rounded');
eq(buttons(milestones)[4].getAttribute('aria-pressed'), 'true', 'showing blocked again left its button unpressed');

// 3. the zoom, and the keyboard
ok(api.state('chart-trends-coverage').zoomable && cv.dataZoom && cv.dataZoom.map((z) => z.type).join() === 'slider',
  'sixty runs in 400px (more than 400 / 24) offer no slider zoom, or an inside zoom that cancels the page\'s wheel');
ok(!byId['chart-trends-cost'].first.dataZoom, 'four runs in 1280px are zoomable');
ok(!byId['chart-context-history'].first.dataZoom, 'five runs in 400px are zoomable');
ok(!ms.first.dataZoom, 'a bar chart is zoomable');
const reset = coverage.querySelector('[data-chart-action="reset"]');
ok(reset && reset.tagName === 'BUTTON' && reset.getAttribute('data-chart-for') === 'chart-trends-coverage', 'the zoomable chart has no Reset button of its own');
ok(/Reset/.test(reset.textContent) && reset.getAttribute('aria-label'), 'the Reset button has no label');
ok(!coverage.querySelector('.mj-chart-tools').hidden && reset.disabled && reset.hidden,
  'the range row is hidden, or Reset is shown or enabled at the whole range (where it would spend a row on nothing)');
ok(!cost.querySelector('[data-chart-action="reset"]') || cost.querySelector('.mj-chart-tools').hidden, 'a chart that fits shows a Reset');
api.zoom('chart-trends-coverage', 40, 50);
eq(api.state('chart-trends-coverage').window, [40, 50], 'zooming did not narrow the window');
ok(!reset.disabled && !reset.hidden, 'Reset stays disabled or hidden once zoomed');
ok(/run 40 – run 50 · 11 of 60/.test(coverage.querySelector('.mj-chart-range').textContent), 'the row does not say which window is shown');
const frame = coverage.querySelector('[data-chart-frame]');
eq([frame.getAttribute('tabindex'), frame.getAttribute('role')], ['0', 'img'], 'the frame does not take focus');
frame.fire('keydown', { key: 'Home' });
frame.fire('keydown', { key: 'ArrowRight' });
const tipAction = byId['chart-trends-coverage'].actions.filter((a) => a.type === 'showTip').pop();
eq(tipAction.dataIndex, 41, 'Home then ArrowRight did not move the tooltip to the second category of the window');
ok(/run 41: lines .*%, functions .*%, regions .*%/.test(api.state('chart-trends-coverage').tip), 'the live region does not say the tooltip');
// on a phone the tooltip never floats over the plot: it is docked in the header, the range's
// slot saying the category and the keys its values
ok(cv.tooltip.showContent === false && byId['chart-milestones'].first.tooltip.showContent === false
  && byId['chart-trends-cost'].first.tooltip.showContent === true, 'a tooltip floats over a phone\'s plot, or is docked on a wide one');
const dockedRange = coverage.querySelector('.mj-chart-range');
ok(/^run 41/.test(dockedRange.textContent) && dockedRange.hasAttribute('data-chart-pointing'), `the phone's header does not say the category pointed at: ${dockedRange.textContent}`);
// the component keeps the tooltip in its frame (tipPlace()), so that ECharts' own confinement
// cannot pull a tooltip set over the header band back over the plot
ok(cv.tooltip.confine === false && byId['chart-milestones'].first.tooltip.confine === false, 'ECharts confines a tooltip the component places');
frame.fire('keydown', { key: 'End' });
frame.fire('keydown', { key: 'ArrowRight' });
eq(api.state('chart-trends-coverage').window, [41, 51], 'moving past the window did not pan it');
reset.fire('click');
eq(api.state('chart-trends-coverage').window, [0, 59], 'Reset did not give the whole range back');
ok(reset.disabled && reset.hidden, 'Reset stays enabled or shown at the whole range');
ok(/Plus - 0/.test(frame.getAttribute('aria-keyshortcuts')), 'a zoomable frame does not name its zoom keys');
// a key held with ctrl, alt or meta is the browser's: page zoom, Back
for (const mod of ['ctrlKey', 'metaKey', 'altKey']) {
  for (const k of ['-', '=', '0', 'ArrowLeft', 'Home']) {
    let cancelled = false;
    frame.fire('keydown', { key: k, [mod]: true, preventDefault() { cancelled = true; } });
    ok(!cancelled, `${mod} + ${k} on a chart was cancelled, so the browser cannot zoom the page or go Back`);
  }
}
eq(api.state('chart-trends-coverage').window, [0, 59], 'a key held with a modifier zoomed the chart');
// the wheel: a plain turn is the page's; ctrl (a pinch) zooms
let cancelled = false;
frame.fire('wheel', { deltaY: 100, deltaMode: 0, clientX: 200, clientY: 100, preventDefault() { cancelled = true; } });
ok(!cancelled, 'a plain wheel over a zoomable chart was cancelled, so the page stops scrolling under it');
eq(api.state('chart-trends-coverage').window, [0, 59], 'a plain wheel zoomed the chart');
frame.fire('wheel', { deltaY: -200, deltaMode: 0, ctrlKey: true, clientX: 200, clientY: 100, preventDefault() { cancelled = true; } });
ok(cancelled, 'a ctrl + wheel over a zoomable chart was left to the page');
const pinched = api.state('chart-trends-coverage').window;
ok(pinched[1] - pinched[0] < 59, `a ctrl + wheel did not zoom in: ${pinched}`);
reset.fire('click');
// the tooltip is anchored on a series measured at the category; with none, on the axis
const costFrame = cost.querySelector('[data-chart-frame]');
costFrame.fire('keydown', { key: 'Home' });
costFrame.fire('keydown', { key: 'ArrowRight' });
let shown = byId['chart-trends-cost'].actions.filter((a) => a.type === 'showTip').pop();
eq([shown.seriesIndex, shown.dataIndex], [1, 1], 'the tooltip at r1 is anchored on wall, which has no value there');
ok(/^Fri Jan 1, 2027 · 09:30: wall not recorded, queued 1 min$/.test(api.state('chart-trends-cost').tip), `the live region at the second run is wrong: ${api.state('chart-trends-cost').tip}`);
eq(values(cost), ['–', '1'], 'the keys do not show the values of the category pointed at');
api.toggle('chart-trends-cost', 'queued');
costFrame.fire('keydown', { key: 'ArrowRight' });
costFrame.fire('keydown', { key: 'ArrowRight' });
shown = byId['chart-trends-cost'].actions.filter((a) => a.type === 'showTip').pop();
ok(shown.seriesIndex === undefined && Number.isFinite(shown.x) && Number.isFinite(shown.y),
  `with no visible series measured at r3 the tooltip is not placed on the axis: ${JSON.stringify(shown)}`);
api.toggle('chart-trends-cost', 'queued');

// 4. the marks
own(cv).forEach((s, i) => {
  ok(s.lineStyle.width === 2 && s.smooth === false && s.showSymbol === false && s.connectNulls === false, `${s.name} is not a 2px straight line without symbols of its own`);
  ok(!s.areaStyle, `${s.name} fills its own area, which stops square at every gap`);
  ok(s.emphasis.focus !== 'series', `${s.name} is focused by ECharts' emphasis, which the axis tooltip lends every series at once`);
  const a = fills(cv)[i];
  ok(a && a.name === s.name && a.silent === true && a.lineStyle.width === 0 && a.z < s.z, `${s.name} has no silent area series of its own name under it`);
  ok(a.areaStyle.opacity === 0, `${s.name}'s area is filled while no series is focused, so three fills muddy each other`);
  // the fill is the series colour itself at the stylesheet's strength, fading to none: never a mix with the card
  eq(a.areaStyle.color.stops, [{ offset: 0, color: s.itemStyle.color + '29' }, { offset: 1, color: s.itemStyle.color + '00' }], `${s.name}'s area does not fade from its colour at --chart-area-opacity`);
  // a gap is a gap in the fill too: no tint under an interval nothing was recorded in
  ok(a.connectNulls === false, `${s.name}'s area is drawn across its gaps, tinting what was not recorded`);
});
// an area is an amount from its baseline: a lone line on an axis that does not hold zero (the
// history's 70-74%) has none, and is lifted by a glow of its own colour instead
const lone1 = fills(byId['chart-context-history'].first)[0];
const loneLine = own(byId['chart-context-history'].first)[0];
ok(lone1.areaStyle.opacity === 0, 'a lone line on an axis that does not hold zero fills an area down to the axis floor');
// on a light card a glow around an unfilled line reads as a blurred stroke: there a lone line is
// lifted by weight alone (the dark theme's glow is asked below)
ok(loneLine.lineStyle.shadowBlur === 0 && loneLine.lineStyle.width > 2, 'a lone line on a light card is blurred by a glow, or not lifted by weight');
ok(byId['chart-context-history'].first.series.every((x) => !/^area-/.test(x.id) || x.connectNulls === false), 'a lone line\'s area is drawn across its gaps');
// pointing at a key focuses its series: the others fade, its area fills; letting go gives every line back
// the focus is the last option that styles the lines (the dots at the category follow it)
const focus = () => lastWith(byId['chart-trends-coverage'], (o) => o.series && o.series.some((s) => s.lineStyle));
// the others step back by colour and never below 3:1 on the card; their latest values keep a text
// token; the focused line is heavier; the coverage axis (60-81%) holds no zero, so nothing is filled
const lum = (h) => { const c = [1, 3, 5].map((k) => parseInt(h.slice(k, k + 2), 16) / 255).map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4)); return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]; };
const ratio = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };
const card = toHex(LIGHT['--mj-raised']);
const lineColours = own(cv).map((s) => s.itemStyle.color);
buttons(coverage)[1].fire('mouseenter');
let focused = focus();
const strokes = own(focused).map((s) => s.lineStyle);
ok(strokes.every((l) => l.opacity === 1), `a faded line is faded by opacity, which takes it below 3:1: ${strokes.map((l) => l.opacity)}`);
ok(strokes[1].color === lineColours[1] && strokes[0].color !== lineColours[0] && strokes[2].color !== lineColours[2], 'pointing at the functions key did not step the other lines back');
ok([0, 2].every((k) => ratio(strokes[k].color, card) >= 3), `a faded line falls below 3:1 on the card: ${[0, 2].map((k) => ratio(strokes[k].color, card).toFixed(2))}`);
ok(strokes[1].width > strokes[0].width, 'the focused line is no heavier than the ones that stepped back');
const fadedEnds = own(focused).map((s) => s.markPoint);
ok(fadedEnds[0].label.color === toHex(LIGHT['--mj-muted']) && fadedEnds[0].label.opacity === 1 && ratio(fadedEnds[0].itemStyle.color, card) >= 3
  && fadedEnds[1].label.color === toHex(LIGHT['--mj-fg']), 'a faded line\'s latest value is not in the muted text token, or its dot falls below 3:1');
eq(fills(focused).map((s) => s.areaStyle.opacity), [0, 0, 0], 'pointing at the functions key filled an area down to an axis that does not hold zero');
buttons(coverage)[1].fire('mouseleave');
focused = focus();
eq([own(focused).map((s) => s.lineStyle.color), fills(focused).map((s) => s.areaStyle.opacity)], [lineColours, [0, 0, 0]], 'letting go of a key left the lines faded or filled');
// on an axis that holds zero (queued time starts at 0 min) the line focused fills its area alone
const costChart = byId['chart-trends-cost'];
buttons(cost)[1].fire('mouseenter');
eq(fills(lastWith(costChart, (o) => o.series && o.series.some((s) => s.lineStyle))).map((s) => s.areaStyle.opacity), [0, 1], 'focusing queued on an axis from zero did not fill its area alone');
buttons(cost)[1].fire('mouseleave');
// a floating tooltip covers no dot at the category pointed at: in a frame too narrow for it to
// stand beside the crosshair, at run 8, whose dots are in the upper third, it stands lower down
const place = cv.tooltip.position([100, 110], at(byId['chart-trends-coverage'], 8).slice(0, 3), null, null, { contentSize: [180, 110], viewSize: [200, 320] });
const dotsAt = [0, 1, 2].map((k) => [20 + 8 * 10, 300 - cov(8, k) * 3]);
ok(dotsAt.every(([x, y]) => !(x >= place[0] && x <= place[0] + 180 && y >= place[1] && y <= place[1] + 110)), `the tooltip at ${place} covers a dot it describes: ${JSON.stringify(dotsAt)}`);
ok(place[1] >= 0 && place[1] + 110 <= 320 && place[0] >= 0 && place[0] + 180 <= 200, `the tooltip at ${place} runs out of its frame`);
// where every place beside the rule covers a line, the tooltip leaves the plot: laid out as one line
// over the card's header band, as wide as the frame; where a place covers none, it floats there
const costTip = byId['chart-trends-cost'].first.tooltip;
const floatHtml = costTip.formatter(at(byId['chart-trends-cost'], 1));
const tipDom = { innerHTML: floatHtml, style: {}, querySelector(sel) { return this.innerHTML.includes(sel.slice(1)) ? {} : null; },
  get strip() { return this.innerHTML.includes('mj-chart-tip--strip'); },
  get offsetWidth() { return this.strip ? 1280 : 1270; }, get scrollWidth() { return this.strip ? 600 : 1270; },
  get offsetHeight() { return this.strip ? 30 : 300; } };
const stripped = costTip.position([30, 100], at(byId['chart-trends-cost'], 1).slice(0, 2), tipDom, null, { contentSize: [1270, 300], viewSize: [1280, 320] });
ok(stripped[0] === 0 && tipDom.strip && tipDom.style.width === '1280px', `a tooltip that covers a line wherever it floats was not laid over the header band: ${stripped}`);
ok(['wall', 'queued', 'not recorded', '1 min'].every((w) => tipDom.innerHTML.includes(w)), `the header band's tooltip does not say what the floating one does: ${tipDom.innerHTML}`);
tipDom.innerHTML = floatHtml;
Object.defineProperty(tipDom, 'offsetWidth', { get() { return this.strip ? 1280 : 100; } });
Object.defineProperty(tipDom, 'offsetHeight', { get() { return this.strip ? 30 : 40; } });
const floated = costTip.position([400, 100], at(byId['chart-trends-cost'], 1).slice(0, 2), tipDom, null, { contentSize: [100, 40], viewSize: [1280, 320] });
ok(!tipDom.strip && tipDom.style.width === '' && floated[0] >= 400, `a tooltip with a free place beside the rule did not float there: ${floated}`);
// on a phone a bar's name stands on its own line above its bar, so the bar has the whole width
const msY = ms.first.yAxis.axisLabel;
ok(msY.inside === true && msY.align === 'left' && msY.verticalAlign === 'bottom' && msY.formatter === undefined && msY.hideOverlap === false,
  `a phone's bar names are a column beside the bars, or broken over lines: ${JSON.stringify(msY)}`);
ok(ms.first.grid.left <= 8 && ms.first.series.every((s) => s.barMaxWidth <= 16), 'a phone\'s bars do not take the plot\'s width');
eq(milestones.querySelector('[data-chart-frame]').style.height, `${3 * 44 + 40}px`, 'a phone\'s bar rows have no room for a name over each bar');
// a gap is bridged, silently, between the values either side of it, and the header says what the dashes are
const co = byId['chart-trends-cost'].first.series;
eq(co[0].markLine.data, [[{ coord: [0, 10] }, { coord: [2, 12] }]], 'the gap in wall time is not bridged from the value before it to the value after it');
ok(co[0].markLine.silent === true && co[0].markLine.lineStyle.type !== 'solid', 'a bridge is not a dashed and silent line');
// the bridges and the latest values come in once the line has drawn itself in, faint
ok(co[0].markLine.lineStyle.opacity === 0 && co[0].markPoint.itemStyle.opacity === 0, 'a bridge or a latest value is drawn before its line has drawn itself in');
const shownMarks = lastWith(byId['chart-trends-cost'], (o) => o.series && o.series[0] && o.series[0].markLine);
const bridgeOpacity = shownMarks.series[0].markLine.lineStyle.opacity;
ok(bridgeOpacity > 0 && bridgeOpacity < 1 && shownMarks.series[0].markPoint.itemStyle.opacity === 1, `the bridges are not revealed, faint, once the line is drawn: ${bridgeOpacity}`);
eq(co[1].markLine.data, [], 'a line with no gap is bridged');
ok(own(cv).every((s) => s.markLine.data.length === 0), 'a trailing missing value is bridged to nothing');
ok(!cost.querySelector('.mj-chart-gap').hidden && coverage.querySelector('.mj-chart-gap').hidden, 'the not-recorded key is not shown exactly where a bridge is drawn');
// dates are labelled compactly on the axis, in full in the tooltip
const xl = byId['chart-trends-cost'].first.xAxis.axisLabel;
eq(started.map((c, i) => xl.formatter(c, i)), ['Dec 31\n{year|2026}', 'Jan 1\n{year|2027}', '18:00', 'Jan 2'], 'the dated runs are not labelled compactly');
eq(started.map((c, i) => xl.interval(i, c)), [true, true, true, true], 'four runs in 1280px are thinned');
ok(cv.xAxis.axisLabel.formatter === undefined && cv.xAxis.axisLabel.interval === undefined, 'a category that is not a date is relabelled as one');
// the pill says the day alone; the tooltip beside it says it in full with the time, and never twice
eq(byId['chart-trends-cost'].first.tooltip.axisPointer.label.formatter({ axisDimension: 'x', value: started[1], seriesData: [{ dataIndex: 1 }] }), 'Jan 1', 'the crosshair does not name the run briefly');
ok(/Fri Jan 1, 2027 · 09:30/.test(byId['chart-trends-cost'].first.tooltip.formatter(at(byId['chart-trends-cost'], 1))), 'the tooltip does not name the run in full');
eq(own(cv).map((s) => s.markPoint.data[0].coord), [[59, 79.66666666666667], [59, 80.66666666666667], [58, 81.33333333333333]], 'the latest value of each line is not marked');
ok(own(cv).every((s) => s.markPoint.data.length === 1), 'a line measured at every run has a dot besides its latest value');
eq(byId['chart-trends-cost'].first.series[0].markPoint.data.map((d) => d.coord), [[2, 12], [0, 10]],
  'a wall time with no measured neighbour is not dotted, so the chart shows less than its table');
const top = ms.first.series;
eq(top[4].data[0].itemStyle.borderRadius, [0, 4, 4, 0], 'the outermost segment is not rounded at its data end');
eq(top[0].data[0].itemStyle.borderRadius, 0, 'a segment at the baseline is rounded');
ok(top.every((s) => s.itemStyle.borderWidth === 1 && s.itemStyle.borderColor === toHex(LIGHT['--mj-raised']) && s.barMaxWidth > 0), 'stacked segments are not parted by the card\'s colour');
// M001 is [0, 4, 2, 0, 0]: a recorded zero stays in the tooltip and draws no sliver of border
ok(top[0].data[1].value === 0 && top[0].data[1].itemStyle.borderWidth === 0 && top[0].data[1].itemStyle.opacity === 0, 'a recorded zero is drawn as a sliver of its parting borders');
ok(top.every((s) => s.barMinHeight > 0), 'a count of one may be drawn thinner than a mark');
// M000 is [5, 2, 1, 1, 3]: its outermost segment, blocked, is labelled with the whole
eq(top[4].data[0].label && top[4].data[0].label.formatter, '12', 'the outermost segment of a stack does not say the stack\'s whole');
ok(!top[3].data[0].label, 'an inner segment of a stack is labelled');

// 5. motion: the design's duration, and none under reduced motion, followed when it changes
eq([cv.animation, cv.animationDurationUpdate], [true, 300], 'without reduced motion the update is not the design\'s duration');
page.motion.matches = true;
page.change();
ok(Object.values(byId).every((c) => c.last.animation === false && c.last.animationDurationUpdate === 0), 'turning reduced motion on left a chart animating');
eq(api.state('chart-milestones').animation, false, 'the state does not say motion is off');

// 6. a theme change draws again in the other theme's tokens
root.classList.add('dark');
page.theme();
await settle();
ok(Object.values(byId).every((c) => c.disposed), 'a theme change kept the light drawing');
const dark = charts.find((c) => c.box.id === 'chart-trends-coverage-frame' && !c.disposed);
eq(colours(dark.first)[0], toHex(DARK['--mj-series-1']), 'the dark drawing does not take the dark tokens');
eq(dark.first.dataZoom[0].startValue, 0, 'the redraw lost the window');
const darkLone = own(charts.find((c) => c.box.id === 'chart-context-history-frame' && !c.disposed).first)[0];
ok(darkLone.lineStyle.shadowBlur > 0 && darkLone.lineStyle.shadowColor.startsWith(darkLone.itemStyle.color), 'a lone line on the dark card has no glow of its own colour');
// a redraw is no entrance: its marks are lit from the start
ok(own(dark.first).every((s) => s.markPoint.itemStyle.opacity === 1), 'a redraw held its marks back as if it were an entrance');
root.classList.remove('dark');

// 5. drawn without animation from the start when reduced motion is asked for
strip();
page = run(true);
await settle();
ok(charts.length === 4 && charts.every((c) => c.first.animation === false && c.first.animationDuration === 0), 'under reduced motion a chart is drawn animated');
console.log('ok');
NODE
grep -qx ok "$OUT" || { echo "    the component check printed no verdict"; sed 's/^/    | /' "$OUT"; exit 1; }
echo "    pass: one tooltip per category in the unit, a card header with a legend that shows values and repaints nothing, a zoom with its Reset, the marks, honest gaps, dates, reduced motion, the theme, the table without JS"

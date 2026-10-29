# majordomus-covers: none
# A chart on the site is read by pointing at it (I4400, ADR 0152). site/charts.js draws every
# <figure data-chart> from the table it wraps; this case runs it in node over a page shaped
# like the three that carry charts (trends, economics, the plan), with a document small enough
# to write here and an ECharts that records what it is asked to draw, so every answer is the
# component's own and none is the library's. What is decided:
#
#   1. one tooltip at a category: every chart triggers on the axis, under a crosshair on a
#      line (its value label in the figure's unit) and a band on bars; the tooltip names every
#      visible series at the category in the unit, says "not recorded" for a missing value,
#      adds the whole of a stack, and escapes what the table says;
#   2. the legend: two or more series have a row of toggle buttons (one per series, saying
#      with aria-pressed whether it is shown) over a selection ECharts keeps but does not draw,
#      and one series has none; each series is painted in its column's colour (a status badge's,
#      a neutral state's gray, else the series token of its place), and hiding a series repaints
#      none of the others, leaves the value axis free to rescale, and rounds the new outermost
#      segment of a stack;
#   3. the zoom: a line chart whose categories exceed its width / 24 has a slider zoom (no
#      `inside` zoom, which would cancel every wheel over the plot) and a Reset button that is
#      disabled at the whole range, enabled once zoomed, and gives the whole range back; a plain
#      wheel is never cancelled, ctrl + wheel zooms; a chart that fits, and every bar chart, has
#      none; the keyboard moves the tooltip along the categories, anchored on a series measured
#      there (or on the axis when none is), and leaves a key held with ctrl, alt or meta alone;
#   4. the marks: 2px straight lines with symbols only on hover, an area fading from the series
#      colour at the stylesheet's --chart-area-opacity, the latest value marked and a value with
#      no measured neighbour dotted; bars rounded at the data end only, stacked segments parted
#      by the page's colour;
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
const cost = h('figure', { id: 'chart-trends-cost', 'data-chart': 'line', 'data-chart-unit': 'min' },
  h('table', {}, h('thead', {}, h('tr', {}, h('th', {}, 'Run'), h('th', { 'data-series': 'wall' }, 'Wall'), h('th', { 'data-series': 'queued' }, 'Queued'))),
    // wall is not recorded on r1 and r3: the first series has no value there, and r0 stands alone
    h('tbody', {}, ...[0, 1, 2, 3].map((i) => h('tr', {}, h('td', {}, `r${i}`), h('td', {}, i % 2 ? 'not recorded' : String(10 + i)), h('td', {}, String(i)))))));
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
  convertToPixel(finder, v) { return 300 - v * 3; }
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

// 7. the script leaves every table where it was, shown
tables.forEach((t, i) => { ok(t.parentNode && !t.hidden, `table ${i} was taken off the page`); });
ok(coverage.children[0].tagName === 'FIGCAPTION' && coverage.children[1].className === 'mj-chart', 'the chart is drawn under the caption, above its table');

// 1. one tooltip at a category
for (const [id, c] of Object.entries(byId)) {
  const o = c.first;
  eq(o.tooltip.trigger, 'axis', `${id} triggers its tooltip on the axis`);
  eq(o.tooltip.axisPointer.type, o.series[0].type === 'line' ? 'cross' : 'shadow', `${id} points with a crosshair on a line, a band on bars`);
  eq([o.tooltip.backgroundColor, o.tooltip.borderColor, o.tooltip.textStyle.color], [toHex(LIGHT['--mj-raised']), toHex(LIGHT['--mj-line']), toHex(LIGHT['--mj-fg'])], `${id} draws its tooltip in the page's surface, border and text`);
}
const cv = byId['chart-trends-coverage'].first;
eq(cv.tooltip.axisPointer.label.formatter({ axisDimension: 'y', value: 72.456 }), '72.46%', 'the crosshair says its value in the unit');
const at = (c, i) => c.first.series.map((s, k) => ({ seriesIndex: k, dataIndex: i }));
let html = cv.tooltip.formatter(at(byId['chart-trends-coverage'], 59));
for (const want of ['lines', 'functions', 'regions', '79.67%', '80.67%', 'not recorded', 'run 59']) ok(html.includes(want), `the coverage tooltip at the last run lacks "${want}": ${html}`);
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
  ok(l && l.selectedMode === true && l.show === false && l.data.length === byId[id].first.series.length, `${id} has no selectable legend of every series, kept off the canvas`);
}
const buttons = (f) => f.querySelectorAll('[data-chart-action="series"]');
ok(!buttons(history).length, 'one series has legend buttons');
for (const [f, n] of [[coverage, 3], [cost, 2], [milestones, 5]]) {
  const b = buttons(f);
  ok(b.length === n && b.every((x) => x.tagName === 'BUTTON' && x.getAttribute('aria-pressed') === 'true' && x.getAttribute('data-chart-for') === f.id),
    `${f.id} has no pressed legend button of each series for its chart`);
}
eq(buttons(coverage).map((b) => b.textContent), ['lines', 'functions', 'regions'], 'a legend button does not name its series');
ok(coverage.querySelector('.mj-chart-keys').classList.contains('mj-chart-keys--line') && !milestones.querySelector('.mj-chart-keys').classList.contains('mj-chart-keys--line'),
  'a line chart\'s keys are not strokes, or a bar chart\'s are');
const colours = (o) => o.series.map((s) => s.itemStyle.color);
eq(colours(cv), ['--mj-series-1', '--mj-series-2', '--mj-series-3'].map((t) => toHex(LIGHT[t])), 'the coverage series are the series tokens of their places');
const ms = byId['chart-milestones'];
eq(colours(ms.first), [toHex(STATUS.done), toHex(STATUS.active), toHex(LIGHT['--mj-series-4']), toHex(LIGHT['--mj-series-3']), toHex(STATUS.blocked)],
  'the milestone series are their badges\' colours, the two neutral states two series colours from the end');
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
ok(!coverage.querySelector('.mj-chart-tools').hidden && reset.disabled, 'Reset is hidden, or enabled at the whole range');
ok(!cost.querySelector('[data-chart-action="reset"]') || cost.querySelector('.mj-chart-tools').hidden, 'a chart that fits shows a Reset');
api.zoom('chart-trends-coverage', 40, 50);
eq(api.state('chart-trends-coverage').window, [40, 50], 'zooming did not narrow the window');
ok(!reset.disabled, 'Reset stays disabled once zoomed');
ok(/run 40 – run 50 · 11 of 60/.test(coverage.querySelector('.mj-chart-range').textContent), 'the row does not say which window is shown');
const frame = coverage.querySelector('[data-chart-frame]');
eq([frame.getAttribute('tabindex'), frame.getAttribute('role')], ['0', 'img'], 'the frame does not take focus');
frame.fire('keydown', { key: 'Home' });
frame.fire('keydown', { key: 'ArrowRight' });
const tipAction = byId['chart-trends-coverage'].actions.filter((a) => a.type === 'showTip').pop();
eq(tipAction.dataIndex, 41, 'Home then ArrowRight did not move the tooltip to the second category of the window');
ok(/run 41: lines .*%, functions .*%, regions .*%/.test(api.state('chart-trends-coverage').tip), 'the live region does not say the tooltip');
frame.fire('keydown', { key: 'End' });
frame.fire('keydown', { key: 'ArrowRight' });
eq(api.state('chart-trends-coverage').window, [41, 51], 'moving past the window did not pan it');
reset.fire('click');
eq(api.state('chart-trends-coverage').window, [0, 59], 'Reset did not give the whole range back');
ok(reset.disabled, 'Reset stays enabled at the whole range');
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
ok(/^r1: wall not recorded, queued 1 min$/.test(api.state('chart-trends-cost').tip), `the live region at r1 is wrong: ${api.state('chart-trends-cost').tip}`);
api.toggle('chart-trends-cost', 'queued');
costFrame.fire('keydown', { key: 'ArrowRight' });
costFrame.fire('keydown', { key: 'ArrowRight' });
shown = byId['chart-trends-cost'].actions.filter((a) => a.type === 'showTip').pop();
ok(shown.seriesIndex === undefined && Number.isFinite(shown.x) && Number.isFinite(shown.y),
  `with no visible series measured at r3 the tooltip is not placed on the axis: ${JSON.stringify(shown)}`);
api.toggle('chart-trends-cost', 'queued');

// 4. the marks
for (const s of cv.series) {
  ok(s.lineStyle.width === 2 && s.smooth === false && s.showSymbol === false && s.connectNulls === false, `${s.name} is not a 2px straight line with symbols on hover only`);
  ok(s.areaStyle.opacity === 0.16, `${s.name}'s area does not take --chart-area-opacity`);
  eq(s.areaStyle.color.stops, [{ offset: 0, color: s.itemStyle.color }, { offset: 1, color: s.itemStyle.color + '00' }], `${s.name}'s area does not fade from its colour`);
  eq(s.emphasis.focus, 'series', `${s.name} does not focus on hover`);
}
eq(cv.series.map((s) => s.markPoint.data[0].coord), [[59, 79.66666666666667], [59, 80.66666666666667], [58, 81.33333333333333]], 'the latest value of each line is not marked');
ok(cv.series.every((s) => s.markPoint.data.length === 1), 'a line measured at every run has a dot besides its latest value');
eq(byId['chart-trends-cost'].first.series[0].markPoint.data.map((d) => d.coord), [[2, 12], [0, 10]],
  'a wall time with no measured neighbour is not dotted, so the chart shows less than its table');
const top = ms.first.series;
eq(top[4].data[0].itemStyle.borderRadius, [0, 4, 4, 0], 'the outermost segment is not rounded at its data end');
eq(top[0].data[0].itemStyle.borderRadius, 0, 'a segment at the baseline is rounded');
ok(top.every((s) => s.itemStyle.borderWidth === 1 && s.itemStyle.borderColor === toHex(LIGHT['--mj-bg']) && s.barMaxWidth > 0), 'stacked segments are not parted by the page\'s colour');

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
root.classList.remove('dark');

// 5. drawn without animation from the start when reduced motion is asked for
strip();
page = run(true);
await settle();
ok(charts.length === 4 && charts.every((c) => c.first.animation === false && c.first.animationDuration === 0), 'under reduced motion a chart is drawn animated');
console.log('ok');
NODE
grep -qx ok "$OUT" || { echo "    the component check printed no verdict"; sed 's/^/    | /' "$OUT"; exit 1; }
echo "    pass: one tooltip per category in the unit, a legend that repaints nothing, a zoom with its Reset, the marks, reduced motion, the theme, the table without JS"

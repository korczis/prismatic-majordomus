// The chart: one component for every chart on the site (ADR 0152).
//
// A chart is declared, never scripted. A template that already renders derived data as a
// table wraps it in <figure data-chart="bar|line" id="chart-<name>">, marks the header of
// each column to draw with `data-series`, and this file draws the table: the first column
// is the category, every marked column a series. The numbers are read from the cells the
// page shows, so a chart cannot disagree with its table, and there is no second copy of the
// data anywhere, in a script, an attribute or a file.
//
//   data-chart="bar" | "line"   what to draw
//   data-chart-stack            bars are stacked: the series are parts of one whole
//   data-chart-horizontal       categories run down the side (many rows, long names, phones)
//   data-chart-unit="%"         what the values are in, shown on the axis and in the tooltip
//   data-chart-label="..."      what the picture is of, for a reader who cannot see it
//
// A series takes the colour its column header is drawn in when the header carries a status
// (`.mj-badge`, `.mj-status`), so a state is the colour the design declares for that state
// and none is chosen here. A state of no coloured meaning (`--mj-neutral`) is the neutral
// family's gray (`--mj-muted`, which stands 3:1 on the card in both themes): the first plain,
// the second hatched, so two of them are told apart by pattern and not by a colour; a third
// takes a series colour from the end of the series. Any other series takes the series colour
// of its place among the unmarked columns. A colour belongs to the column, never to its rank
// among the series shown, so hiding one series repaints none of the others. Every colour is a
// token of the page, read back from the page.
//
// A chart is read by pointing at it (I4400). It is drawn in a card: a header row with the
// legend on the left (a toggle button per series, with the series' latest value beside its
// name; on a line, the value at the category pointed at) and the zoomed range with its Reset
// (shown while zoomed) on the right, then the plot. Pointing at a category of a line draws a
// vertical rule whose pill says the day where the axis labels are (the label it stands on is
// hidden while it is there, both its lines; no pill says a value, since the pointer's height is
// no value of the table), a dot on every line measured there, and one tooltip naming every
// visible series beside the rule, at a height where it covers none of the lines; where every
// such place would cover one, the tooltip leaves the plot and is laid out as one line over the
// card's header band, so it never stands over a line. The line nearest the pointer is focused
// (heavier; in the dark theme lifted by a glow, which on a light card reads as a blur) and the
// rest step back in colour, never below 3:1 on the card.
// On bars a rounded band lies behind the row, ending with it, and the tooltip stands wherever
// it covers least of the other rows and never the row itself.
// Pressing a legend button hides or shows its series and the axis rescales to what is left. A
// line chart with more categories than its width holds can be zoomed and panned (the slider,
// laid exactly along the plot; a trackpad pinch or ctrl + wheel, shift + wheel, a mouse drag,
// the keyboard) and Reset gives the whole range back; a plain wheel always scrolls the page.
// Categories that are dates are labelled compactly (`Aug 20`, the year under the first label
// of each year, a time only between two labels of one day) and thinned to what the width
// holds; they are drawn one step per record, not to a time scale, so the tooltip names the
// full date and time and which record of how many it is. On a phone (a card narrower than
// NARROW) the latest values are the legend's alone, a bar's row name stands on its own line
// above its bar (so the bar has the plot's whole width and no name is broken), and the tooltip
// never floats over the plot: it is docked in the header, whose range slot names the category
// pointed at while the keys give each series' value there.
//
// A missing value is never drawn as one. A line stops where a value is missing; a measured
// value with no measured neighbour is a dot; between two measured values across a gap a faint
// dashed bridge shows where the line resumes, which is not data: it carries no value, no
// tooltip names it, and the header's `not recorded` key says what it is. Only a lone series,
// or the series pointed at, fills its area, so overlapping fills never muddy the plot, and
// only on a value axis that holds zero: an area is an amount from its baseline, and drawn
// down to an axis floor it would show a change as a magnitude. A fill stops where its line
// stops, so no tint lies under what was not recorded. A recorded zero is in the tooltip and
// draws no mark.
//
// The frame takes focus: the arrow keys, Home and End move along the categories with the
// tooltip following, + and - zoom, 0 resets, Escape lets go; a key held with ctrl, alt or
// meta is the browser's (page zoom, Back) and is left to it. Motion is the design's one
// duration: the chart draws itself in once, in four of them (at most ENTRY); the marks drawn
// over a line (its bridges, its latest value and its label) wait, unseen, until the line has
// actually drawn itself in, and then fade in; nothing moves at all under
// prefers-reduced-motion, followed when it changes, and every mark is there at once.
//
// The table stays on the page. It is the data, and it is what a reader without this script,
// a screen reader and a printout get; the chart is a second view of it. A row whose first
// cell is a link makes its category a link to the same page. A row that carries an id
// (`chart-<name>:<row>`) is addressable: the fragment lands on the row with no script at
// all, and with it the mark is highlighted too.
//
// Apache ECharts is vendored at build (scripts/site-build) and is not in any page's own
// scripts: it is fetched the first time a chart nears the viewport, once per page. It draws
// in SVG, so the plot's text takes the page's font features (tabular figures) and prints sharp.
(function () {
  'use strict';

  var figures = Array.prototype.slice.call(document.querySelectorAll('figure[data-chart]'));
  if (!figures.length || !window.Promise) { return; }

  var script = document.currentScript;
  var echartsSrc = script ? script.getAttribute('data-echarts-src') : null;
  var root = document.documentElement;
  var loading = null;
  var states = [];
  var SERIES_TOKENS = ['--mj-series-1', '--mj-series-2', '--mj-series-3', '--mj-series-4',
                       '--mj-series-5', '--mj-series-6', '--mj-series-7', '--mj-series-8'];
  // The width one category of a line chart needs before its points crowd each other: past
  // width / STEP categories the range becomes zoomable rather than squeezed further.
  var STEP = 24;
  // The width one date label of the category axis is given: labels are thinned to one in
  // every so many categories that this width holds, so they never crowd.
  var LABEL_ROOM = 64;
  // A value label is this many pixels tall; two latest-value labels closer than that are
  // moved apart (the common ECharts build carries no label layout of its own).
  var LABEL_HEIGHT = 15;
  // How far a pinch or a ctrl + wheel turn must travel, in pixels, to double or halve the
  // window: a trackpad sends many small deltas, a mouse wheel few large ones.
  var PINCH = 200;
  // How close, in pixels, the pointer must come to a line for that line to be focused.
  var NEAR = 28;
  // Below this width a chart is laid out for a phone: no latest-value labels beside the lines
  // (the legend shows the latest values), a bar's name on its own line above it, the tooltip
  // docked in the header.
  var NARROW = 480;
  // A phone's bar row: its name (ROW_NAME tall) over its bar (ROW_BAR thick), parted by
  // ROW_PART, in a band ROW tall that keeps the rows apart.
  var ROW = 44;
  var ROW_BAR = 14;
  var ROW_NAME = 14;
  var ROW_PART = 3;
  // How strongly a bridge is drawn, lit: faint, since it is not data.
  var BRIDGE = 0.55;
  // The longest a chart takes to draw itself in, in milliseconds, however slow the design's
  // duration: a dashboard is read, not watched.
  var ENTRY = 700;
  // The smallest a recorded, non-zero bar is drawn, in pixels (4 of them coloured inside the
  // card's parting borders), so a count of one is a mark and never a sliver that reads as a
  // rendering fault.
  var MIN_BAR = 6;
  // Two neighbouring states closer than this in OKLab are told apart by lightness (palette()).
  var SAME = 0.09;
  // A zoom handle: a symbol of HANDLE pixels drawn with a RIM in the card's colour over its
  // edge, so the accent dot a reader sees is HANDLE - RIM (12px) across, parted from the track
  // by the rim, and the target a pointer can take is HANDLE + RIM (24px).
  var HANDLE = 18;
  var RIM = 6;
  // The least contrast a mark keeps on the card, faded or not (WCAG 1.4.11 asks 3:1; a little
  // more, since a colour is rounded to a pixel's).
  var MARK = 3.1;
  // The keys the frame answers, as aria-keyshortcuts names them; a zoomable chart adds its own.
  var KEYS = 'ArrowLeft ArrowRight ArrowUp ArrowDown Home End Escape';
  var ZOOM_KEYS = ' Plus - 0';
  var MONTHS = 'Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec'.split(' ');
  var DAYS = 'Sun Mon Tue Wed Thu Fri Sat'.split(' ');
  var STAMP = new RegExp('^(\\d{4})-(\\d{2})-(\\d{2})(?:[ T](\\d{2}):(\\d{2})' +
    '(?::\\d{2}(?:\\.\\d+)?)?(?:Z|[+-]\\d{2}:?\\d{2})?)?$');
  var motion = window.matchMedia ? window.matchMedia('(prefers-reduced-motion: reduce)') : null;

  function isDark() { return root.classList.contains(root.dataset.themeClass); }
  function still() { return Boolean(motion && motion.matches); }
  function narrow(state) { return (state.box.clientWidth || 0) < NARROW; }
  // Run a step after the one ECharts is in: an option set from inside its own drawing (a
  // tooltip's formatter, an event) is refused, so it waits for the next frame.
  function later(fn) { (window.requestAnimationFrame || window.setTimeout)(fn); }

  // A colour as six-digit sRGB hex, whatever notation the stylesheet declared it in: the
  // browser resolves it and one painted pixel is read back. The same reading
  // site/diagrams.js takes, for the same reason: a canvas library is handed a colour it can
  // parse, and no colour is written in this file.
  var probe = document.createElement('span');
  var canvas = document.createElement('canvas');
  canvas.width = canvas.height = 1;
  var paint = canvas.getContext('2d', { willReadFrequently: true });
  function hex(cssColour) {
    if (!paint || !cssColour) { return ''; }
    paint.save();
    paint.clearRect(0, 0, 1, 1);
    paint.fillStyle = cssColour;
    paint.fillRect(0, 0, 1, 1);
    var px = paint.getImageData(0, 0, 1, 1).data;
    paint.restore();
    return '#' + [px[0], px[1], px[2]].map(function (c) {
      return (c < 16 ? '0' : '') + c.toString(16);
    }).join('');
  }
  // Two colours mixed, a share of the second into the first, as six-digit hex.
  function mix(a, b, share) {
    var p = channels(a), q = channels(b);
    if (!p || !q) { return a; }
    return '#' + p.map(function (c, i) {
      var v = Math.round(c + (q[i] - c) * share);
      return (v < 16 ? '0' : '') + v.toString(16);
    }).join('');
  }
  // A six-digit hex colour's red, green and blue, 0 to 255.
  function channels(colour) {
    if (!colour || colour.length !== 7) { return null; }
    return [1, 3, 5].map(function (i) { return parseInt(colour.slice(i, i + 2), 16); });
  }
  // How far apart two colours look: their distance in OKLab, where 0.02 is about the least a
  // reader notices and SAME is less than a reader tells apart at a glance between two marks.
  function apart(a, b) {
    var p = perceived(a), q = perceived(b);
    if (!p || !q) { return Infinity; }
    return Math.sqrt(Math.pow(p[0] - q[0], 2) + Math.pow(p[1] - q[1], 2) +
      Math.pow(p[2] - q[2], 2));
  }
  // A colour in OKLab: lightness and two opponent axes, where distance is what a reader sees.
  function perceived(colour) {
    var c = channels(colour);
    if (!c) { return null; }
    var v = c.map(function (x) {
      x /= 255;
      return x <= 0.04045 ? x / 12.92 : Math.pow((x + 0.055) / 1.055, 2.4);
    });
    var l = Math.cbrt(0.4122214708 * v[0] + 0.5363325363 * v[1] + 0.0514459929 * v[2]);
    var m = Math.cbrt(0.2119034982 * v[0] + 0.6806995451 * v[1] + 0.1073969566 * v[2]);
    var s = Math.cbrt(0.0883024619 * v[0] + 0.2817188376 * v[1] + 0.6299787005 * v[2]);
    return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
            1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
            0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
  }
  // A colour's relative luminance and two colours' contrast, as WCAG 2.1 measures them.
  function luminance(colour) {
    var c = channels(colour);
    if (!c) { return 0; }
    var v = c.map(function (x) {
      x /= 255;
      return x <= 0.04045 ? x / 12.92 : Math.pow((x + 0.055) / 1.055, 2.4);
    });
    return 0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2];
  }
  function contrast(a, b) {
    var p = luminance(a), q = luminance(b);
    return (Math.max(p, q) + 0.05) / (Math.min(p, q) + 0.05);
  }
  // A colour moved toward a ground as far as it keeps `floor` contrast on it: a mark that
  // steps back and is still a mark. A colour already below the floor is left as it is.
  function toward(colour, ground, floor) {
    if (!channels(colour) || !channels(ground) || contrast(colour, ground) < floor) {
      return colour;
    }
    var lo = 0, hi = 1;
    for (var k = 0; k < 14; k++) {
      var m = (lo + hi) / 2;
      if (contrast(mix(colour, ground, m), ground) >= floor) { lo = m; } else { hi = m; }
    }
    return mix(colour, ground, lo);
  }
  // A series colour as it is drawn while another series is focused: part of the way to the
  // neutral gray, then toward the card as far as it stays a mark (MARK). The colour still
  // names its series; it is quieter by hue and by weight, never by an opacity that would take
  // it below 3:1.
  function recede(t, colour) {
    var quiet = t.muted ? mix(colour, t.muted, 0.4) : colour;
    if (contrast(quiet, t.plot) < MARK) { quiet = colour; }
    return toward(quiet, t.plot, MARK);
  }
  // A token colour at a share of its opacity, as eight-digit hex: a halo, a shadow, a fill.
  function alpha(colour, share) {
    if (!colour || colour.length !== 7) { return colour; }
    var a = Math.round(Math.max(0, Math.min(1, share)) * 255);
    return colour + (a < 16 ? '0' : '') + a.toString(16);
  }
  function token(name) {
    if (!getComputedStyle(root).getPropertyValue(name).trim()) { return ''; }
    probe.style.color = 'var(' + name + ')';
    document.body.appendChild(probe);
    var resolved = getComputedStyle(probe).color;
    probe.remove();
    return hex(resolved);
  }
  // The colour a status column's marks are drawn in: the stylesheet's `--chart-status` on its
  // badge (share/design/figure.css: the status's ink mixed with its border, the family's
  // middle step, which a filled mark needs where the ink is a text colour), read as the badge's
  // page resolves it; the badge's own text colour where the stylesheet declares none.
  function statusColour(marked) {
    if (getComputedStyle(marked).getPropertyValue('--chart-status').trim()) {
      probe.style.color = 'var(--chart-status)';
      marked.appendChild(probe);
      var resolved = getComputedStyle(probe).color;
      probe.remove();
      var mixed = hex(resolved);
      if (mixed) { return mixed; }
    }
    return hex(getComputedStyle(marked).color);
  }
  // A number the stylesheet declares on an element (`--chart-area-opacity` on .mj-chart),
  // or the fallback when the page declares none.
  function declared(element, name, fallback) {
    var value = parseFloat(getComputedStyle(element).getPropertyValue(name));
    return isFinite(value) ? value : fallback;
  }
  // The design's one duration (`--mj-motion-fast`), in milliseconds; none if undeclared.
  function duration() {
    var raw = getComputedStyle(root).getPropertyValue('--mj-motion-fast').trim();
    var value = parseFloat(raw);
    if (!isFinite(value)) { return 0; }
    return /ms$/.test(raw) ? value : value * 1000;
  }
  // How wide a text is drawn in a font, or a fair guess where nothing can measure it.
  function textWidth(text, font) {
    if (paint && typeof paint.measureText === 'function') {
      paint.font = font;
      var width = paint.measureText(text).width;
      if (isFinite(width) && width > 0) { return width; }
    }
    return text.length * 7;
  }

  function usable(lib) { return Boolean(lib) && typeof lib.init === 'function'; }

  function loadECharts() {
    // known by what it can do: an element with id="echarts" is `window.echarts` too
    if (usable(window.echarts)) { return Promise.resolve(window.echarts); }
    if (loading) { return loading; }
    loading = new Promise(function (resolve, reject) {
      if (!echartsSrc) { reject(new Error('no ECharts build is named on this page')); return; }
      var s = document.createElement('script');
      s.src = echartsSrc;
      s.onload = function () {
        if (usable(window.echarts)) { resolve(window.echarts); return; }
        reject(new Error('the ECharts build defined nothing'));
      };
      s.onerror = function () { reject(new Error('the ECharts build did not load')); };
      document.head.appendChild(s);
    });
    return loading;
  }

  // A cell's number: its `data-value` when the template gave the unrounded one, else the
  // number its text starts with. `96.2%` is 96.2, `1,234` is 1234, a dash is no value.
  function number(cell) {
    var raw = cell.getAttribute('data-value');
    if (raw == null) { raw = cell.textContent; }
    var match = /-?\d[\d,]*(?:\.\d+)?(?:e-?\d+)?/i.exec(String(raw).replace(/−/g, '-'));
    if (!match) { return null; }
    var value = Number(match[0].replace(/,/g, ''));
    return isFinite(value) ? value : null;
  }

  // A value as a reader reads it, in the figure's unit. A cell may carry the unrounded value;
  // the chart shows at most two decimals, and a missing value says so.
  function amount(value, unit) {
    if (value == null || !isFinite(value)) { return 'not recorded'; }
    var digits = Math.abs(value) >= 100 ? 1 : 2;
    var text = Number(value.toFixed(digits))
      .toLocaleString('en', { maximumFractionDigits: digits });
    if (!unit) { return text; }
    if (unit === '%') { return text + '%'; }
    // one of a counted unit is singular: `1 issue`, not `1 issues`
    return text + ' ' + (Math.abs(value) === 1 && /[^s]s$/.test(unit) ? unit.slice(0, -1) : unit);
  }

  // Everything put into the tooltip is text from the table, so it is escaped: a series or a
  // category named `<b>` is shown as written, never parsed.
  var ENTITIES = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };
  function escapeHtml(text) {
    return String(text).replace(/[&<>"']/g, function (c) { return ENTITIES[c]; });
  }

  // ------------------------------------------------------------------ dates

  // A category that is a date (`2026-08-20`, `2026-08-20 14:05`, an ISO stamp), or null.
  function stamp(text) {
    var m = STAMP.exec(text);
    if (!m || Number(m[2]) < 1 || Number(m[2]) > 12) { return null; }
    return { year: Number(m[1]), month: Number(m[2]) - 1, day: Number(m[3]),
             time: m[4] ? m[4] + ':' + m[5] : '' };
  }
  // Every category as a date, or null when any one is not: a chart is labelled as dates
  // only when all of its categories are dates.
  function stamps(categories) {
    var out = [];
    for (var i = 0; i < categories.length; i++) {
      var s = stamp(categories[i]);
      if (!s) { return null; }
      out.push(s);
    }
    return out;
  }
  function sameDay(a, b) {
    return a.year === b.year && a.month === b.month && a.day === b.day;
  }
  function dayText(s) { return MONTHS[s.month] + ' ' + s.day; }
  // a time is worth showing on an axis only when it is one (a date written at 00:00 is a day)
  function clock(s) { return s.time && s.time !== '00:00' ? s.time : ''; }
  // A category as the tooltip and the live region name it: in full, with the time recorded.
  function heading(state, i) {
    var s = state.dates && state.dates[i];
    if (!s) { return i >= 0 ? state.data.categories[i] : ''; }
    var weekday = DAYS[new Date(Date.UTC(s.year, s.month, s.day)).getUTCDay()];
    return weekday + ' ' + dayText(s) + ', ' + s.year + (s.time ? ' · ' + s.time : '');
  }
  // A category in a few characters: the slider's labels, the range's.
  function brief(state, i, year) {
    var s = state.dates && state.dates[i];
    if (!s) { return state.data.categories[i] || ''; }
    return dayText(s) + (year ? ', ' + s.year : '') + (clock(s) ? ' ' + clock(s) : '');
  }
  // The crosshair's pill: the day alone. The tooltip beside it (or the header, on a phone)
  // says the date in full with its time, and two labels never say the same thing.
  function day(state, i) {
    var s = state.dates && state.dates[i];
    return s ? dayText(s) : state.data.categories[i] || '';
  }
  // A category as the header says it on a phone, where the tooltip is docked: one line, the
  // date with its year and time, and which record of how many it is.
  function docked(state, i) {
    var s = state.dates && state.dates[i], n = state.data.categories.length;
    if (!s) { return state.data.categories[i] || ''; }
    return dayText(s) + ', ' + s.year + (s.time ? ' · ' + s.time : '') + ' · ' + (i + 1) +
      ' of ' + n;
  }
  // How many categories one date label stands for, so the labels the plot holds never crowd.
  function labelStep(state) {
    var w = state.window || whole(state);
    var room = Math.max(1, Math.floor(plotWidth(state) / LABEL_ROOM));
    return Math.max(1, Math.ceil((w[1] - w[0] + 1) / room));
  }
  // The category an axis label stands for. ECharts counts a label from the start of the
  // window it shows, so the label's own text finds its place in the whole range: first from
  // the window this file last saw (two runs may share a time), then anywhere, since a zoom is
  // drawn before its window is known here.
  function labelIndex(state, value, index) {
    var c = state.data.categories, w = state.window || whole(state);
    var k = c.indexOf(String(value), w[0]);
    if (k < 0) { k = c.indexOf(String(value)); }
    return k >= 0 ? k : Math.min(c.length - 1, w[0] + index);
  }
  // A date label: the day, the year under the first label of each year, and the time
  // instead of the day only between two labels of the same day. Labels sit on every
  // step-th category counted from the first of the whole range, so panning slides them and
  // never reshuffles.
  function dateLabel(state, i) {
    var step = labelStep(state), w = state.window || whole(state);
    var first = Math.ceil(w[0] / step) * step;
    var s = state.dates[i], prev = i - step >= first ? state.dates[i - step] : null;
    if (!s) { return state.data.categories[i] || ''; }
    var top = prev && sameDay(prev, s) && clock(s) ? clock(s) : dayText(s);
    return !prev || prev.year !== s.year ? top + '\n{year|' + s.year + '}' : top;
  }

  // ------------------------------------------------------------------ the table

  // The table as the chart reads it: categories, the marked series and each row's link.
  function read(figure) {
    var table = figure.querySelector('table');
    if (!table || !table.tHead || !table.tBodies.length) { return null; }
    var heads = Array.prototype.slice.call(table.tHead.rows[table.tHead.rows.length - 1].cells);
    var series = [];
    var slot = 0;
    heads.forEach(function (th, column) {
      if (!th.hasAttribute('data-series')) { return; }
      // a status column keeps its element, so its colour is read again in the other theme;
      // any other column keeps its place among the unmarked ones, and that is its colour
      var marked = th.querySelector('.mj-badge, .mj-status');
      series.push({
        column: column,
        name: th.getAttribute('data-series') || th.textContent.trim(),
        marked: marked,
        slot: marked ? -1 : slot++,
        data: []
      });
    });
    if (!series.length) { return null; }
    var categories = [], links = [], ids = [];
    Array.prototype.forEach.call(table.tBodies[0].rows, function (row) {
      var first = row.cells[0];
      if (!first) { return; }
      var anchor = first.querySelector('a[href]');
      var name = first.getAttribute('data-category') ||
        first.textContent.trim().replace(/\s+/g, ' ');
      categories.push(name);
      links.push(anchor ? anchor.href : null);
      ids.push(row.id || null);
      series.forEach(function (s) {
        s.data.push(row.cells[s.column] ? number(row.cells[s.column]) : null);
      });
    });
    if (!categories.length) { return null; }
    return { categories: categories, series: series, links: links, ids: ids };
  }

  // What a figure declares, read once.
  function shape(figure) {
    var kind = figure.getAttribute('data-chart') === 'line' ? 'line' : 'bar';
    return {
      kind: kind,
      horizontal: figure.hasAttribute('data-chart-horizontal') && kind === 'bar',
      stacked: figure.hasAttribute('data-chart-stack'),
      unit: figure.getAttribute('data-chart-unit') || ''
    };
  }

  // The page's colours, read again on every draw, so a theme change is a redraw.
  function palette(state) {
    var t = {
      fg: token('--mj-fg'), body: token('--mj-body'), muted: token('--mj-muted'),
      faint: token('--mj-faint'), line: token('--mj-line'), bg: token('--mj-bg'),
      raised: token('--mj-raised'), sunken: token('--mj-sunken'), accent: token('--mj-accent'),
      accentSoft: token('--mj-accent-soft'), accentFill: token('--mj-accent-fill'),
      series: SERIES_TOKENS.map(token).filter(Boolean),
      area: declared(state.box, '--chart-area-opacity', 0),
      // the fill under a line focused among others: lighter, since a gap in it is a gap in
      // a tint the rest of the plot does not carry
      focusArea: declared(state.box, '--chart-focus-area-opacity', -1),
      font: getComputedStyle(document.body).fontFamily
    };
    // the card the plot is drawn on (.mj-chart-card is `--mj-raised`, as the site's cards are):
    // a stack's segments are parted by it and a hovered point is ringed in it
    t.plot = t.raised || t.bg;
    if (t.focusArea < 0) { t.focusArea = t.area; }
    // The dark card swallows a tint, so the dark theme declares stronger fills of its own
    // (`--chart-area-opacity-dark`, `--chart-focus-area-opacity-dark`); where it declares none,
    // the light ones half as strong again.
    if (isDark()) {
      t.area = declared(state.box, '--chart-area-opacity-dark', t.area * 1.5);
      t.focusArea = declared(state.box, '--chart-focus-area-opacity-dark', t.focusArea * 1.5);
    }
    // a shadow is the page's ink in the light theme and its darkest ground in the dark one
    t.shadow = isDark() ? t.bg : t.fg;
    var neutral = token('--mj-neutral'), quiet = 0;
    var coloured = state.data.series.map(function () { return false; });
    t.hatch = state.data.series.map(function () { return false; });
    t.colours = state.data.series.map(function (s, i) {
      if (s.marked) {
        // a state's meaning is told by its ink: the neutral one is the neutral family's
        if (hex(getComputedStyle(s.marked).color) !== neutral) {
          coloured[i] = true;
          return statusColour(s.marked);
        }
        // a state of no coloured meaning: the neutral gray, the second hatched (a bar has a
        // pattern to carry; a line has none, so its second takes a series colour)
        quiet += 1;
        var gray = t.muted && (quiet === 1 || (quiet === 2 && state.shape.kind === 'bar'));
        if (gray) { t.hatch[i] = quiet === 2; return t.muted; }
        if (!t.series.length) { return t.muted; }
        var n = quiet - (state.shape.kind === 'bar' ? 2 : 1);
        return t.series[(t.series.length - n % t.series.length) % t.series.length];
      }
      return t.series.length ? t.series[s.slot % t.series.length] : t.fg;
    });
    // Two states drawn side by side (neighbouring columns, so neighbouring segments of a
    // stack) whose colours a reader cannot tell apart at a glance are told apart by lightness:
    // the second steps a quarter of the way toward the page's ink, which keeps its family and
    // only raises its contrast on the card, in either theme. Decided by the columns' order,
    // never by what is shown, so hiding a series repaints none of the others.
    for (var k = 1; k < t.colours.length; k++) {
      if (coloured[k] && coloured[k - 1] && t.fg && apart(t.colours[k - 1], t.colours[k]) < SAME) {
        t.colours[k] = mix(t.colours[k], t.fg, 0.24);
      }
    }
    // each colour as it is drawn while another series is focused (recede())
    t.receded = t.colours.map(function (c) { return recede(t, c); });
    // the zoom's track outside the window: the quiet ink lightened toward the card as far as
    // it keeps 3:1 there, so the range that is not shown is still a visible part of the control
    t.track = toward(t.faint || t.muted, t.plot, MARK);
    // the band behind a bar's row: the card, a step toward the ink
    t.band = mix(t.plot, t.fg, isDark() ? 0.07 : 0.05);
    return t;
  }

  function visible(state, i) {
    return state.selected[state.data.series[i].name] !== false;
  }
  function lastIndex(data) {
    for (var i = data.length - 1; i >= 0; i--) { if (data[i] != null) { return i; } }
    return -1;
  }

  // Whether a line chart holds more categories than its width does: then it is zoomable.
  function crowded(state) {
    if (state.shape.kind !== 'line') { return false; }
    var width = state.box.clientWidth || 0;
    return width > 0 && state.data.categories.length > Math.floor(width / STEP);
  }
  function whole(state) { return [0, state.data.categories.length - 1]; }
  function full(state) {
    var w = state.window;
    return !w || (w[0] <= 0 && w[1] >= state.data.categories.length - 1);
  }
  // The plot's width, near enough to thin the labels by: the frame less its value axis and,
  // on a line, the room of the latest-value labels.
  function plotWidth(state) {
    var right = state.shape.kind === 'line' ? endRoom(state) : 16;
    return Math.max(0, (state.box.clientWidth || 0) - right - 48);
  }

  // ------------------------------------------------------------------ bars

  // A bar's rounded end is the one away from the baseline, and in a stack only the outermost
  // visible segment of its sign has one: the rest meet square, the card showing between them.
  function radius(state, i, row) {
    var value = state.data.series[i].data[row];
    if (value == null || value === 0) { return 0; }
    if (state.shape.stacked) {
      for (var k = state.data.series.length - 1; k > i; k--) {
        var above = state.data.series[k].data[row];
        if (visible(state, k) && above != null && above !== 0 && (above > 0) === (value > 0)) {
          return 0;
        }
      }
    }
    var r = 4;
    if (state.shape.horizontal) { return value > 0 ? [0, r, r, 0] : [r, 0, 0, r]; }
    return value > 0 ? [r, r, 0, 0] : [0, 0, r, r];
  }
  // The whole of a stack at one category: the sum of what its visible series recorded.
  function stackTotal(state, row) {
    var sum = 0, any = false;
    state.data.series.forEach(function (s, k) {
      if (visible(state, k) && s.data[row] != null) { sum += s.data[row]; any = true; }
    });
    return any ? sum : null;
  }
  // Each bar, rounded at its data end; the outermost segment of a stack is labelled with the
  // stack's whole, quietly, past its end.
  function bars(state, i) {
    var t = state.tokens;
    return state.data.series[i].data.map(function (value, row) {
      if (value == null) { return null; }
      // a recorded zero is in the tooltip and has no mark: its parting borders alone would
      // draw a sliver where there is nothing
      if (value === 0) { return { value: 0, itemStyle: { borderWidth: 0, opacity: 0 } }; }
      var r = radius(state, i, row);
      var item = { value: value, itemStyle: { borderRadius: r } };
      if (state.shape.stacked && r !== 0 && value > 0 && t) {
        item.label = {
          show: true, position: state.shape.horizontal ? 'right' : 'top', distance: 6,
          color: t.muted, fontFamily: t.font, fontSize: 11, fontWeight: 500,
          formatter: amount(stackTotal(state, row), '')
        };
      }
      return item;
    });
  }

  // ------------------------------------------------------------------ lines

  // The measured values of a line that the window shows with no measured neighbour beside
  // them: a line has no segment there and draws symbols only under the pointer, so without a
  // dot of their own the chart would show less than its table.
  function lone(data, w, skip) {
    var out = [];
    for (var k = w[0]; k <= w[1]; k++) {
      if (data[k] == null || k === skip) { continue; }
      var before = k > w[0] && data[k - 1] != null;
      var after = k < w[1] && data[k + 1] != null;
      if (!before && !after) { out.push(k); }
    }
    return out;
  }

  // Whether a line steps back because another is focused.
  function dimmed(state, i) {
    return state.focused != null && state.focused !== i && state.data.series.length > 1;
  }

  // Whether the value axis a line is drawn on holds zero. An area encodes an amount from its
  // baseline, so it is filled only down to a zero the axis shows: on an axis that starts at
  // 60%, a fill would draw a rise from 61% to 84% as growth from nothing. Asked of the axis as
  // ECharts laid it out; before the first drawing, of the values the window shows.
  function grounded(state) {
    var extent = null;
    try {
      extent = state.chart.getModel().getComponent('yAxis').axis.scale.getExtent();
    } catch (e) {
      extent = null;
    }
    if (extent && isFinite(extent[0]) && isFinite(extent[1])) {
      return extent[0] <= 0 && extent[1] >= 0;
    }
    var w = state.window || whole(state), low = Infinity, high = -Infinity;
    state.data.series.forEach(function (s, i) {
      if (!visible(state, i)) { return; }
      for (var k = w[0]; k <= w[1]; k++) {
        if (s.data[k] != null) { low = Math.min(low, s.data[k]); high = Math.max(high, s.data[k]); }
      }
    });
    return low <= 0 && high >= 0;
  }
  // How much of a line's area is drawn (its gradient carries the strength): all of it under a
  // lone line or the line focused among others, while the axis holds zero; else none.
  function filled(state, i) {
    var t = state.tokens;
    if (!t || t.area <= 0 || !visible(state, i) || !grounded(state)) { return 0; }
    return state.data.series.length < 2 || state.focused === i ? 1 : 0;
  }
  // A line's stroke: its colour, or its receded colour while another is focused. The line
  // focused among others, or a lone line, says "this one" without claiming an area the axis
  // does not ground: in the dark theme by a soft glow of its own colour (a lone line keeps its
  // weight, a focused one is heavier too), and in the light theme by weight alone, since on a
  // white card a glow around an unfilled line reads as a blurred stroke, not as emphasis.
  function stroke(state, i) {
    var t = state.tokens, colour = t.colours[i], many = state.data.series.length > 1;
    var lifted = !many || state.focused === i, glow = lifted && isDark();
    return {
      width: lifted && (many || !isDark()) ? 2.5 : 2,
      color: dimmed(state, i) ? t.receded[i] : colour, opacity: 1, cap: 'round', join: 'round',
      shadowBlur: glow ? 8 : 0, shadowColor: alpha(colour, glow ? 0.35 : 0)
    };
  }

  // The latest value of each visible line: a dot on it with a soft halo of its colour and
  // its value beside it, while the window shows it. Labels closer than one line of text are
  // moved apart, in their order. A lone value (above) is a smaller dot with no label. While
  // another line is focused, a line's dot takes its receded colour and its value the muted
  // ink (a text token, 4.5:1 on the card), so both stay readable. On a phone, where the
  // labels are the legend's, the dots are smaller and their halos tighter, so two latest
  // values a few pixels apart read as two dots, each in its ring, and never as one blot.
  function ends(state, t) {
    var w = state.window || whole(state);
    var marks = state.data.series.map(function (s, i) {
      var at = lastIndex(s.data);
      var shown = at >= 0 && visible(state, i) && at >= w[0] && at <= w[1];
      return { i: i, at: at, value: shown ? s.data[at] : null, shown: shown, y: 0, dy: 0,
               lone: visible(state, i) ? lone(s.data, w, at) : [] };
    });
    var placed = marks.filter(function (m) { return m.shown; });
    // pixels exist only once the chart has an option to place them by
    if (state.drawn && placed.length > 1) {
      placed.forEach(function (m) {
        var y = state.chart.convertToPixel({ yAxisIndex: 0 }, m.value);
        m.y = isFinite(y) ? y : 0;
      });
      placed.sort(function (a, b) { return a.y - b.y; });
      for (var k = 1; k < placed.length; k++) {
        var floor = placed[k - 1].y + placed[k - 1].dy + LABEL_HEIGHT;
        if (placed[k].y < floor) { placed[k].dy = floor - placed[k].y; }
      }
    }
    // on a phone the legend says the latest values and the plot keeps the width
    var labelled = !narrow(state), lit = state.lit;
    var unit = state.shape.unit === '%' ? '%' : '';
    return marks.map(function (m) {
      var dim = dimmed(state, m.i);
      var colour = dim ? t.receded[m.i] : t.colours[m.i];
      var data = m.shown ? [{ coord: [m.at, m.value], value: m.value }] : [];
      var values = state.data.series[m.i].data;
      m.lone.forEach(function (k) {
        data.push({ coord: [k, values[k]], value: values[k], symbolSize: 6,
                    itemStyle: { borderColor: t.plot, borderWidth: 1, shadowBlur: 0 },
                    label: { show: false } });
      });
      return Object.assign({
        silent: true, symbol: 'circle', symbolSize: labelled ? 8 : 7,
        // the dot sits in a ring of the card and a soft halo of its own colour
        itemStyle: { color: colour, borderColor: t.plot, borderWidth: labelled ? 2 : 1.5,
                     opacity: lit, shadowBlur: dim ? 0 : labelled ? 10 : 4,
                     shadowColor: alpha(colour, dim ? 0 : labelled ? 0.55 : 0.4) },
        label: {
          show: labelled, position: 'right', distance: 8, offset: [0, m.dy], opacity: lit,
          color: dim ? t.muted : t.fg, fontFamily: t.font, fontSize: 12,
          fontWeight: dim ? 500 : 600,
          // a figure without its unit, as the legend gives it; the axis names the unit
          formatter: function () { return amount(m.value, unit); }
        },
        emphasis: { disabled: true },
        data: data
      }, after());
    });
  }
  // The marks drawn over a line (its latest value and label, its bridges, a lone value's dot)
  // come in once the line has drawn itself in, never before it: a bridge or a dot drawn ahead
  // of its line is a fragment floating where there is no line yet. ECharts delays no marker's
  // entrance and a timer cannot know when the browser finished drawing (a busy page draws
  // late, and an option set under a line drawing in slows it down), so the marks are drawn
  // from the start at no opacity (`state.lit`), nothing is set under the line while it draws,
  // and they are lit when ECharts says the drawing has finished (its `finished` event, after
  // the line's own animation, however long it took): faded in over two of the design's
  // durations, frame by frame, since ECharts fades no marker's opacity. Under reduced motion
  // and on every drawing after the first they are lit from the start.
  function after() {
    var speed = still() ? 0 : duration() * 2;
    return { animation: !still(), animationDuration: speed, animationDurationUpdate: speed };
  }
  function reveal(state) {
    if (state.revealed || !state.chart) { return; }
    state.revealed = true;
    // bars carry no marks over them
    if (state.shape.kind !== 'line') { state.lit = 1; return; }
    state.lit = 0;
    // the marks are placed once, where the drawn chart puts them, and then lit
    relabel(state);
    var steps = still() ? 1 : Math.max(1, Math.round(duration() * 2 / 16)), k = 0;
    var step = function () {
      if (!state.chart || state.lit >= 1) { return; }
      k += 1;
      state.lit = k >= steps ? 1 : 1 - Math.pow(1 - k / steps, 3);
      state.chart.setOption({ series: lights(state) });
      if (k < steps) { later(step); }
    };
    later(step);
  }
  // The opacity of the marks over each line, and nothing else, for one step of their fade.
  function lights(state) {
    return state.data.series.map(function (s, k) {
      return { id: 'series-' + k,
               markPoint: { itemStyle: { opacity: state.lit }, label: { opacity: state.lit } },
               markLine: { lineStyle: { opacity: BRIDGE * state.lit } } };
    });
  }
  // How long the chart takes to draw itself in: four of the design's durations, at most ENTRY,
  // only the first time it is drawn (a redraw in the other theme or at another width is not
  // an entrance), and none under reduced motion.
  function entering(state) {
    if (still() || state.entered) { return 0; }
    return Math.min(ENTRY, duration() * 4);
  }

  // The bridges of a line across its gaps: from the last measured value before a run of
  // missing ones to the first after it, while the window shows both ends. Faint and dashed,
  // silent, and part of no tooltip: it says where the line resumes, not what was measured.
  function bridges(state, t, i) {
    var w = state.window || whole(state);
    var values = state.data.series[i].data, data = [], from = -1;
    if (visible(state, i)) {
      for (var k = 0; k < values.length; k++) {
        if (values[k] == null) { continue; }
        if (from >= 0 && k - from > 1 && from >= w[0] && k <= w[1]) {
          data.push([{ coord: [from, values[from]] }, { coord: [k, values[k]] }]);
        }
        from = k;
      }
    }
    return Object.assign({
      silent: true, symbol: ['none', 'none'],
      lineStyle: { color: dimmed(state, i) ? t.receded[i] : t.colours[i], width: 1.5,
                   type: [2, 4], cap: 'round', opacity: BRIDGE * state.lit },
      label: { show: false }, emphasis: { disabled: true }, tooltip: { show: false },
      data: data
    }, after());
  }

  // The room right of the plot: the widest latest-value label as it is drawn, or, on a phone,
  // where there are none, the end dot's halo.
  function endRoom(state) {
    if (narrow(state)) { return 14; }
    var widest = 0, unit = state.shape.unit === '%' ? '%' : '';
    var font = '600 12px ' + getComputedStyle(document.body).fontFamily;
    state.data.series.forEach(function (s) {
      var at = lastIndex(s.data);
      if (at >= 0) { widest = Math.max(widest, textWidth(amount(s.data[at], unit), font)); }
    });
    return Math.ceil(20 + widest);
  }

  // The pills of the crosshair stand where the axis labels are, and an axis label under one
  // is hidden while it is there (the pill says the value at that place, more exactly), so no
  // two texts are ever drawn over each other. Found by geometry in the drawn SVG: a pill is a
  // text in the card's colour (on the ink), everything else a text that may be under it.
  function veil(state) {
    unveil(state);
    var svg = state.box.querySelector && state.box.querySelector('svg');
    if (!svg || !state.tokens || state.shape.kind !== 'line') { return; }
    var texts = Array.prototype.slice.call(svg.getElementsByTagName('text'));
    var pills = [], rest = [];
    texts.forEach(function (el) {
      var fill = el.getAttribute('fill') || '';
      var box = el.getBoundingClientRect();
      if (!box.width) { return; }
      (fill && hex(fill) === state.tokens.plot ? pills : rest).push({ el: el, box: box });
    });
    if (!pills.length) { return; }
    var hide = function (r) {
      r.el.style.opacity = '0';
      state.veiled.push(r.el);
      r.hidden = true;
    };
    var gone = rest.filter(function (r) {
      return pills.some(function (p) {
        return r.box.left < p.box.right + 8 && r.box.right > p.box.left - 8 &&
          r.box.top < p.box.bottom + 3 && r.box.bottom > p.box.top - 3;
      });
    });
    gone.forEach(hide);
    // A date label of two lines (the day, the year under it) is drawn as two texts: the line
    // stacked on a hidden one, centred on it, goes with it, so no year is left under the axis
    // with no date above it.
    rest.forEach(function (r) {
      if (r.hidden) { return; }
      var mate = gone.some(function (g) {
        var centred = Math.abs((r.box.left + r.box.right) / 2 - (g.box.left + g.box.right) / 2);
        var stacked = Math.min(Math.abs(r.box.top - g.box.bottom),
          Math.abs(g.box.top - r.box.bottom));
        return centred < 6 && stacked < 8;
      });
      if (mate) { hide(r); }
    });
  }
  function unveil(state) {
    (state.veiled || []).forEach(function (el) { el.style.opacity = ''; });
    state.veiled = [];
  }

  // The dots at the category pointed at: one on each visible line measured there, in the
  // line's colour, ringed in the card's and lifted by a soft halo of its own, so it reads on
  // both grounds. Drawn as the marks of each line's area series (never ECharts' hover symbol,
  // which the light theme's white ring swallowed), and none while nothing is pointed at.
  function cursor(state) {
    if (!state.chart || state.shape.kind !== 'line' || state.cursoring || !state.revealed) {
      return;
    }
    state.cursoring = true;
    later(function () {
      state.cursoring = false;
      if (!state.chart || !state.drawn || !state.tokens) { return; }
      var t = state.tokens, at = state.pointing;
      state.chart.setOption({ series: state.data.series.map(function (s, i) {
        var v = at == null ? null : s.data[at];
        var dim = dimmed(state, i);
        return { id: 'area-' + i, markPoint: {
          silent: true, symbol: 'circle', symbolSize: dim ? 8 : 10, animation: false,
          itemStyle: { color: dim ? t.receded[i] : t.colours[i], borderColor: t.plot,
                       borderWidth: dim ? 2 : 2.5, shadowBlur: dim ? 0 : 8,
                       shadowColor: alpha(t.colours[i], dim ? 0 : 0.6), opacity: 1 },
          label: { show: false }, emphasis: { disabled: true },
          data: v == null || !visible(state, i) ? [] : [{ coord: [at, v] }]
        } };
      }) });
    });
  }

  // The grid ECharts laid out, as it laid it out (it moves the plot's edges to hold the axis
  // labels of the window shown), or null where the build does not say.
  function gridBox(state) {
    try {
      var r = state.chart.getModel().getComponent('grid').coordinateSystem.getRect();
      return isFinite(r.x) && r.width > 0
        ? { left: r.x, right: r.x + r.width, top: r.y, bottom: r.y + r.height } : null;
    } catch (e) {
      return null;
    }
  }
  // The plot's box in the frame's pixels, once it is drawn: the slider is laid along it and
  // a tooltip kept clear of its axis.
  function plotBox(state) {
    var c = state.chart, w = state.window || whole(state);
    if (!c || !state.drawn) { return null; }
    var v = state.shape.horizontal ? { yAxisIndex: 0 } : { xAxisIndex: 0 };
    var a = c.convertToPixel(v, w[0]), b = c.convertToPixel(v, w[1]);
    if (!isFinite(a) || !isFinite(b)) { return null; }
    return { left: Math.min(a, b), right: Math.max(a, b) };
  }

  // ------------------------------------------------------------------ the tooltip

  // A series' key: a stroke for a line and a block for a bar, as its mark is, hatched where
  // the mark is.
  function swatch(state, i, colour) {
    if (!colour) { return '<span style="display:inline-block;flex:none;width:8px;' +
      'margin-right:8px"></span>'; }
    var bar = state.shape.kind === 'bar';
    var fill = state.tokens.hatch[i]
      ? 'repeating-linear-gradient(-45deg,' + colour + ' 0 2px,transparent 2px 4px)' : colour;
    return '<span style="display:inline-block;flex:none;width:' + (bar ? 8 : 10) + 'px;height:' +
      (bar ? 8 : 3) + 'px;margin-right:8px;border-radius:2px;background:' + fill + '"></span>';
  }

  // One tooltip for the category: every visible series, keyed as its mark is, the value set
  // strong and the name quiet; the series under the pointer is in the strong ink too, and a
  // missing value says so in the quietest ink. A stack adds its whole. The same words go to
  // the live region. As a strip, the same in one line: the tooltip laid over the card's header
  // band where no place beside the rule leaves every line uncovered (tipPlace()).
  function tip(state, items, asText, strip) {
    var rows = [], total = 0, any = false;
    var unit = state.shape.unit;
    var t = state.tokens;
    var index = items.length ? items[0].dataIndex : -1;
    var title = heading(state, index);
    // every item is a series of the table: the formatter has taken the area series out
    items.forEach(function (item) {
      var s = state.data.series[item.seriesIndex];
      var value = s.data[item.dataIndex];
      if (value != null) { total += value; any = true; }
      rows.push({ i: item.seriesIndex, name: s.name, value: amount(value, unit),
                  missing: value == null, colour: t.colours[item.seriesIndex],
                  hot: state.hover === item.seriesIndex });
    });
    if (state.shape.stacked && rows.length > 1) {
      rows.push({ i: -1, name: 'total', value: amount(any ? total : null, unit), colour: '',
                  missing: !any, hot: false, total: true });
    }
    if (asText) {
      return title + ': ' + rows.map(function (r) { return r.name + ' ' + r.value; }).join(', ');
    }
    // A stack of many parts is told in two columns, so the tooltip is short enough to stand
    // beside, above or below the row it describes rather than over it.
    var two = state.shape.kind === 'bar' && state.shape.stacked && rows.length > 5;
    var foot = two && rows.length && rows[rows.length - 1].total ? rows.pop() : null;
    // dates are drawn one step per record, not to a time scale: the tooltip says which record
    var place = state.dates && index >= 0
      ? '<span style="color:' + t.faint + ';font-weight:400">' + (index + 1) + ' of ' +
        state.data.categories.length + '</span>' : '';
    if (strip) {
      return '<div class="mj-chart-tip mj-chart-tip--strip" style="display:flex;' +
        'align-items:center;gap:16px;margin:-4px -2px;white-space:nowrap;' +
        'font-variant-numeric:tabular-nums"><span style="display:inline-flex;gap:8px;' +
        'font-size:11px;font-weight:500;color:' + t.muted + '"><span>' + escapeHtml(title) +
        '</span>' + place + '</span>' + rows.map(function (r) {
          return '<span style="display:inline-flex;align-items:center;gap:6px;color:' +
            (r.hot ? t.fg : t.muted) + '"><span style="display:inline-flex;align-items:center">' +
            swatch(state, r.i, r.colour) + escapeHtml(r.name) + '</span><span style="' +
            (r.missing ? 'font-weight:400;font-style:italic;color:' + t.faint
              : 'font-weight:600;color:' + t.fg) + '">' + escapeHtml(r.value) + '</span></span>';
        }).join('') + '</div>';
    }
    var row = function (r) {
      return '<div style="display:flex;align-items:center;justify-content:space-between;' +
        'gap:' + (two ? 10 : 20) + 'px;line-height:' + (two ? 18 : 20) + 'px;min-width:0' +
        (r.total ? ';margin-top:4px;padding-top:4px;border-top:1px solid ' + t.line : '') +
        (r.total && two ? ';grid-column:1 / -1' : '') + '">' +
        '<span style="display:inline-flex;align-items:center;min-width:0;white-space:nowrap;' +
        'color:' + (r.hot ? t.fg : t.muted) + '">' + swatch(state, r.i, r.colour) +
        escapeHtml(r.name) + '</span><span style="white-space:nowrap;' + (r.missing
          ? 'font-weight:400;font-style:italic;color:' + t.faint
          : 'font-weight:600;color:' + t.fg) + '">' + escapeHtml(r.value) + '</span></div>';
    };
    return '<div class="mj-chart-tip" style="font-variant-numeric:tabular-nums">' +
      '<div style="display:flex;justify-content:space-between;gap:16px;margin-bottom:6px;' +
      'font-size:11px;font-weight:500;letter-spacing:0.01em;color:' + t.muted + '">' +
      '<span>' + escapeHtml(title) + '</span>' + place + '</div>' +
      (two ? '<div style="display:grid;grid-template-columns:1fr 1fr;column-gap:16px">' : '') +
      rows.map(row).join('') + (foot ? row(foot) : '') + (two ? '</div>' : '') + '</div>';
  }

  // How far a stack or a row of bars reaches at a category: its whole, or its longest bar.
  function reach(state, row) {
    var value = state.shape.stacked ? stackTotal(state, row) : null;
    if (value == null) {
      value = 0;
      state.data.series.forEach(function (s, k) {
        if (visible(state, k) && s.data[row] != null) { value = Math.max(value, s.data[row]); }
      });
    }
    return Math.max(0, value);
  }
  // A bar row's box in the frame's pixels: from just before its baseline to just past its end
  // and the whole's label, across most of its band. The band behind a row pointed at is drawn
  // in it, and a tooltip is kept off it.
  function rowBox(state, row) {
    var c = state.chart, horizontal = state.shape.horizontal;
    var along = horizontal ? { yAxisIndex: 0 } : { xAxisIndex: 0 };
    var across = horizontal ? { xAxisIndex: 0 } : { yAxisIndex: 0 };
    var mid = c.convertToPixel(along, row);
    var next = c.convertToPixel(along, row + (row > 0 ? -1 : 1));
    var band = isFinite(next) && next !== mid ? Math.abs(next - mid) : 30;
    var base = c.convertToPixel(across, 0), end = c.convertToPixel(across, reach(state, row));
    if (!isFinite(mid) || !isFinite(base) || !isFinite(end)) { return null; }
    var total = state.shape.stacked ? stackTotal(state, row) : null;
    var label = total == null ? 0
      : textWidth(amount(total, ''), '500 11px ' + state.tokens.font) + 10;
    var thick = Math.max(8, band - 4);
    if (horizontal && labelsAbove(state)) {
      // a phone's row is its name over its bar: the band holds both
      var name = textWidth(state.data.categories[row] || '', '12px ' + state.tokens.font);
      var rise = ROW_BAR / 2 + ROW_PART + ROW_NAME + 3;
      return { x: Math.min(base, end) - 6, y: mid - rise,
               width: Math.max(Math.abs(end - base) + label + 14, name + 14),
               height: rise + ROW_BAR / 2 + 4 };
    }
    if (horizontal) {
      return { x: Math.min(base, end) - 6, y: mid - thick / 2,
               width: Math.abs(end - base) + label + 14, height: thick };
    }
    var top = Math.min(base, end) - (label ? 18 : 6);
    return { x: mid - thick / 2, y: top, width: thick, height: Math.max(base, end) - top };
  }

  // How much of a rectangle two boxes share, in square pixels.
  function overlap(a, b) {
    var w = Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x);
    var h = Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y);
    return w > 0 && h > 0 ? w * h : 0;
  }
  function clamp(v, lo, hi) { return Math.max(lo, Math.min(v, Math.max(lo, hi))); }
  // The cheapest of some places for a box: the one that covers least of what is drawn, by the
  // cost each place is given, the first of equals; every place kept inside the frame first.
  // Its top left corner, and what it covers.
  function cheapest(places, box, view, cost) {
    var chosen = null, least = Infinity, covered = 0;
    places.forEach(function (p, k) {
      var r = { x: clamp(p[0], 0, view[0] - box[0]), y: clamp(p[1], 0, view[1] - box[1]),
                width: box[0], height: box[1] };
      var c = cost(r);
      if (c + k * 1e-3 < least) { least = c + k * 1e-3; chosen = r; covered = c; }
    });
    return chosen ? { x: chosen.x, y: chosen.y, cost: covered } : { x: 0, y: 0, cost: 0 };
  }
  function best(places, box, view, cost) {
    var spot = cheapest(places, box, view, cost);
    return [spot.x, spot.y];
  }
  // The card's header band in the frame's pixels (above the frame, so its top is negative), or
  // null where there is none to stand in.
  function headBand(state) {
    var head = state.head;
    if (!head || head.hidden || typeof head.getBoundingClientRect !== 'function') { return null; }
    var h = head.getBoundingClientRect(), f = state.box.getBoundingClientRect();
    return h.height > 0 ? { top: h.top - f.top, height: h.height } : null;
  }

  // Where a tooltip stands, which is where it hides none of the data, or the least of it (on a
  // phone it is docked in the header and never floats over the plot). On a line it stands
  // beside the rule, on either side, at whichever of five heights from the top of the plot to
  // its bottom covers none of the lines' points; never left of the plot, so it covers neither
  // the value axis nor the rule's pill. Where every such place covers a line, the tooltip
  // leaves the plot: laid out as one line (tip()'s strip), it stands over the card's header
  // band, centred on the rule, where the legend's keys say the same values, and covers no line
  // at all. On bars it stands beside the end of the row pointed at, or below or above it, or
  // at an edge of the plot, wherever it covers least of the other rows, and never the row
  // itself. Either way it is kept inside the frame (or its header): this file confines it, so
  // that ECharts' own confinement does not pull it back over the plot.
  function tipPlace(state) {
    return function (at, params, dom, rect, size) {
      var box = size.contentSize, view = size.viewSize, gap = 14;
      var list = Array.isArray(params) ? params : [params];
      var row = list.length && list[0] ? list[0].dataIndex : -1;
      if (!state.chart || row < 0 || row == null) {
        return best([[at[0] + gap, 8]], box, view, function () { return 0; });
      }
      var grid = gridBox(state) || {};
      var plot = plotBox(state) || { left: 0, right: view[0] };
      var top = isFinite(grid.top) ? grid.top : 0;
      var bottom = isFinite(grid.bottom) ? grid.bottom : view[1];
      var left = isFinite(grid.left) ? grid.left : plot.left;
      if (state.shape.kind === 'line') {
        // the tooltip as the formatter wrote it, measured as it is drawn: a strip set last time
        // is put back first
        var floating = Boolean(dom && dom.querySelector && state.float);
        if (floating && dom.querySelector('.mj-chart-tip--strip')) { dom.innerHTML = state.float; }
        if (floating) { dom.style.width = ''; }
        if (floating && dom.offsetWidth) { box = [dom.offsetWidth, dom.offsetHeight]; }
        var points = linePoints(state, row);
        var xs = [at[0] + gap, at[0] - gap - box[0]];
        var room = bottom - top - box[1], places = [];
        var ys = [0, 0.25, 0.5, 0.75, 1].map(function (f) { return top + 4 + (room - 8) * f; });
        // the side with more room first, so that of two equal places it is the one taken
        if (view[0] - at[0] < at[0] - left) { xs.reverse(); }
        ys.forEach(function (y) {
          xs.forEach(function (x) { places.push([Math.max(left, x), y]); });
        });
        var spot = cheapest(places, box, view, function (r) {
          var cost = 0;
          points.forEach(function (p) {
            if (p.x > r.x - 8 && p.x < r.x + r.width + 8 && p.y > r.y - 8 &&
                p.y < r.y + r.height + 8) { cost += p.weight; }
          });
          return cost;
        });
        var band = spot.cost > 0 && floating && state.strip ? headBand(state) : null;
        if (!band) { return [spot.x, spot.y]; }
        // the strip is as wide as the frame, so it stands in for the whole header band
        dom.innerHTML = state.strip;
        var w = dom.scrollWidth;
        dom.style.boxSizing = 'border-box';
        dom.style.width = view[0] + 'px';
        var h = dom.offsetHeight;
        // a strip wider than the card is no strip: the least covering place stands
        if (!w || dom.scrollWidth > view[0]) {
          dom.innerHTML = state.float;
          dom.style.width = '';
          return [spot.x, spot.y];
        }
        return [0, band.top + (band.height - h) / 2];
      }
      var own = rowBox(state, row);
      if (!own) { return best([[at[0] + gap, at[1]]], box, view, function () { return 0; }); }
      var others = [];
      for (var k = 0; k < state.data.categories.length; k++) {
        var b = k === row ? null : rowBox(state, k);
        if (b) { others.push(b); }
      }
      var mid = own.y + own.height / 2, centre = own.x + own.width / 2;
      var horizontal = state.shape.horizontal, edge = view[0] - box[0] - 4;
      var below = own.y + own.height + 6, above = own.y - 6 - box[1];
      var spots = horizontal
        ? [[own.x + own.width + gap, mid - box[1] / 2],
           [at[0] - box[0] / 2, below], [at[0] - box[0] / 2, above],
           [edge, below], [edge, above], [edge, top], [edge, bottom - box[1]]]
        : [[own.x + own.width + 6, at[1] - box[1] / 2], [own.x - 6 - box[0], at[1] - box[1] / 2],
           [centre - box[0] / 2, own.y - 6 - box[1]]];
      return best(spots, box, view, function (r) {
        var cost = overlap(r, own) * 1000;
        others.forEach(function (o) { cost += overlap(r, o); });
        return cost;
      });
    };
  }
  // The points of every visible line the window shows, in the frame's pixels, and between
  // each two a point of the segment that joins them, each weighed by what covering it costs:
  // a dot at the category pointed at is never covered, the line focused is covered last.
  function linePoints(state, at) {
    var w = state.window || whole(state), out = [];
    state.data.series.forEach(function (s, i) {
      if (!visible(state, i)) { return; }
      var weight = state.focused === i ? 4 : 1, last = null;
      for (var k = w[0]; k <= w[1]; k++) {
        if (s.data[k] == null) { last = null; continue; }
        var p = state.chart.convertToPixel({ gridIndex: 0 }, [k, s.data[k]]);
        if (!p || !isFinite(p[0]) || !isFinite(p[1])) { continue; }
        out.push({ x: p[0], y: p[1], weight: k === at ? 1000 : weight });
        if (last) {
          out.push({ x: (p[0] + last[0]) / 2, y: (p[1] + last[1]) / 2, weight: weight });
        }
        last = p;
      }
    });
    return out;
  }

  // ------------------------------------------------------------------ the option

  // Whether a bar's names stand above their bars: on a phone, where a column of names would
  // take half the card and leave the bars slivers.
  function labelsAbove(state) {
    return state.shape.horizontal && narrow(state);
  }
  // How tall a horizontal bar chart's frame is: a readable row per category, however many there
  // are; on a phone a taller one, which holds the name over the bar.
  function rows(state) {
    var n = state.data.categories.length;
    state.box.style.height = (labelsAbove(state) ? n * ROW + 40 : Math.max(300, n * 30 + 48)) +
      'px';
  }

  function option(state) {
    var data = state.data, kind = state.shape.kind, unit = state.shape.unit;
    var horizontal = state.shape.horizontal, stacked = state.shape.stacked;
    var t = state.tokens = palette(state);
    var many = data.series.length > 1, n = data.series.length;
    var small = narrow(state);
    var text = { color: t.muted, fontFamily: t.font, fontSize: 11 };
    state.zoomable = crowded(state);
    if (!state.zoomable) { state.window = null; }
    var w = state.window || whole(state);
    var dated = kind === 'line' && Boolean(state.dates);
    // On a phone a row's name stands on its own line above its bar (above()), from the plot's
    // left edge and inside the plot, so the names take no column and the bars the whole width;
    // a name is cut, with an ellipsis, only where it is wider than the card. Elsewhere the names
    // are a column of their own beside the bars.
    var above = labelsAbove(state);
    var room = horizontal ? (above ? Math.max(48, state.box.clientWidth - 48) : 200) : undefined;
    var category = {
      type: 'category', data: data.categories, inverse: horizontal, boundaryGap: kind === 'bar',
      // a name inside the plot stands over the band behind its row (band()), not under it
      z: above ? 3 : 0,
      axisLine: { show: true, lineStyle: { color: t.line } }, axisTick: { show: false },
      axisLabel: Object.assign({
        hideOverlap: !above, width: room, overflow: 'truncate', margin: above ? 0 : 10,
        lineHeight: above ? ROW_NAME : 14
      }, text, horizontal ? { color: t.body, fontSize: 12 } : {}, above ? {
        // the name's foot stands ROW_PART above its bar, which is centred in the row
        inside: true, align: 'left', verticalAlign: 'bottom',
        padding: [0, 0, ROW_BAR / 2 + ROW_PART, 0]
      } : {})
    };
    if (dated) {
      category.axisLabel.interval = function (index, name) {
        return labelIndex(state, name, index) % labelStep(state) === 0;
      };
      category.axisLabel.formatter = function (name, index) {
        return dateLabel(state, labelIndex(state, name, index));
      };
      category.axisLabel.rich = { year: { color: t.faint, fontSize: 10, lineHeight: 14 } };
    }
    // the unit is named once: at the top of a line's value axis, under a bar's
    var named = unit && unit !== '%';
    var value = {
      type: 'value', scale: kind === 'line',
      name: named ? unit : '', nameLocation: horizontal ? 'middle' : 'end',
      nameGap: horizontal ? 30 : 14,
      nameTextStyle: Object.assign({}, text, horizontal ? {} : { align: 'right' }),
      axisLine: { show: false }, axisTick: { show: false },
      axisLabel: Object.assign({
        margin: 12, formatter: function (v) { return unit === '%' ? v + '%' : String(v); }
      }, text),
      splitLine: { lineStyle: { color: t.line, width: 1 } }
    };
    // A line is read at an x, so the pointer is a vertical rule whose pill says the day where
    // the axis labels are (the label it stands on is hidden, veil()). There is no horizontal
    // rule: its pill would say the pointer's height, a number the table does not hold, beside
    // the values the tooltip gives. A bar is read as a mark, so the pointer is the band behind
    // its category.
    var pill = {
      show: true, backgroundColor: t.fg, color: t.plot, fontFamily: t.font, fontSize: 11,
      fontWeight: 500, padding: [4, 7], borderRadius: 6,
      shadowBlur: 6, shadowColor: alpha(t.shadow, 0.22),
      formatter: function (p) {
        var picked = p.seriesData && p.seriesData[0];
        var i = picked ? picked.dataIndex : data.categories.indexOf(String(p.value));
        return i >= 0 ? day(state, i) : String(p.value);
      }
    };
    var pointer = kind === 'line'
      ? {
        type: 'line', snap: true,
        lineStyle: { color: t.faint, width: 1, type: [3, 3] },
        label: pill
      }
      // ECharts' band spans the whole grid, past the row's end; it keeps the axis trigger and
      // is not drawn: the band a reader sees is band(), rounded, inset, ending with the row
      : { type: 'shadow', z: 0, shadowStyle: { color: t.band, opacity: 0 } };
    var speed = still() ? 0 : duration();
    var entry = entering(state);
    var o = {
      animation: !still(),
      animationDuration: entry, animationEasing: 'cubicOut',
      animationDurationUpdate: speed * 2, animationEasingUpdate: 'cubicOut',
      backgroundColor: 'transparent',
      textStyle: text,
      color: t.colours,
      aria: { enabled: true, label: { description: state.label } },
      grid: {
        // a line's value pill is wider than its ticks and stands right-aligned to them; a
        // phone's bar names stand inside the plot, the first just under the header
        left: kind === 'line' ? 14 : above ? 6 : 4,
        right: kind === 'line' ? endRoom(state) : (stacked ? 40 : 16),
        top: kind === 'line' && named ? 30 : above ? 6 : 12,
        bottom: state.zoomable ? 38 : 4, containLabel: true
      },
      // the selection lives in ECharts' legend, which filters the series and rescales the
      // axis; what a reader sees and presses is the row of buttons in the card's header
      // (keys()), because an entry drawn on the canvas can be neither focused nor wrapped.
      // A line's area series bears its name, so it is shown and hidden with its line.
      legend: many ? {
        show: false, selectedMode: true, selected: Object.assign({}, state.selected),
        data: data.series.map(function (s) { return s.name; })
      } : undefined,
      tooltip: {
        // placed where it hides nothing, or least (tipPlace()), which keeps it inside the frame
        // or over the header band; on a phone it is docked in the card's header (dock()) and
        // never floats over the plot
        trigger: 'axis', axisPointer: pointer, confine: false, showContent: !small,
        transitionDuration: speed / 1000,
        backgroundColor: t.raised, borderColor: t.line, borderWidth: 1,
        padding: small ? [8, 10] : [10, 12],
        textStyle: { color: t.fg, fontFamily: t.font, fontSize: small ? 11 : 12 },
        extraCssText: 'border-radius:10px;' + (small ? '' : 'min-width:11rem;') +
          'box-shadow:0 1px 2px ' + alpha(t.shadow, 0.08) + ',0 12px 32px -12px ' +
          alpha(t.shadow, 0.35) + ';',
        position: tipPlace(state),
        formatter: function (items) {
          // the area series are not data: no tooltip names them (tip() reads only the table's)
          var list = (Array.isArray(items) ? items : [items]).filter(function (item) {
            return item && item.seriesIndex < n && !/^area-/.test(String(item.seriesId || ''));
          });
          if (list.length) { track(state, list[0].dataIndex); }
          state.tip = tip(state, list, true);
          state.strip = kind === 'line' && !small ? tip(state, list, false, true) : '';
          state.float = tip(state, list);
          return state.float;
        }
      },
      xAxis: horizontal ? value : category,
      yAxis: horizontal ? category : value,
      series: data.series.map(function (s, i) {
        var colour = t.colours[i];
        var base = { id: 'series-' + i, name: s.name, type: kind };
        if (kind === 'line') {
          // straight segments between measured points (nothing between two was measured), a
          // gap where a value is missing, no symbols: the dots at the category pointed at are
          // drawn by cursor(), the same in both themes. The area is its own series (below).
          return Object.assign(base, {
            data: s.data, smooth: false, connectNulls: false, triggerLineEvent: false,
            showSymbol: false, symbol: 'none', z: 3, lineStyle: stroke(state, i),
            itemStyle: { color: colour, borderColor: t.plot, borderWidth: 2 },
            // the series pointed at is focused by fade(), never by ECharts' emphasis, which the
            // axis tooltip lends every series at once (each would fade all the others)
            emphasis: { focus: 'none', scale: false }
          });
        }
        var hatch = t.hatch[i] ? {
          symbol: 'rect', symbolSize: 1, color: alpha(t.plot, 0.7), rotation: -Math.PI / 4,
          dashArrayX: [1, 0], dashArrayY: [2, 3]
        } : undefined;
        return Object.assign(base, {
          data: bars(state, i), stack: stacked ? 'whole' : undefined,
          barMaxWidth: above ? ROW_BAR : 28, barGap: '20%', barMinHeight: MIN_BAR,
          // a 1px border in the card's colour on each side is the 2px gap between segments
          itemStyle: { color: colour, borderColor: t.plot, borderWidth: stacked ? 1 : 0,
                       decal: hatch },
          // a bar is read in its row: pointing at one fades nothing (the band and the tooltip
          // say which row); a legend key focuses its series (fade())
          emphasis: { focus: 'none' },
          animationDuration: entry,
          animationDelay: function (index) { return entry ? Math.min(index, 12) * 24 : 0; }
        });
      })
    };
    if (kind === 'line') {
      var marks = ends(state, t);
      o.series.forEach(function (s, i) {
        s.markPoint = marks[i];
        s.markLine = bridges(state, t, i);
      });
      // Each line's area, as a series of its own under the lines, filled only under a lone
      // line or the line focused among others, so overlapping fills never muddy the plot, and
      // only while the value axis holds zero (grounded()): an area is an amount from its
      // baseline, and down to an axis floor of 60% it would draw a change as a magnitude.
      // It stops where its line stops: no tint lies under an interval nothing was recorded
      // in, where the dashed bridge alone says the line resumes.
      // The fill is the series colour itself at a share of its opacity, fading to the same
      // colour at none (a fade to `transparent` would pass through black and grey it): never
      // a mix with the card, which in the dark theme smears an orange into brown. The dark
      // theme's strengths are its own (palette()).
      var strength = many ? t.focusArea : t.area;
      data.series.forEach(function (s, i) {
        var colour = t.colours[i];
        o.series.push({
          id: 'area-' + i, name: s.name, type: 'line', data: s.data, z: 1,
          connectNulls: false, silent: true, smooth: false, showSymbol: false, symbol: 'none',
          triggerLineEvent: false, lineStyle: { width: 0, opacity: 0 },
          areaStyle: t.area > 0 ? {
            opacity: filled(state, i), origin: 'auto',
            color: new state.echarts.graphic.LinearGradient(0, 0, 0, 1, [
              { offset: 0, color: alpha(colour, strength) }, { offset: 1, color: alpha(colour, 0) }
            ])
          } : undefined,
          emphasis: { disabled: true }, tooltip: { show: false },
          markPoint: { silent: true, animation: false, data: [] }
        });
      });
    }
    if (state.zoomable) {
      // The slider only. ECharts' `inside` zoom cancels every wheel event over the plot,
      // whatever its ctrl and shift settings say, so the page would stop scrolling under the
      // pointer; the wheel, the pinch and the drag are this file's (wheel(), drag()), and a
      // plain wheel is left to the page. Laid along the plot (align()), so a handle stands
      // over the date it selects: a 4px track that keeps 3:1 on the card outside the window
      // (t.track), the window in the accent, and for each handle a 12px accent dot in a rim of
      // the card (RIM) that parts it from the track and makes the 24px target, the rim taking
      // the accent's tint while pointed at or dragged. No mini series: a line with gaps drawn
      // that small reads as broken, and the plot shows it.
      var edge = state.slider || { left: 48, right: o.grid.right };
      o.dataZoom = [
        {
          type: 'slider', xAxisIndex: 0, filterMode: 'filter', startValue: w[0], endValue: w[1],
          minValueSpan: 2, height: 4, bottom: 14, left: edge.left, right: edge.right,
          brushSelect: false, showDataShadow: false, borderRadius: 2,
          borderColor: alpha(t.track, 0), backgroundColor: t.track, fillerColor: t.accent,
          handleIcon: 'circle', handleSize: HANDLE,
          handleStyle: { color: t.accent, borderColor: t.plot, borderWidth: RIM, shadowBlur: 0 },
          moveHandleSize: 0,
          emphasis: { handleStyle: { color: t.accent, borderColor: mix(t.plot, t.accent, 0.22),
                                     borderWidth: RIM, shadowBlur: 0 } },
          showDetail: true,
          textStyle: { color: t.muted, fontFamily: t.font, fontSize: 11 },
          labelFormatter: function (index) { return brief(state, index, false); }
        }
      ];
    }
    if (kind === 'bar') {
      // the band behind the row pointed at (band()), under the bars and over the gridlines
      o.graphic = { elements: [{ id: 'band', type: 'rect', silent: true, z: 1, invisible: true,
                                 shape: { x: 0, y: 0, width: 0, height: 0, r: 6 },
                                 style: { fill: t.band } }] };
    }
    state.animation = o.animation;
    return o;
  }

  // The slider laid along the plot: once a drawing has settled (a zoom lays the axis out
  // again, and its labels move the plot's edges while it does), the track spans exactly the
  // categories the plot draws; the edges, when they moved, or null.
  function align(state) {
    if (!state.zoomable || !state.chart) { return null; }
    var plot = gridBox(state) || plotBox(state), width = state.box.clientWidth || 0;
    if (!plot || !width) { return null; }
    // ECharts sets the slider's box where its handles' edges are, so the track starts half a
    // handle in from `left`: the box is let out by that half at each end
    var half = HANDLE / 2;
    var edge = { left: Math.round(plot.left - half), right: Math.round(width - plot.right + half) };
    var was = state.slider;
    if (was && Math.abs(was.left - edge.left) < 1 && Math.abs(was.right - edge.right) < 1) {
      return null;
    }
    state.slider = edge;
    return edge;
  }

  // ------------------------------------------------------------------ the header

  // The Reset row the build wrote, as a template, into every page that carries a chart
  // (site/templates/partials/charts.html): the markup the interaction check counts is the
  // markup a zoomable chart shows. A page without the template gets the same row, made here.
  // It is the right end of the card's header.
  function tools(state) {
    if (state.tools) { return state.tools; }
    var proto = document.querySelector('template[data-chart-tools]');
    var row = proto && proto.content && proto.content.firstElementChild
      ? proto.content.firstElementChild.cloneNode(true) : null;
    if (!row) {
      row = document.createElement('div');
      row.className = 'mj-chart-tools';
      var span = document.createElement('span');
      span.className = 'mj-chart-range';
      row.appendChild(span);
      var made = document.createElement('button');
      made.type = 'button';
      made.className = 'mj-button mj-button--quiet';
      made.setAttribute('data-chart-action', 'reset');
      made.setAttribute('aria-label', 'Reset zoom: show the whole range');
      made.textContent = 'Reset zoom';
      row.appendChild(made);
    }
    row.hidden = true;
    var button = row.querySelector('[data-chart-action="reset"]');
    // the instance answers for its chart; the template is what the interaction check claims
    button.setAttribute('data-chart-for', state.figure.id || '');
    if (state.box.id) { button.setAttribute('aria-controls', state.box.id); }
    button.addEventListener('click', function () {
      // Reset is gone once the whole range is back: the focus it had goes to the chart
      var had = document.activeElement === button;
      reset(state);
      if (had && typeof state.box.focus === 'function') { state.box.focus(); }
    });
    state.head.appendChild(row);
    state.tools = row;
    state.reset = button;
    state.range = row.querySelector('.mj-chart-range');
    return row;
  }

  // A key's parts: its mark, its name and its value. The value is drawn and not read out
  // (aria-hidden): a toggle is named by its series, and the numbers are the table's.
  function part(className, hidden) {
    var span = document.createElement('span');
    span.className = className;
    if (hidden) { span.setAttribute('aria-hidden', 'true'); }
    return span;
  }

  // The legend: one toggle button per series, in the header's left end, from the template the
  // build wrote (partials/charts.html) or made here. A button says whether its series is shown
  // with aria-pressed, which the stylesheet draws as a struck-through name and a hollow key,
  // so the state is never told by a colour alone; its key is the series colour, a stroke for
  // a line and a block for a bar, as the mark is, and beside its name stands its value.
  function keyButton(model) {
    if (model) { return model.cloneNode(true); }
    var button = document.createElement('button');
    button.type = 'button';
    button.className = 'mj-chart-key';
    button.setAttribute('data-chart-action', 'series');
    button.appendChild(part('mj-chart-key-mark', true));
    button.appendChild(part('mj-chart-key-name', false));
    button.appendChild(part('mj-chart-key-value', true));
    return button;
  }
  function keys(state) {
    if (state.keys || state.data.series.length < 2) { return state.keys; }
    var proto = document.querySelector('template[data-chart-legend]');
    var row = proto && proto.content && proto.content.firstElementChild
      ? proto.content.firstElementChild.cloneNode(true) : null;
    var model = row ? row.querySelector('[data-chart-action="series"]') : null;
    if (model) { model.remove(); }
    if (!row) {
      row = document.createElement('div');
      row.className = 'mj-chart-keys';
      row.setAttribute('role', 'group');
    }
    row.setAttribute('aria-label', 'Series: press one to hide or show it');
    if (state.shape.kind === 'line') { row.classList.add('mj-chart-keys--line'); }
    state.data.series.forEach(function (s, i) {
      var button = keyButton(model);
      // the instance answers for its chart; the template is what the interaction check claims
      button.setAttribute('data-chart-for', state.figure.id || '');
      button.setAttribute('data-chart-series', s.name);
      if (state.box.id) { button.setAttribute('aria-controls', state.box.id); }
      var name = button.querySelector('.mj-chart-key-name');
      if (name) { name.textContent = s.name; } else { button.textContent = s.name; }
      if (!button.querySelector('.mj-chart-key-value')) {
        button.appendChild(part('mj-chart-key-value', true));
      }
      button.addEventListener('click', function () {
        if (!state.chart) { return; }
        state.chart.dispatchAction({ type: 'legendToggleSelect', name: s.name });
      });
      // pointing at a key focuses its series on the chart, as pointing at the series does
      var on = function () { if (visible(state, i)) { fade(state, i); } };
      var off = function () { fade(state, null); };
      button.addEventListener('mouseenter', on);
      button.addEventListener('focus', on);
      button.addEventListener('mouseleave', off);
      button.addEventListener('blur', off);
      row.appendChild(button);
    });
    state.head.insertBefore(row, state.head.firstChild);
    state.keys = row;
    return row;
  }
  // One series has no legend to press; its key and value stand in the header as a readout.
  function readout(state) {
    if (state.readout || state.data.series.length !== 1) { return state.readout; }
    var row = document.createElement('div');
    row.className = 'mj-chart-keys' + (state.shape.kind === 'line' ? ' mj-chart-keys--line' : '');
    var item = part('mj-chart-key mj-chart-key--static', false);
    item.appendChild(part('mj-chart-key-mark', true));
    var name = part('mj-chart-key-name', false);
    name.textContent = state.data.series[0].name;
    item.appendChild(name);
    item.appendChild(part('mj-chart-key-value', true));
    row.appendChild(item);
    state.head.insertBefore(row, state.head.firstChild);
    state.readout = row;
    return row;
  }
  // The key of the bridges, after the legend's last key while a bridge is drawn: what the
  // dashes are. It flows with the keys, so on a phone it wraps beside them, not on a row alone.
  function gapKey(state) {
    if (state.gap) { return state.gap; }
    var key = part('mj-chart-gap', false);
    key.setAttribute('title', 'A dashed line crosses categories where no value was recorded; ' +
      'it is not data');
    key.appendChild(part('mj-chart-gap-mark', true));
    var words = part('mj-chart-gap-name', false);
    words.textContent = 'not recorded';
    key.appendChild(words);
    key.hidden = true;
    var holder = state.keys || state.readout;
    if (holder) {
      holder.appendChild(key);
    } else {
      state.head.insertBefore(key, state.tools || null);
    }
    state.gap = key;
    return key;
  }
  // The header is shown while it holds anything.
  function showHead(state) {
    var shown = Boolean(state.keys || state.readout) ||
      Boolean(state.tools && !state.tools.hidden) || Boolean(state.gap && !state.gap.hidden);
    state.head.hidden = !shown;
  }

  // The value a key shows: on a line the value at the category pointed at, else the latest
  // the window shows; on bars the whole of its column (a unit that adds up), pointed at or not,
  // since a row's own counts are the tooltip's and a header that changed to them would read as
  // totals. A key's value is a figure without its unit, as the end labels are, which the axis
  // names; a share keeps its `%`. Nothing is shown for a hidden series, and a dash where the
  // category pointed at recorded nothing.
  function keyText(state, i) {
    var s = state.data.series[i], unit = state.shape.unit === '%' ? '%' : '';
    if (!visible(state, i)) { return ''; }
    // on a phone the tooltip is docked in the header: the keys say the row pointed at, and
    // the line beside Reset says which row it is and its whole (dock())
    var docking = state.shape.kind === 'line' || narrow(state);
    if (state.pointing != null && docking) {
      var v = s.data[state.pointing];
      return v == null ? '–' : amount(v, unit);
    }
    if (state.shape.kind === 'line') {
      var w = state.window || whole(state);
      for (var k = w[1]; k >= w[0]; k--) {
        if (s.data[k] != null) { return amount(s.data[k], unit); }
      }
      return '';
    }
    if (unit) { return ''; }
    var sum = 0, any = false;
    s.data.forEach(function (x) { if (x != null) { sum += x; any = true; } });
    return any ? amount(sum, '') : '';
  }
  function paintValues(state) {
    var holder = state.keys || state.readout;
    if (!holder) { return; }
    var spans = holder.querySelectorAll('.mj-chart-key-value');
    Array.prototype.forEach.call(spans, function (span, i) {
      if (i < state.data.series.length) { span.textContent = keyText(state, i); }
    });
  }
  // Each key's colour and state, again after a redraw (another theme) and after a toggle.
  function paintKeys(state) {
    var holder = keys(state) || readout(state);
    if (!holder) { return; }
    holder.hidden = false;
    var items = state.keys
      ? state.keys.querySelectorAll('[data-chart-action="series"]')
      : holder.querySelectorAll('.mj-chart-key');
    Array.prototype.forEach.call(items, function (item, i) {
      if (state.keys) { item.setAttribute('aria-pressed', visible(state, i) ? 'true' : 'false'); }
      var colour = state.tokens ? state.tokens.colours[i] : '';
      if (colour && typeof item.style.setProperty === 'function') {
        item.style.setProperty('--chart-key', colour);
      }
      if (state.tokens && state.tokens.hatch[i]) {
        item.setAttribute('data-chart-hatch', '');
      }
    });
    paintValues(state);
    showHead(state);
  }

  // The right end of the header: the window a zoomable chart shows, and its Reset while it
  // is zoomed (at the whole range there is nothing to reset, and Reset takes no room). On a
  // phone every chart has it, as the tooltip's dock (dock()).
  function showTools(state) {
    var small = narrow(state);
    if (!state.zoomable && !small && !state.tools) { return; }
    var row = tools(state);
    row.hidden = !state.zoomable && !small;
    state.reset.disabled = full(state);
    state.reset.hidden = !state.zoomable || full(state);
    dock(state);
    showHead(state);
  }
  // What the header's range says. On a phone, while a category is pointed at, it is the
  // tooltip docked out of the plot: the date in full (a bar's row and its whole), with the
  // keys giving each series' value. Otherwise the window shown: its first and last category
  // and how many of the whole it holds; on bars, how many rows and their whole.
  function dock(state) {
    if (!state.range) { return; }
    var c = state.data.categories, d = state.dates, small = narrow(state), at = state.pointing;
    var bar = state.shape.kind === 'bar', unit = state.shape.unit;
    var pointing = small && at != null && at >= 0 && at < c.length;
    if (pointing) {
      state.range.setAttribute('data-chart-pointing', '');
    } else if (typeof state.range.removeAttribute === 'function') {
      state.range.removeAttribute('data-chart-pointing');
    }
    if (pointing) {
      var all = bar && state.shape.stacked ? stackTotal(state, at) : null;
      state.range.textContent = bar
        ? c[at] + (all != null ? ' · ' + amount(all, unit) : '') : docked(state, at);
      return;
    }
    if (bar) {
      var sum = null;
      if (state.shape.stacked && unit !== '%') {
        c.forEach(function (x, k) {
          var v = stackTotal(state, k);
          if (v != null) { sum = (sum || 0) + v; }
        });
      }
      state.range.textContent = c.length + ' rows' +
        (sum != null ? ' · ' + amount(sum, unit) : '');
      return;
    }
    var w = state.window || whole(state);
    // on a phone the range is said without its year, so it and Reset share one line
    var span = d && small ? brief(state, w[0], false) + ' – ' + brief(state, w[1], false)
      : d && d[w[0]].year === d[w[1]].year
      ? brief(state, w[0], false) + ' – ' + brief(state, w[1], true)
      : d ? brief(state, w[0], true) + ' – ' + brief(state, w[1], true)
        : c[w[0]] + ' – ' + c[w[1]];
    state.range.textContent = span + ' · ' + (w[1] - w[0] + 1) + ' of ' + c.length;
  }

  // ------------------------------------------------------------------ focus

  // Focus one series and fade the rest, or none. A line's focus is set here as its options (the
  // other lines and their marks fade, its area fills), not as ECharts' emphasis: the tooltip
  // lends that emphasis to every series at the category pointed at, and a state ECharts keeps
  // does not survive the options set again under it. A bar fades the other series' bars, since
  // pointing at a bar fades nothing.
  function fade(state, i) {
    if (!state.chart) { return; }
    if (state.shape.kind === 'line') {
      if (state.focused === i) { return; }
      state.focused = i;
      state.hover = i;
      state.chart.setOption({ series: restyle(state) });
      cursor(state);
      return;
    }
    if (state.faded === i) { return; }
    state.faded = i;
    // a bar steps back by colour as a line does, never below 3:1 on the card
    var t = state.tokens;
    state.chart.setOption({ series: state.data.series.map(function (s, k) {
      var dim = i != null && k !== i;
      return { id: 'series-' + k, itemStyle: { color: dim ? t.receded[k] : t.colours[k] } };
    }) });
  }
  // Everything about the lines that the focus, the window and the scale decide: each line's
  // stroke, its latest value and bridges, and its area. Set again whenever one of them moves.
  function restyle(state) {
    var t = state.tokens, marks = ends(state, t);
    var lines = state.data.series.map(function (s, k) {
      return { id: 'series-' + k, lineStyle: stroke(state, k), markPoint: marks[k],
               markLine: bridges(state, t, k) };
    });
    var fills = state.data.series.map(function (s, k) {
      return { id: 'area-' + k,
               areaStyle: t.area > 0 ? { opacity: filled(state, k) } : undefined };
    });
    return lines.concat(fills);
  }
  // The line nearest the pointer at the category under it, if it is near enough.
  function nearest(state, x, y) {
    var at = state.chart.convertFromPixel({ gridIndex: 0 }, [x, y]);
    var k = at ? Math.round(at[0]) : -1;
    var best = NEAR, found = null;
    state.data.series.forEach(function (s, n) {
      if (!visible(state, n) || s.data[k] == null) { return; }
      var p = state.chart.convertToPixel({ gridIndex: 0 }, [k, s.data[k]]);
      var d = p ? Math.abs(p[1] - y) : Infinity;
      if (d < best) { best = d; found = n; }
    });
    return found;
  }
  // The pointer has let go of the chart: the keys show their resting values again, the dots
  // at the category go, and every axis label is shown.
  function letGo(state) {
    unveil(state);
    if (state.pointing == null) { return; }
    state.pointing = null;
    pointed(state);
  }
  // The category pointed at, from the tooltip or, on a phone where the tooltip is docked,
  // from the axis pointer.
  function track(state, index) {
    state.cursor = index;
    if (state.pointing !== index) {
      state.pointing = index;
      pointed(state);
    }
  }
  // The category pointed at changed: the keys' values, the docked tooltip, the dots on the
  // lines and the band behind a bar's row follow it.
  function pointed(state) {
    paintValues(state);
    dock(state);
    cursor(state);
    band(state);
  }
  // The band behind the bar row pointed at: rounded, from just before the row's baseline to
  // just past its end and its whole, across most of its band, in a tint of the card a step
  // toward the ink; none while nothing is pointed at.
  function band(state) {
    if (!state.chart || state.shape.kind !== 'bar' || state.banding) { return; }
    state.banding = true;
    later(function () {
      state.banding = false;
      if (!state.chart || !state.drawn || !state.tokens) { return; }
      var at = state.pointing, box = at == null ? null : rowBox(state, at);
      state.chart.setOption({ graphic: { elements: [{
        id: 'band', type: 'rect', invisible: !box,
        shape: box ? { x: box.x, y: box.y, width: box.width, height: box.height, r: 6 }
          : { x: 0, y: 0, width: 0, height: 0, r: 6 },
        style: { fill: state.tokens.band }
      }] } });
    });
  }

  // ------------------------------------------------------------------ zoom and keys

  function zoomTo(state, start, end) {
    if (!state.chart || !state.zoomable) { return; }
    var last = state.data.categories.length - 1;
    var span = Math.min(last, Math.max(2, end - start));
    start = Math.max(0, Math.min(start, last - span));
    state.chart.dispatchAction({ type: 'dataZoom', startValue: start, endValue: start + span });
    sync(state);
  }
  function reset(state) {
    if (!state.chart) { return; }
    state.chart.dispatchAction({ type: 'dataZoom', start: 0, end: 100 });
    sync(state);
  }

  // The window the zoom shows now, as category indexes, and everything that depends on it.
  function sync(state) {
    var w = whole(state);
    if (state.zoomable && state.chart && typeof state.chart.getOption === 'function') {
      var zoom = (state.chart.getOption().dataZoom || [])[0];
      if (zoom && zoom.startValue != null && zoom.endValue != null) {
        w = [Number(zoom.startValue), Number(zoom.endValue)];
      }
    }
    var moved = !state.window || state.window[0] !== w[0] || state.window[1] !== w[1];
    state.window = w;
    state.box.setAttribute('aria-keyshortcuts', KEYS + (state.zoomable ? ZOOM_KEYS : ''));
    showTools(state);
    if (moved) { relabel(state); }
  }

  // The marks that depend on the window and the scale, again after either changed (a zoom, a
  // series hidden or shown): the latest values, the bridges, the areas (whether the axis now
  // holds zero) and the keys' values. Setting them lays the axis out again, so the date
  // labels are thinned to the new window too.
  function relabel(state) {
    if (!state.chart || state.relabelling) { return; }
    if (state.shape.kind !== 'line') { paintValues(state); return; }
    state.relabelling = true;
    (window.requestAnimationFrame || window.setTimeout)(function () {
      state.relabelling = false;
      if (!state.chart || !state.drawn) { return; }
      var next = restyle(state);
      var bridged = next.some(function (s) { return s.markLine && s.markLine.data.length > 0; });
      // nothing is set under a line while it draws itself in (reveal()); the header's key of
      // the bridges is shown from the start, so the header never grows a line under a reader
      if (state.revealed) {
        state.chart.setOption({ series: next });
        cursor(state);
      }
      gapKey(state).hidden = !bridged;
      paintValues(state);
      showHead(state);
    });
  }

  // The keyboard: the arrows, Home and End move along the categories and the tooltip
  // follows (a zoomed window pans to keep the category in it); + and - zoom around it, 0
  // gives the whole range back, Escape lets go. A key held with ctrl, alt or meta is the
  // browser's: ctrl/cmd with + - 0 zooms the page, alt or cmd with an arrow goes Back, and a
  // reader who zooms the page because the chart is hard to read must not be stopped here.
  // Shift is not a modifier here: it is how + and _ are typed.
  function key(state, event) {
    if (!state.chart || event.ctrlKey || event.metaKey || event.altKey) { return; }
    var n = state.data.categories.length;
    var w = state.window || whole(state);
    var at = state.cursor == null ? w[1] : state.cursor;
    var span = w[1] - w[0];
    var start;
    switch (event.key) {
      case 'ArrowRight': case 'ArrowDown': at = Math.min(n - 1, at + 1); break;
      case 'ArrowLeft': case 'ArrowUp': at = Math.max(0, at - 1); break;
      case 'Home': at = w[0]; break;
      case 'End': at = w[1]; break;
      case '+': case '=':
        if (!state.zoomable) { return; }
        span = Math.max(2, Math.round(span * 0.6));
        start = at - Math.round(span / 2);
        zoomTo(state, start, start + span);
        break;
      case '-': case '_':
        if (!state.zoomable) { return; }
        span = Math.round(span / 0.6) + 1;
        start = at - Math.round(span / 2);
        zoomTo(state, start, start + span);
        break;
      case '0':
        if (!state.zoomable) { return; }
        reset(state);
        break;
      case 'Escape':
        state.cursor = null;
        state.chart.dispatchAction({ type: 'hideTip' });
        state.live.textContent = '';
        letGo(state);
        event.preventDefault();
        return;
      default: return;
    }
    event.preventDefault();
    point(state, at);
  }

  // Point at one category: the window pans to hold it, the tooltip opens on it, the keys show
  // its values and the live region says what the tooltip says.
  function point(state, at) {
    var w = state.window || whole(state);
    if (state.zoomable && (at < w[0] || at > w[1])) {
      var span = w[1] - w[0];
      var start = at < w[0] ? at : at - span;
      zoomTo(state, start, start + span);
    }
    state.cursor = at;
    state.pointing = at;
    var items = here(state, at), anchor = -1;
    items.forEach(function (item) {
      if (anchor < 0 && state.data.series[item.seriesIndex].data[at] != null) {
        anchor = item.seriesIndex;
      }
    });
    // ECharts places an axis tooltip from the point of the series it is handed, and a series
    // with no value there has no point: the tooltip would stay where it was while the live
    // region moved on. So the tooltip is anchored on a series measured at the category, and
    // when none is, on the category's place on its axis.
    if (anchor >= 0) {
      state.chart.dispatchAction({ type: 'showTip', seriesIndex: anchor, dataIndex: at });
    } else if (items.length) {
      var p = place(state, at);
      if (p) { state.chart.dispatchAction({ type: 'showTip', x: p[0], y: p[1] }); }
    }
    state.tip = tip(state, items, true);
    state.live.textContent = state.tip;
    pointed(state);
  }

  // Every visible series at a category, as the tooltip is handed them.
  function here(state, at) {
    var items = [];
    state.data.series.forEach(function (s, i) {
      if (visible(state, i)) { items.push({ seriesIndex: i, dataIndex: at }); }
    });
    return items;
  }

  // A category's pixel: on its axis, and halfway across the frame the other way.
  function place(state, at) {
    var horizontal = state.shape.horizontal;
    var c = state.chart.convertToPixel(horizontal ? { yAxisIndex: 0 } : { xAxisIndex: 0 }, at);
    if (!isFinite(c)) { return null; }
    var across = (horizontal ? state.box.clientWidth : state.box.clientHeight) / 2;
    return horizontal ? [across, c] : [c, across];
  }

  // The width of one category of a line chart's window, in pixels, or 0 before it has any.
  function categoryWidth(state) {
    var w = state.window || whole(state);
    if (w[1] <= w[0]) { return 0; }
    var a = state.chart.convertToPixel({ xAxisIndex: 0 }, w[0]);
    var b = state.chart.convertToPixel({ xAxisIndex: 0 }, w[1]);
    var width = Math.abs(b - a) / (w[1] - w[0]);
    return isFinite(width) ? width : 0;
  }
  // Whether a pointer event is over the plot (not the slider, the axis labels or the margin).
  function overPlot(state, event) {
    if (typeof state.chart.containPixel !== 'function') { return true; }
    var r = state.box.getBoundingClientRect();
    var at = [event.clientX - r.left, event.clientY - r.top];
    return state.chart.containPixel({ gridIndex: 0 }, at);
  }

  // The wheel over a zoomable plot. A plain wheel is the page's and is never cancelled. ctrl
  // (which is also what a trackpad pinch sends) zooms around the category under the pointer;
  // shift pans. Deltas are added up, because a trackpad sends many too small to move a whole
  // category each.
  function wheel(state, event) {
    if (!state.chart || !state.zoomable || !(event.ctrlKey || event.shiftKey)) { return; }
    if (!overPlot(state, event)) { return; }
    event.preventDefault();
    var scale = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? state.box.clientHeight : 1;
    var w = state.window || whole(state);
    var span = w[1] - w[0];
    if (event.ctrlKey) {
      state.pinch += (event.deltaY || 0) * scale;
      var next = Math.round(span * Math.exp(state.pinch / PINCH));
      if (next === span) { return; }
      state.pinch = 0;
      var r = state.box.getBoundingClientRect();
      var at = state.chart.convertFromPixel({ xAxisIndex: 0 }, event.clientX - r.left);
      at = isFinite(at) ? Math.max(w[0], Math.min(w[1], at)) : (w[0] + w[1]) / 2;
      // the category under the pointer stays under it
      var start = Math.round(at - (at - w[0]) * next / Math.max(1, span));
      zoomTo(state, start, start + next);
      return;
    }
    var step = categoryWidth(state);
    if (!step) { return; }
    state.pan += (event.deltaX || event.deltaY || 0) * scale;
    var by = state.pan < 0 ? Math.ceil(state.pan / step) : Math.floor(state.pan / step);
    if (!by) { return; }
    state.pan -= by * step;
    zoomTo(state, w[0] + by, w[1] + by);
  }

  // A mouse drag across a zoomable plot pans it. Touch is left to the page, so a finger still
  // scrolls it and a pinch still zooms it; the slider is the control on a phone.
  function drag(state, event) {
    if (!state.chart || !state.zoomable || event.pointerType !== 'mouse' || event.button !== 0) {
      return;
    }
    if (!overPlot(state, event)) { return; }
    var step = categoryWidth(state);
    if (!step) { return; }
    var x = event.clientX, from = (state.window || whole(state)).slice();
    state.dragged = false;
    function move(e) {
      if (!state.dragged && Math.abs(e.clientX - x) < 4) { return; }
      state.dragged = true;
      state.chart.getZr().setCursorStyle('grabbing');
      var by = Math.round((x - e.clientX) / step);
      zoomTo(state, from[0] + by, from[1] + by);
    }
    function up() {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      window.removeEventListener('pointercancel', up);
      // the click that ends a drag is not a click on the category under it
      window.setTimeout(function () { state.dragged = false; }, 0);
    }
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    window.addEventListener('pointercancel', up);
  }

  // ------------------------------------------------------------------ drawing

  // The category under a pixel of the plot, or -1 off it.
  function under(state, x, y) {
    var plot = typeof state.chart.containPixel !== 'function' ||
      state.chart.containPixel({ gridIndex: 0 }, [x, y]);
    if (!plot) { return -1; }
    var at = state.chart.convertFromPixel({ gridIndex: 0 }, [x, y]);
    var i = at ? Math.round(state.shape.horizontal ? at[1] : at[0]) : -1;
    return i >= 0 && i < state.data.categories.length ? i : -1;
  }

  function draw(state) {
    return loadECharts().then(function (echarts) {
      if (state.chart) { state.chart.dispose(); }
      state.echarts = echarts;
      state.drawn = false;
      state.focused = null;
      state.faded = null;
      state.card.hidden = false;
      state.box.hidden = false;
      if (state.shape.horizontal) { rows(state); }
      state.chart = echarts.init(state.box, null, { renderer: 'svg' });
      state.small = narrow(state);
      // the first drawing is an entrance and the marks over a line wait for its end; any later
      // one, and every one under reduced motion, draws them at once
      if (state.entered || still()) { state.revealed = true; }
      state.lit = state.revealed ? 1 : 0;
      state.chart.setOption(option(state));
      // The slider is laid along the plot before the browser first paints it: ECharts lays the
      // grid out within setOption, so the edges it settled on are known now, and a slider set
      // to them in the same task never shifts once the entrance has drawn it.
      var edge = align(state);
      if (edge) { state.chart.setOption({ dataZoom: [{ left: edge.left, right: edge.right }] }); }
      state.entered = true;
      // should no `finished` ever come (a drawing ECharts never settles), the marks are lit
      // well after any entrance could have ended
      if (!state.revealed && window.setTimeout) {
        window.setTimeout(function () { reveal(state); }, ENTRY * 3);
      }
      state.drawn = true;
      state.figure.setAttribute('data-chart-rendered', isDark() ? 'dark' : 'light');
      // the window a reader chose survives a redraw (the option carried it); the marks are
      // placed now that the chart has pixels
      sync(state);
      relabel(state);
      paintKeys(state);
      state.chart.on('mouseover', function (params) {
        if (params.componentType === 'series' && state.shape.kind === 'bar') {
          state.hover = params.seriesIndex;
        }
      });
      state.chart.on('mouseout', function () {
        if (state.shape.kind === 'bar') { state.hover = null; }
      });
      state.chart.on('globalout', function () {
        if (state.shape.kind === 'line' && state.data.series.length > 1) { fade(state, null); }
        state.crosshair = false;
        unveil(state);
        if (document.activeElement !== state.box) { letGo(state); }
      });
      // While the crosshair is drawn, the axis labels under its pills are hidden after every
      // frame ECharts paints: a frame may draw the labels again (an option set, a zoom).
      state.chart.on('updateAxisPointer', function (event) {
        var info = (event && event.axesInfo) || [];
        // on a phone the tooltip is docked and has no formatter to say what is pointed at:
        // the category axis's pointer says it
        if (narrow(state)) {
          var dim = state.shape.horizontal ? 'y' : 'x';
          var on = info.filter(function (a) { return a.axisDim === dim; })[0];
          var at = on ? Math.round(Number(on.value)) : NaN;
          if (isFinite(at) && at >= 0 && at < state.data.categories.length) {
            track(state, at);
            state.tip = tip(state, here(state, at), true);
          }
        }
        if (state.shape.kind !== 'line') { return; }
        state.crosshair = Boolean(info.length);
        if (!state.crosshair) { unveil(state); }
      });
      state.chart.on('rendered', function () { if (state.crosshair) { veil(state); } });
      // a series hidden or shown: the others keep their colours (each is fixed on its own
      // series), the axis rescales to what is left, a stack rounds its new outermost segments
      // and labels its new whole
      state.chart.on('legendselectchanged', function (params) {
        state.selected = Object.assign({}, params.selected);
        if (state.shape.kind === 'bar' && state.shape.stacked) {
          state.chart.setOption({ series: state.data.series.map(function (s, i) {
            return { id: 'series-' + i, data: bars(state, i) };
          }) });
        }
        relabel(state);
        paintKeys(state);
      });
      state.chart.on('datazoom', function () { sync(state); });
      // once a drawing settles, the marks over a line come in (once: reveal()) and the slider
      // is laid along the plot as it was finally laid out
      state.chart.on('finished', function () {
        reveal(state);
        if (!align(state)) { return; }
        later(function () {
          var w = state.window || whole(state), edge = state.slider;
          if (!state.chart || !state.zoomable || !edge) { return; }
          state.chart.setOption({ dataZoom: [{ left: edge.left, right: edge.right,
                                               startValue: w[0], endValue: w[1] }] });
        });
      });
      var zr = state.chart.getZr();
      // a category whose row is a link is a pointer and goes where the row goes; a zoomable
      // plot can be dragged; on a line the series nearest the pointer is focused
      zr.on('mousemove', function (event) {
        if (state.dragged) { return; }
        var i = under(state, event.offsetX, event.offsetY);
        zr.setCursorStyle(i >= 0 && state.data.links[i] ? 'pointer'
          : state.zoomable && i >= 0 ? 'grab' : 'default');
        if (i < 0 && document.activeElement !== state.box) { letGo(state); }
        if (state.shape.kind === 'line' && state.data.series.length > 1) {
          fade(state, i >= 0 ? nearest(state, event.offsetX, event.offsetY) : null);
        }
      });
      zr.on('click', function (event) {
        if (state.dragged) { return; }
        var i = under(state, event.offsetX, event.offsetY);
        var href = i >= 0 ? state.data.links[i] : null;
        if (href) { window.location.assign(href); }
      });
      address();
    }).catch(function (error) {
      // the table is the chart's fallback and it never left the page
      state.card.hidden = true;
      state.figure.setAttribute('data-chart-error', String(error && error.message || error));
      if (window.console) { window.console.error('[charts]', state.figure.id, error); }
    });
  }

  figures.forEach(function (figure) {
    var data = read(figure);
    if (!data) { return; }
    // the card: the header (legend, range, Reset) over the frame the plot is drawn in
    var card = document.createElement('div');
    card.className = 'mj-chart-card';
    var head = document.createElement('div');
    head.className = 'mj-chart-head';
    head.hidden = true;
    var box = document.createElement('div');
    var label = (figure.getAttribute('data-chart-label') || 'Chart') +
      '. The same figures are in the table that follows.';
    box.className = 'mj-chart';
    if (figure.id) { box.id = figure.id + '-frame'; }
    box.setAttribute('role', 'img');
    box.setAttribute('aria-label', label);
    // the frame takes focus, so the keyboard reaches what the pointer does
    box.setAttribute('tabindex', '0');
    box.setAttribute('data-chart-frame', '');
    box.setAttribute('aria-keyshortcuts', KEYS);
    card.appendChild(head);
    card.appendChild(box);
    // what the tooltip says, said again for a reader who hears the page
    var live = document.createElement('span');
    live.className = 'mj-chart-live';
    live.setAttribute('role', 'status');
    live.setAttribute('aria-live', 'polite');
    var before = figure.querySelector('figcaption');
    var at = before && before === figure.firstElementChild ? before.nextSibling : figure.firstChild;
    figure.insertBefore(card, at);
    figure.insertBefore(live, card.nextSibling);
    var state = {
      figure: figure, data: data, card: card, head: head, box: box, live: live, label: label,
      shape: shape(figure), dates: stamps(data.categories),
      chart: null, echarts: null, drawn: false, requested: false, selected: {}, window: null,
      cursor: null, pointing: null, hover: null, focused: null, faded: null, zoomable: false,
      tools: null, reset: null, range: null, keys: null, readout: null, gap: null, tip: '',
      tokens: null, relabelling: false, animation: !still(), pinch: 0, pan: 0, dragged: false,
      entered: false, revealed: false, lit: 0, slider: null, veiled: [], crosshair: false,
      cursoring: false, banding: false, float: '', strip: ''
    };
    data.series.forEach(function (s) { state.selected[s.name] = true; });
    if (state.shape.horizontal) { rows(state); }
    box.addEventListener('keydown', function (event) { key(state, event); });
    // not passive: a ctrl or shift wheel over a zoomable plot is the chart's, and is cancelled
    box.addEventListener('wheel', function (event) { wheel(state, event); }, { passive: false });
    box.addEventListener('pointerdown', function (event) { drag(state, event); });
    box.addEventListener('blur', function () {
      if (state.chart) { state.chart.dispatchAction({ type: 'hideTip' }); }
      letGo(state);
    });
    states.push(state);
  });
  if (!states.length) { return; }

  function request(state) {
    if (state.requested) { return; }
    state.requested = true;
    draw(state);
  }

  // `#chart-<name>` is the figure; `#chart-<name>:<row>` is one of its rows, which is an
  // element of the page in its own right. Either way the chart is drawn at once, and a row's
  // mark is highlighted with its tooltip open.
  function address() {
    var hash = window.location.hash.slice(1);
    try { hash = decodeURIComponent(hash); } catch (e) { /* as written */ }
    states.forEach(function (state) {
      var id = state.figure.id;
      var on = Boolean(id) && (hash === id || hash.indexOf(id + ':') === 0);
      state.figure.classList.toggle('is-addressed', on);
      if (!on) { return; }
      if (!state.requested) { request(state); return; }
      if (!state.chart) { return; }
      var row = state.data.ids.indexOf(hash);
      state.chart.dispatchAction({ type: 'downplay' });
      if (row >= 0) {
        state.chart.dispatchAction({ type: 'highlight', dataIndex: row });
        point(state, row);
      }
    });
  }
  window.addEventListener('hashchange', address);
  address();

  if ('IntersectionObserver' in window) {
    var seen = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (!entry.isIntersecting) { return; }
        seen.unobserve(entry.target);
        for (var i = 0; i < states.length; i++) {
          if (states[i].figure === entry.target) { request(states[i]); }
        }
      });
    }, { rootMargin: '300px 0px' });
    states.forEach(function (state) { seen.observe(state.figure); });
  } else {
    states.forEach(request);
  }

  // Draw again in the other palette when the theme changes, and to the new width on resize:
  // a width that changes whether the categories fit draws the chart again, with or without
  // its zoom.
  new MutationObserver(function () {
    var theme = isDark() ? 'dark' : 'light';
    states.forEach(function (state) {
      var drawnIn = state.figure.getAttribute('data-chart-rendered');
      if (state.chart && drawnIn !== theme) { draw(state); }
    });
  }).observe(root, { attributes: true, attributeFilter: ['class'] });

  if ('ResizeObserver' in window) {
    var resized = new ResizeObserver(function (entries) {
      entries.forEach(function (entry) {
        for (var i = 0; i < states.length; i++) {
          var state = states[i];
          if (state.box !== entry.target || !state.chart) { continue; }
          // a width that changes whether the categories fit, or whether the card is a phone's
          // (whose tooltip is docked), draws the chart again
          if (crowded(state) !== state.zoomable || narrow(state) !== state.small) {
            draw(state);
            continue;
          }
          state.chart.resize();
          relabel(state);
        }
      });
    });
    states.forEach(function (state) { resized.observe(state.box); });
  }

  // Reduced motion is followed when it changes, not only when the page loads.
  function moved() {
    states.forEach(function (state) {
      if (!state.chart) { return; }
      var speed = still() ? 0 : duration();
      state.animation = !still();
      state.chart.setOption({
        animation: state.animation, animationDuration: 0,
        animationDurationUpdate: speed * 2, tooltip: { transitionDuration: speed / 1000 }
      });
    });
  }
  if (motion) {
    if (typeof motion.addEventListener === 'function') {
      motion.addEventListener('change', moved);
    } else if (typeof motion.addListener === 'function') {
      motion.addListener(moved);
    }
  }

  function find(id) {
    for (var i = 0; i < states.length; i++) {
      if (states[i].figure.id === id) { return states[i]; }
    }
    return null;
  }

  // What a test or a probe may ask: the charts by id, what each one read from its table and
  // what a reader has done to it; and two acts a probe cannot aim at a canvas for.
  window.mjCharts = {
    ids: function () { return states.map(function (s) { return s.figure.id; }); },
    state: function (id) {
      var s = find(id);
      if (!s) { return null; }
      var w = s.window || whole(s);
      return {
        id: id, rendered: Boolean(s.chart), kind: s.shape.kind,
        categories: s.data.categories.slice(),
        series: s.data.series.map(function (x, i) {
          return { name: x.name, data: x.data.slice(), visible: visible(s, i),
                   colour: s.tokens ? s.tokens.colours[i] : '',
                   hatched: Boolean(s.tokens && s.tokens.hatch[i]) };
        }),
        links: s.data.links.slice(), zoomable: s.zoomable, window: [w[0], w[1]],
        whole: full(s), cursor: s.cursor, tip: s.tip, animation: s.animation
      };
    },
    // hide or show one series, as its legend entry does
    toggle: function (id, name) {
      var s = find(id);
      if (s && s.chart) { s.chart.dispatchAction({ type: 'legendToggleSelect', name: name }); }
    },
    // zoom to a window of categories, as the slider does
    zoom: function (id, start, end) {
      var s = find(id);
      if (s) { zoomTo(s, start, end); }
    }
  };
})();

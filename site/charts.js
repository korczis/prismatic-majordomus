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
// and none is chosen here, except a state of no coloured meaning (`--mj-neutral`), which takes
// a series colour from the end of the series; any other series takes the series colour of its place
// among the unmarked columns. A colour belongs to the column, never to its rank among the
// series shown, so hiding one series repaints none of the others. Every colour is a token of
// the page, read back from the page.
//
// A chart is read by pointing at it (I4400). Pointing at a category draws a crosshair (a
// shaded band on bars) and one tooltip naming every visible series there, in the figure's
// unit. The legend is a row of toggle buttons under the frame (the page's own controls, so
// the keyboard reaches them, they wrap at any width and a hidden series is struck through,
// not only paler); pressing one hides or shows its series and the axis rescales to what is
// left. A line chart with more categories than its width holds can be zoomed and panned
// (the slider, a trackpad pinch or ctrl + wheel, shift + wheel, a mouse drag, the keyboard)
// and a Reset button gives the whole range back; a plain wheel always scrolls the page. The
// hovered series is focused and the rest fade; the latest value of each line is marked and
// labelled, and a measured value with no measured neighbour is a dot, since it has no line.
// The frame takes focus: the arrow keys, Home and End move along the categories with the
// tooltip following, + and - zoom, 0 resets, Escape lets go; a key held with ctrl, alt or
// meta is the browser's (page zoom, Back) and is left to it. Motion is the design's one
// duration, and none at all under prefers-reduced-motion, followed when it changes.
//
// The table stays on the page. It is the data, and it is what a reader without this script,
// a screen reader and a printout get; the chart is a second view of it. A row whose first
// cell is a link makes its mark a link to the same page. A row that carries an id
// (`chart-<name>:<row>`) is addressable: the fragment lands on the row with no script at
// all, and with it the mark is highlighted too.
//
// Apache ECharts is vendored at build (scripts/site-build) and is not in any page's own
// scripts: it is fetched the first time a chart nears the viewport, once per page.
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
  // A value label is this many pixels tall; two latest-value labels closer than that are
  // moved apart (the common ECharts build carries no label layout of its own).
  var LABEL_HEIGHT = 15;
  // How far a pinch or a ctrl + wheel turn must travel, in pixels, to double or halve the
  // window: a trackpad sends many small deltas, a mouse wheel few large ones.
  var PINCH = 200;
  // The keys the frame answers, as aria-keyshortcuts names them; a zoomable chart adds its own.
  var KEYS = 'ArrowLeft ArrowRight ArrowUp ArrowDown Home End Escape';
  var ZOOM_KEYS = ' Plus - 0';
  var motion = window.matchMedia ? window.matchMedia('(prefers-reduced-motion: reduce)') : null;

  function isDark() { return root.classList.contains(root.dataset.themeClass); }
  function still() { return Boolean(motion && motion.matches); }

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
  function token(name) {
    if (!getComputedStyle(root).getPropertyValue(name).trim()) { return ''; }
    probe.style.color = 'var(' + name + ')';
    document.body.appendChild(probe);
    var resolved = getComputedStyle(probe).color;
    probe.remove();
    return hex(resolved);
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
      font: getComputedStyle(document.body).fontFamily
    };
    // each column's colour: the status it is marked with, else its slot of the series. A
    // status filed under no coloured meaning (`--mj-neutral`) carries no colour, and two of
    // them must still be told apart and still be seen: a gray of the frame (--mj-line) is the
    // border and the hover band, so each neutral column takes a series colour instead, counted
    // from the end of the series so it meets no unmarked column's slot first
    var neutral = token('--mj-neutral'), quiet = 0;
    t.colours = state.data.series.map(function (s) {
      if (s.marked) {
        var colour = hex(getComputedStyle(s.marked).color);
        if (colour !== neutral) { return colour; }
        if (!t.series.length) { return t.muted; }
        quiet += 1;
        return t.series[(t.series.length - quiet % t.series.length) % t.series.length];
      }
      return t.series.length ? t.series[s.slot % t.series.length] : t.fg;
    });
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

  // A bar's rounded end is the one away from the baseline, and in a stack only the outermost
  // visible segment of its sign has one: the rest meet square, the page showing between them.
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
  function bars(state, i) {
    return state.data.series[i].data.map(function (value, row) {
      if (value == null) { return null; }
      return { value: value, itemStyle: { borderRadius: radius(state, i, row) } };
    });
  }

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

  // The latest value of each visible line: a dot on it and its value beside it, while the
  // window shows it. Labels closer than one line of text are moved apart, in their order.
  // A lone value (above) is a smaller dot with no label.
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
    return marks.map(function (m) {
      var data = m.shown ? [{ coord: [m.at, m.value], value: m.value }] : [];
      var values = state.data.series[m.i].data;
      m.lone.forEach(function (k) {
        data.push({ coord: [k, values[k]], value: values[k], symbolSize: 6,
                    itemStyle: { borderWidth: 1 }, label: { show: false } });
      });
      return {
        silent: true, symbol: 'circle', symbolSize: 8, animation: !still(),
        itemStyle: { color: t.colours[m.i], borderColor: t.bg, borderWidth: 2 },
        label: {
          show: true, position: 'right', distance: 6, offset: [0, m.dy],
          color: t.fg, fontFamily: t.font, fontSize: 12, fontWeight: 600,
          formatter: function () { return amount(m.value, state.shape.unit); }
        },
        emphasis: { disabled: true },
        data: data
      };
    });
  }

  // The room the latest-value labels need right of the plot.
  function endRoom(state) {
    var widest = 0;
    state.data.series.forEach(function (s) {
      var at = lastIndex(s.data);
      if (at >= 0) { widest = Math.max(widest, amount(s.data[at], state.shape.unit).length); }
    });
    return 20 + widest * 7;
  }

  // One tooltip for the category: every visible series, keyed by a short stroke of its
  // colour, the value set strong and the name quiet; the series under the pointer is in
  // the strong ink too. A stack adds its whole. The same words go to the live region.
  function tip(state, items, asText) {
    var rows = [], total = 0, any = false;
    var unit = state.shape.unit;
    var t = state.tokens;
    var index = items.length ? items[0].dataIndex : -1;
    var heading = index >= 0 ? state.data.categories[index] : '';
    items.forEach(function (item) {
      var s = state.data.series[item.seriesIndex];
      if (!s) { return; }
      var value = s.data[item.dataIndex];
      if (value != null) { total += value; any = true; }
      rows.push({ name: s.name, value: amount(value, unit), colour: t.colours[item.seriesIndex],
                  hot: state.hover === item.seriesIndex });
    });
    if (state.shape.stacked && rows.length > 1) {
      rows.push({ name: 'total', value: amount(any ? total : null, unit), colour: '',
                  hot: false, total: true });
    }
    if (asText) {
      return heading + ': ' + rows.map(function (r) { return r.name + ' ' + r.value; }).join(', ');
    }
    return '<div class="mj-chart-tip">' +
      '<div style="margin-bottom:4px;color:' + t.muted + '">' + escapeHtml(heading) + '</div>' +
      rows.map(function (r) {
        var key = '<span style="display:inline-block;width:10px;height:2px;margin-right:8px;' +
          'vertical-align:middle;border-radius:1px' + (r.colour ? ';background:' + r.colour : '') +
          '"></span>';
        return '<div style="display:flex;align-items:center;justify-content:space-between;' +
          'gap:16px' + (r.total ? ';margin-top:4px;padding-top:4px;border-top:1px solid ' +
          t.line : '') + '">' +
          '<span style="color:' + (r.hot ? t.fg : t.muted) + '">' + key + escapeHtml(r.name) +
          '</span><span style="font-weight:600;color:' + t.fg +
          ';font-variant-numeric:tabular-nums">' + escapeHtml(r.value) + '</span></div>';
      }).join('') + '</div>';
  }

  function option(state) {
    var data = state.data, kind = state.shape.kind, unit = state.shape.unit;
    var horizontal = state.shape.horizontal, stacked = state.shape.stacked;
    var t = state.tokens = palette(state);
    var many = data.series.length > 1;
    var narrow = state.box.clientWidth < 480;
    var text = { color: t.muted, fontFamily: t.font, fontSize: 12 };
    state.zoomable = crowded(state);
    if (!state.zoomable) { state.window = null; }
    var w = state.window || whole(state);
    var category = {
      type: 'category', data: data.categories, inverse: horizontal, boundaryGap: kind === 'bar',
      axisLine: { show: true, lineStyle: { color: t.line } }, axisTick: { show: false },
      axisLabel: Object.assign({
        hideOverlap: true, width: horizontal ? (narrow ? 96 : 200) : undefined,
        overflow: 'truncate'
      }, text)
    };
    var value = {
      type: 'value', scale: kind === 'line',
      name: unit === '%' ? '' : unit, nameLocation: 'middle', nameGap: horizontal ? 28 : 44,
      nameTextStyle: text, axisLine: { show: false }, axisTick: { show: false },
      axisLabel: Object.assign({
        formatter: function (v) { return unit === '%' ? v + '%' : String(v); }
      }, text),
      splitLine: { lineStyle: { color: t.line, width: 1 } }
    };
    // a line is read at an x, so the pointer is a crosshair labelled in the unit; a bar is
    // read as a mark, so the pointer is the band behind its category
    var pointer = kind === 'line'
      ? {
        type: 'cross', snap: true,
        lineStyle: { color: t.muted, width: 1, type: 'dashed' },
        crossStyle: { color: t.muted, width: 1, type: 'dashed' },
        label: {
          backgroundColor: t.fg, color: t.bg, fontFamily: t.font, fontSize: 11,
          padding: [3, 6], borderRadius: 4,
          formatter: function (p) {
            return p.axisDimension === 'y' ? amount(Number(p.value), unit) : String(p.value);
          }
        }
      }
      // the band is drawn under the bars (ECharts draws a pointer above the series by default)
      : { type: 'shadow', z: 0, shadowStyle: { color: t.sunken, opacity: 1 } };
    var speed = still() ? 0 : duration();
    var o = {
      animation: !still(),
      animationDuration: speed * 3, animationEasing: 'cubicOut',
      animationDurationUpdate: speed * 2, animationEasingUpdate: 'cubicOut',
      backgroundColor: 'transparent',
      textStyle: text,
      color: t.colours,
      aria: { enabled: true, label: { description: state.label } },
      grid: {
        left: 8, right: kind === 'line' ? endRoom(state) : 16, top: 16,
        bottom: state.zoomable ? 48 : 8, containLabel: true
      },
      // the selection lives in ECharts' legend, which filters the series and rescales the
      // axis; what a reader sees and presses is the row of buttons under the frame (keys()),
      // because an entry drawn on the canvas can be neither focused nor wrapped
      legend: many ? {
        show: false, selectedMode: true, selected: Object.assign({}, state.selected),
        data: data.series.map(function (s) { return s.name; })
      } : undefined,
      tooltip: {
        trigger: 'axis', axisPointer: pointer, confine: true, transitionDuration: speed / 1000,
        backgroundColor: t.raised, borderColor: t.line, borderWidth: 1, padding: [8, 10],
        textStyle: { color: t.fg, fontFamily: t.font, fontSize: 12 },
        extraCssText: 'border-radius:8px;min-width:10rem;',
        formatter: function (items) {
          var list = Array.isArray(items) ? items : [items];
          if (list.length) { state.cursor = list[0].dataIndex; }
          state.tip = tip(state, list, true);
          return tip(state, list);
        }
      },
      xAxis: horizontal ? value : category,
      yAxis: horizontal ? category : value,
      series: data.series.map(function (s, i) {
        var colour = t.colours[i];
        var base = {
          id: 'series-' + i, name: s.name, type: kind,
          emphasis: { focus: 'series', blurScope: 'coordinateSystem' }
        };
        if (kind === 'line') {
          // straight segments between measured points (nothing between two was measured),
          // a gap where a value is missing, symbols only where the pointer is
          return Object.assign(base, {
            data: s.data, smooth: false, connectNulls: false, triggerLineEvent: true,
            showSymbol: false, symbol: 'circle', symbolSize: 8,
            lineStyle: { width: 2, color: colour },
            itemStyle: { color: colour, borderColor: t.bg, borderWidth: 2 },
            areaStyle: t.area > 0 ? {
              opacity: t.area,
              // to the same colour at no opacity: a fade to `transparent` would pass through
              // black and grey the fill
              color: new state.echarts.graphic.LinearGradient(0, 0, 0, 1, [
                { offset: 0, color: colour }, { offset: 1, color: colour + '00' }
              ])
            } : undefined,
            blur: { lineStyle: { opacity: 0.2 }, areaStyle: { opacity: t.area / 4 } }
          });
        }
        return Object.assign(base, {
          data: bars(state, i), stack: stacked ? 'whole' : undefined,
          barMaxWidth: 28, barGap: '20%',
          // a 1px border in the page's colour on each side is the 2px gap between segments
          itemStyle: { color: colour, borderColor: t.bg, borderWidth: stacked ? 1 : 0 },
          blur: { itemStyle: { opacity: 0.35 } }
        });
      })
    };
    if (kind === 'line') {
      var marks = ends(state, t);
      o.series.forEach(function (s, i) { s.markPoint = marks[i]; });
    }
    if (state.zoomable) {
      // The slider only. ECharts' `inside` zoom cancels every wheel event over the plot,
      // whatever its ctrl and shift settings say, so the page would stop scrolling under the
      // pointer; the wheel, the pinch and the drag are this file's (wheel(), drag()), and a
      // plain wheel is left to the page.
      o.dataZoom = [
        {
          type: 'slider', xAxisIndex: 0, filterMode: 'filter', startValue: w[0], endValue: w[1],
          minValueSpan: 2, height: 20, bottom: 8, left: 16, right: 16, brushSelect: false,
          borderColor: t.line, borderRadius: 4, backgroundColor: 'transparent',
          fillerColor: 'transparent',
          dataBackground: {
            lineStyle: { color: t.faint, width: 1 }, areaStyle: { color: t.sunken, opacity: 1 }
          },
          selectedDataBackground: {
            lineStyle: { color: t.accent, width: 1 },
            areaStyle: { color: t.accentSoft, opacity: 1 }
          },
          handleStyle: { color: t.raised, borderColor: t.accentFill, borderWidth: 1 },
          moveHandleSize: 4, moveHandleStyle: { color: t.line, opacity: 1 },
          emphasis: {
            handleStyle: { borderColor: t.accent }, moveHandleStyle: { color: t.accent }
          },
          textStyle: { color: t.muted, fontFamily: t.font, fontSize: 11 },
          labelFormatter: function (index) { return data.categories[index] || ''; }
        }
      ];
    }
    state.animation = o.animation;
    return o;
  }

  // ------------------------------------------------------------------ the controls

  // The Reset row the build wrote, as a template, into every page that carries a chart
  // (site/templates/partials/charts.html): the markup the interaction check counts is the
  // markup a zoomable chart shows. A page without the template gets the same row, made here.
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
    button.addEventListener('click', function () { reset(state); });
    var after = state.keys || state.box;
    after.parentNode.insertBefore(row, after.nextSibling);
    state.tools = row;
    state.reset = button;
    state.range = row.querySelector('.mj-chart-range');
    return row;
  }

  // The legend: one toggle button per series, in a row under the frame, from the template the
  // build wrote (partials/charts.html) or made here. A button says whether its series is shown
  // with aria-pressed, which the stylesheet draws as a struck-through name and a hollow key,
  // so the state is never told by a colour alone; its key is the series colour, a stroke for
  // a line and a block for a bar, as the mark is.
  function keyButton(model) {
    if (model) { return model.cloneNode(true); }
    var button = document.createElement('button');
    button.type = 'button';
    button.className = 'mj-chart-key';
    button.setAttribute('data-chart-action', 'series');
    var mark = document.createElement('span');
    mark.className = 'mj-chart-key-mark';
    mark.setAttribute('aria-hidden', 'true');
    button.appendChild(mark);
    var name = document.createElement('span');
    name.className = 'mj-chart-key-name';
    button.appendChild(name);
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
      button.addEventListener('click', function () {
        if (!state.chart) { return; }
        state.chart.dispatchAction({ type: 'legendToggleSelect', name: s.name });
      });
      // pointing at a key focuses its series on the chart, as pointing at the series does
      var focus = function () {
        if (state.chart && visible(state, i)) {
          state.chart.dispatchAction({ type: 'highlight', seriesIndex: i });
        }
      };
      var blur = function () {
        if (state.chart) { state.chart.dispatchAction({ type: 'downplay', seriesIndex: i }); }
      };
      button.addEventListener('mouseenter', focus);
      button.addEventListener('focus', focus);
      button.addEventListener('mouseleave', blur);
      button.addEventListener('blur', blur);
      row.appendChild(button);
    });
    state.box.parentNode.insertBefore(row, state.box.nextSibling);
    state.keys = row;
    return row;
  }
  // Each key's colour and state, again after a redraw (another theme) and after a toggle.
  function paintKeys(state) {
    if (!keys(state)) { return; }
    state.keys.hidden = false;
    var buttons = state.keys.querySelectorAll('[data-chart-action="series"]');
    Array.prototype.forEach.call(buttons, function (button, i) {
      button.setAttribute('aria-pressed', visible(state, i) ? 'true' : 'false');
      var colour = state.tokens ? state.tokens.colours[i] : '';
      if (colour && typeof button.style.setProperty === 'function') {
        button.style.setProperty('--chart-key', colour);
      }
    });
  }

  function showTools(state) {
    if (!state.zoomable && !state.tools) { return; }
    var row = tools(state);
    row.hidden = !state.zoomable;
    var w = state.window || whole(state);
    state.reset.disabled = full(state);
    if (state.range) {
      var c = state.data.categories;
      state.range.textContent = c[w[0]] + ' – ' + c[w[1]] + ' · ' + (w[1] - w[0] + 1) +
        ' of ' + c.length;
    }
  }

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

  // The latest-value marks again after the scale changed: a zoom, a series hidden or shown.
  function relabel(state) {
    if (!state.chart || state.shape.kind !== 'line' || state.relabelling) { return; }
    state.relabelling = true;
    (window.requestAnimationFrame || window.setTimeout)(function () {
      state.relabelling = false;
      if (!state.chart || !state.drawn) { return; }
      var marks = ends(state, state.tokens);
      state.chart.setOption({ series: marks.map(function (m, i) {
        return { id: 'series-' + i, markPoint: m };
      }) });
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
        event.preventDefault();
        return;
      default: return;
    }
    event.preventDefault();
    point(state, at);
  }

  // Point at one category: the window pans to hold it, the tooltip opens on it and the live
  // region says what the tooltip says.
  function point(state, at) {
    var w = state.window || whole(state);
    if (state.zoomable && (at < w[0] || at > w[1])) {
      var span = w[1] - w[0];
      var start = at < w[0] ? at : at - span;
      zoomTo(state, start, start + span);
    }
    state.cursor = at;
    var items = [], anchor = -1;
    state.data.series.forEach(function (s, i) {
      if (!visible(state, i)) { return; }
      items.push({ seriesIndex: i, dataIndex: at });
      if (anchor < 0 && s.data[at] != null) { anchor = i; }
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
      // the click that ends a drag is not a click on the mark under it
      window.setTimeout(function () { state.dragged = false; }, 0);
    }
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    window.addEventListener('pointercancel', up);
  }

  // ------------------------------------------------------------------ drawing

  function draw(state) {
    return loadECharts().then(function (echarts) {
      if (state.chart) { state.chart.dispose(); }
      state.echarts = echarts;
      state.drawn = false;
      state.box.hidden = false;
      state.chart = echarts.init(state.box);
      state.chart.setOption(option(state));
      state.drawn = true;
      state.figure.setAttribute('data-chart-rendered', isDark() ? 'dark' : 'light');
      // the window a reader chose survives a redraw (the option carried it); the marks are
      // placed now that the chart has pixels
      sync(state);
      relabel(state);
      paintKeys(state);
      // a mark whose row is a link goes where the row's link goes
      state.chart.on('click', function (params) {
        if (state.dragged) { return; }
        var href = state.data.links[params.dataIndex];
        if (href) { window.location.assign(href); }
      });
      state.chart.on('mouseover', function (params) {
        if (params.componentType === 'series') { state.hover = params.seriesIndex; }
      });
      state.chart.on('mouseout', function () { state.hover = null; });
      // a series hidden or shown: the others keep their colours (each is fixed on its own
      // series), the axis rescales to what is left, a stack rounds its new outermost segments
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
      // a link is a pointer; a zoomable plot can be dragged
      state.chart.getZr().on('mousemove', function (event) {
        if (state.dragged) { return; }
        var at = state.chart.convertFromPixel({ gridIndex: 0 }, [event.offsetX, event.offsetY]);
        var i = at ? Math.round(state.shape.horizontal ? at[1] : at[0]) : -1;
        var plot = typeof state.chart.containPixel !== 'function' ||
          state.chart.containPixel({ gridIndex: 0 }, [event.offsetX, event.offsetY]);
        state.chart.getZr().setCursorStyle(state.data.links[i] ? 'pointer'
          : state.zoomable && plot ? 'grab' : 'default');
      });
      address();
    }).catch(function (error) {
      // the table is the chart's fallback and it never left the page
      state.box.hidden = true;
      if (state.tools) { state.tools.hidden = true; }
      if (state.keys) { state.keys.hidden = true; }
      state.figure.setAttribute('data-chart-error', String(error && error.message || error));
      if (window.console) { window.console.error('[charts]', state.figure.id, error); }
    });
  }

  figures.forEach(function (figure) {
    var data = read(figure);
    if (!data) { return; }
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
    if (figure.hasAttribute('data-chart-horizontal')) {
      // one readable row per category, however many there are
      box.style.height = Math.max(320, data.categories.length * 26 + 72) + 'px';
    }
    // what the tooltip says, said again for a reader who hears the page
    var live = document.createElement('span');
    live.className = 'mj-chart-live';
    live.setAttribute('role', 'status');
    live.setAttribute('aria-live', 'polite');
    var before = figure.querySelector('figcaption');
    var at = before && before === figure.firstElementChild ? before.nextSibling : figure.firstChild;
    figure.insertBefore(box, at);
    figure.insertBefore(live, box.nextSibling);
    var state = {
      figure: figure, data: data, box: box, live: live, label: label, shape: shape(figure),
      chart: null, echarts: null, drawn: false, requested: false, selected: {}, window: null,
      cursor: null, hover: null, zoomable: false, tools: null, reset: null, range: null, tip: '',
      tokens: null, relabelling: false, animation: !still(), keys: null, pinch: 0, pan: 0,
      dragged: false
    };
    data.series.forEach(function (s) { state.selected[s.name] = true; });
    box.addEventListener('keydown', function (event) { key(state, event); });
    // not passive: a ctrl or shift wheel over a zoomable plot is the chart's, and is cancelled
    box.addEventListener('wheel', function (event) { wheel(state, event); }, { passive: false });
    box.addEventListener('pointerdown', function (event) { drag(state, event); });
    box.addEventListener('blur', function () {
      if (state.chart) { state.chart.dispatchAction({ type: 'hideTip' }); }
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
          if (crowded(state) !== state.zoomable) { draw(state); continue; }
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
        animation: state.animation, animationDuration: speed * 3,
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
                   colour: s.tokens ? s.tokens.colours[i] : '' };
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

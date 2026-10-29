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
// and none is chosen here; any other series takes the next of the design's series colours.
// Every colour is a token of the page, read back from the page. At most six of the series
// colours are drawn: the seventh fails the contrast and colour-blind checks the palette was
// measured with, so a seventh such series stays in the table and the figure says so. A
// status series is not counted against them: its colour is its state's, and its name, in the
// legend and the card, tells it from another state drawn in a near colour.
//
// The table stays on the page. It is the data, and it is what a reader without this script,
// a screen reader and a printout get; the chart is a second view of it. A row whose first
// cell is a link makes its band a link to the same page. A row that carries an id
// (`chart-<name>:<row>`) is addressable: the fragment lands on the row with no script at
// all, and with it the mark is highlighted too.
//
// The chart is read the same way with any hand. The legend above it is a row of buttons that
// show and hide their series (and preview one while pointed at); the drawing takes the focus,
// its arrow keys step a cursor through the categories and say each one in a polite live
// readout, Enter follows the row's link and Escape lets go. A long line zooms with Ctrl or
// Cmd and the wheel or a pinch, never with the plain wheel, which scrolls the page.
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
  var lib = null;
  var states = [];
  var SERIES_TOKENS = ['--mj-series-1', '--mj-series-2', '--mj-series-3', '--mj-series-4',
                       '--mj-series-5', '--mj-series-6', '--mj-series-7', '--mj-series-8'];
  // the series colours that pass the palette's checks in both themes
  var SERIES_DRAWN = 6;
  // a line this long is read a stretch at a time, never narrower than this many categories
  var LONG = 30;
  var SPAN = 4;
  // a series' marks step back to this while another is shown: the relation graphs' fade
  var FADED = 0.15;
  var STATUS_CLASS = /^mj-(badge|status)--/;
  var CONTRAST = '(forced-colors: active), (prefers-contrast: more)';
  // the tooltip's elevation: the ink of the page, faded, so it is one in either theme
  var LIFT = 'box-shadow: 0 1px 2px color-mix(in oklch, var(--mj-fg) 8%, transparent), ' +
             '0 8px 24px -8px color-mix(in oklch, var(--mj-fg) 22%, transparent);';

  function isDark() { return root.classList.contains(root.dataset.themeClass); }
  function query(q) { return Boolean(window.matchMedia) && window.matchMedia(q).matches; }
  function still() { return query('(prefers-reduced-motion: reduce)'); }
  function coarse() { return query('(pointer: coarse)'); }

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
    return '#' + [px[0], px[1], px[2]].map(function (c) { return (c < 16 ? '0' : '') + c.toString(16); }).join('');
  }
  // Any colour expression over the tokens, resolved where it is placed: in the body, or in a
  // status badge, where `currentColor` is that state's ink.
  function resolve(expression, host) {
    probe.style.color = expression;
    (host || document.body).appendChild(probe);
    var resolved = getComputedStyle(probe).color;
    probe.remove();
    probe.style.color = '';
    return hex(resolved);
  }
  function token(name) {
    if (!getComputedStyle(root).getPropertyValue(name).trim()) { return ''; }
    return resolve('var(' + name + ')');
  }
  // A role over a ground, e.g. a gridline that recedes: opaque, and resolved by the browser.
  function mix(name, pct, ground) {
    return resolve('color-mix(in oklch, var(' + name + ') ' + pct + '%, var(' + ground + '))');
  }
  // A length token in pixels (a radius, a step of the type scale), resolved by the browser.
  function len(name) {
    probe.style.borderTopLeftRadius = 'var(' + name + ')';
    document.body.appendChild(probe);
    var px = parseFloat(getComputedStyle(probe).borderTopLeftRadius) || 0;
    probe.remove();
    probe.style.borderTopLeftRadius = '';
    return px;
  }
  // The design's one duration, or none at all for a reader who asked for stillness.
  function motion() {
    return still() ? 0 : (parseFloat(getComputedStyle(root).getPropertyValue('--mj-motion-fast')) || 0);
  }
  function sans() {
    return getComputedStyle(root).getPropertyValue('--font-sans').trim() || getComputedStyle(document.body).fontFamily;
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
        if (usable(window.echarts)) { resolve(window.echarts); } else { reject(new Error('the ECharts build defined nothing')); }
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

  // A number as a reader is shown it: at most two decimals, thousands grouped as the tables
  // group them; with its unit, `%` against the number and a word after a space.
  function numeral(v) {
    var parts = String(Math.round(v * 100) / 100).split('.');
    parts[0] = parts[0].replace(/\B(?=(\d{3})+(?!\d))/g, ',');
    return parts.join('.');
  }
  function amount(v, unit) {
    if (v == null) { return 'not recorded'; }
    return numeral(v) + (unit ? (unit === '%' ? '' : ' ') + unit : '');
  }
  function escape(text) {
    return lib.format.encodeHTML(String(text));
  }

  // The table as the chart reads it: categories, the marked series and each row's link. A
  // status series keeps its badge, which is where its colour is read; every other series
  // takes the next series slot in the order the columns are declared.
  function read(figure) {
    var table = figure.querySelector('table');
    if (!table || !table.tHead || !table.tBodies.length) { return null; }
    var heads = Array.prototype.slice.call(table.tHead.rows[table.tHead.rows.length - 1].cells);
    var series = [];
    var next = 0;
    heads.forEach(function (th, column) {
      if (!th.hasAttribute('data-series')) { return; }
      var marked = th.querySelector('.mj-badge, .mj-status');
      series.push({
        column: column,
        name: th.getAttribute('data-series') || th.textContent.trim(),
        mark: marked,
        status: marked ? Array.prototype.filter.call(marked.classList, function (c) { return STATUS_CLASS.test(c); }).join(' ') : '',
        slot: marked ? 0 : ++next,
        data: []
      });
    });
    if (!series.length) { return null; }
    var categories = [], links = [], ids = [];
    Array.prototype.forEach.call(table.tBodies[0].rows, function (row) {
      var first = row.cells[0];
      if (!first) { return; }
      var anchor = first.querySelector('a[href]');
      categories.push(first.getAttribute('data-category') || first.textContent.trim().replace(/\s+/g, ' '));
      links.push(anchor ? anchor.href : null);
      ids.push(row.id || null);
      series.forEach(function (s) { s.data.push(row.cells[s.column] ? number(row.cells[s.column]) : null); });
    });
    return categories.length ? { categories: categories, series: series, links: links, ids: ids } : null;
  }

  // A series' colour, or the same colour faded over the page for the area under a line. A
  // status badge's ink is the far step of its state (near black as a fill in the light theme),
  // and its line the near one, so a status series is drawn between the two: the state's own
  // middle step, in either theme. The badge sets both, so the probe is resolved inside it. A
  // word the design files no state for falls back as the swatch in figure.css does.
  var STATUS_FILL = 'color-mix(in oklch, var(--mj-status-fg, var(--mj-muted)) 60%, var(--mj-status-line, var(--mj-bg)))';
  function ink(s, fade) {
    if (s.mark) {
      return resolve(fade ? 'color-mix(in oklch, ' + STATUS_FILL + ' ' + fade + '%, var(--mj-bg))' : STATUS_FILL, s.mark);
    }
    var name = SERIES_TOKENS[s.slot - 1];
    return fade ? mix(name, fade, '--mj-bg') : token(name);
  }
  // The swatch beside a series' name: its colour comes from the stylesheet, by slot or status.
  function swatch(s) {
    return s.mark ? '<span class="mj-chart-swatch ' + s.status + '" aria-hidden="true"></span>'
                  : '<span class="mj-chart-swatch" data-series-slot="' + s.slot + '" aria-hidden="true"></span>';
  }
  function shown(state) {
    return state.drawn.filter(function (s) { return !state.hidden[s.name]; });
  }
  function horizontal(state) {
    return state.figure.hasAttribute('data-chart-horizontal') && state.figure.getAttribute('data-chart') !== 'line';
  }

  // One category in words, for the readout, and as the tooltip's card.
  function sentence(state, i) {
    return state.data.categories[i] + ': ' + shown(state).map(function (s) {
      return s.name + ' ' + amount(s.data[i], state.unit);
    }).join(', ');
  }
  function tip(state, i) {
    var unit = state.unit, total = 0, rows = shown(state);
    var html = '<div class="mj-chart-tip-head">' + escape(state.data.categories[i]) + '</div>';
    rows.forEach(function (s) {
      if (s.data[i] != null) { total += s.data[i]; }
      html += '<div class="mj-chart-tip-row">' + swatch(s) + '<span class="mj-chart-tip-name">' + escape(s.name) +
              '</span><span class="mj-chart-tip-value">' + escape(amount(s.data[i], unit)) + '</span></div>';
    });
    if (state.stacked && rows.length > 1) {
      html += '<div class="mj-chart-tip-row mj-chart-tip-total"><span class="mj-chart-tip-name">total</span>' +
              '<span class="mj-chart-tip-value">' + escape(amount(total, unit)) + '</span></div>';
    }
    if (state.data.links[i]) { html += '<div class="mj-chart-tip-foot">Enter or click to open</div>'; }
    return html;
  }

  function option(state) {
    var figure = state.figure, data = state.data, drawn = state.drawn;
    var kind = figure.getAttribute('data-chart') === 'line' ? 'line' : 'bar';
    var across = horizontal(state);
    var stacked = state.stacked;
    var unit = state.unit;
    // a short unit (`%`, `min`) is written on every tick; a word (`issues`) names the axis
    var short = unit.length <= 3;
    var bg = token('--mj-bg'), fg = token('--mj-fg'), body = token('--mj-body'), muted = token('--mj-muted');
    var line = token('--mj-line'), raised = token('--mj-raised'), accent = token('--mj-accent');
    var grid = mix('--mj-line', 60, '--mj-bg');
    var family = sans(), size = len('--text-small') || parseFloat(getComputedStyle(document.body).fontSize);
    var text = { color: muted, fontFamily: family, fontSize: size };
    var m = motion();
    var radius = len('--radius-sm');
    // only the value end of a bar is rounded; it stands square on its baseline
    var ends = across ? [0, radius, radius, 0] : [radius, radius, 0, 0];
    var count = data.categories.length;
    state.narrow = state.box.clientWidth < 480;
    // a few lines are named where they end, on a frame wide enough to hold the names
    var labelled = kind === 'line' && drawn.length > 1 && drawn.length <= 4 && !state.narrow;
    var reach = 0;
    if (labelled && paint) {
      paint.font = size + 'px ' + family;
      drawn.forEach(function (s) { reach = Math.max(reach, paint.measureText(s.name).width); });
    }
    var category = {
      type: 'category', data: data.categories, inverse: across, boundaryGap: kind === 'bar',
      axisLine: { show: true, lineStyle: { color: line } }, axisTick: { show: false },
      axisLabel: Object.assign({ hideOverlap: true, margin: 10, width: across ? (state.narrow ? 96 : 200) : undefined, overflow: 'truncate' }, text)
    };
    var value = {
      type: 'value', scale: kind === 'line',
      name: short ? '' : unit, nameLocation: 'middle', nameGap: across ? 28 : 44, nameTextStyle: text,
      axisLine: { show: false }, axisTick: { show: false },
      axisLabel: Object.assign({ formatter: function (v) { return short ? amount(v, unit) : numeral(v); } }, text),
      splitLine: { lineStyle: { color: grid, type: [3, 4] } }
    };
    // In a stack only the outermost shown segment of a category has the rounded end; the
    // segments under it meet the next one square, so the whole reads as one bar.
    var top = data.categories.map(function (_, i) {
      var at = -1;
      drawn.forEach(function (s, k) { if (!state.hidden[s.name] && s.data[i] != null && s.data[i] !== 0) { at = k; } });
      return at;
    });
    var selected = {};
    drawn.forEach(function (s) { selected[s.name] = !state.hidden[s.name]; });
    var colours = drawn.map(function (s) { return ink(s); });
    // ECharts walks an end label along a line as it is drawn in, from the line's first point:
    // a first value not recorded puts the label at NaN for those frames, and the browser says
    // so in the console. Such a chart is drawn at once, its labels already at their ends.
    var gap = labelled && drawn.some(function (s) { return s.data[0] == null; });
    return {
      animation: m > 0 && !gap,
      animationDuration: 4 * m, animationDurationUpdate: 2 * m,
      animationEasing: 'cubicOut', animationEasingUpdate: 'cubicOut',
      backgroundColor: 'transparent',
      textStyle: text,
      color: colours,
      // ECharts' own label would write `role=img` over the frame, which takes the keys as an
      // application: ECharts writes none, and the frame's aria-label is the chart's only one.
      // Patterns are the second channel for a reader who asked for more contrast.
      aria: { enabled: true, label: { enabled: false }, decal: { show: query(CONTRAST) } },
      // the frame's padding is the outer bound: the tick labels and the unit's axis name are
      // laid out inside it (ECharts 6's successor to `containLabel`, which left the name out)
      grid: { left: 8, right: labelled ? Math.ceil(reach) + 20 : 16, top: 16, bottom: 8,
              outerBoundsMode: 'same', outerBoundsContain: 'all' },
      // the legend is the row of buttons above the frame; this one is kept, undrawn, so that
      // the legend actions show and hide a series
      legend: drawn.length > 1 ? { show: false, selected: selected } : undefined,
      // Every wheel that reaches ECharts zooms: the frame stops a plain one on its way down (see
      // the wheel listener below), so what arrives is Ctrl or Cmd with the wheel, or a pinch.
      // ECharts' own 'ctrl' setting would read only ctrlKey, and only after it had already
      // taken the plain wheel from the page.
      dataZoom: state.zoomable ? [{
        type: 'inside', xAxisIndex: 0, start: state.zoom[0], end: state.zoom[1], minValueSpan: SPAN,
        zoomOnMouseWheel: true, moveOnMouseWheel: false, moveOnMouseMove: true, preventDefaultMouseMove: false
      }] : undefined,
      tooltip: {
        trigger: 'axis', confine: true, renderMode: 'html', className: 'mj-chart-tip',
        // a finger taps a band to open its row and reads the table beneath; a pointer gets the card
        triggerOn: coarse() ? 'none' : 'mousemove|click',
        transitionDuration: m / 1000,
        backgroundColor: raised, borderColor: line, borderWidth: 1, padding: [8, 12], borderRadius: len('--radius-md'),
        textStyle: { color: fg, fontFamily: family, fontSize: size },
        extraCssText: LIFT,
        axisPointer: kind === 'line'
          ? { type: 'line', lineStyle: { color: accent, width: 1, type: [4, 4] }, label: { show: false } }
          // under the bars, so the band is lit behind its marks rather than over them
          : { type: 'shadow', z: 1, shadowStyle: { color: token('--mj-accent-soft') } },
        formatter: function (params) {
          var first = [].concat(params)[0];
          return first ? tip(state, first.dataIndex) : '';
        }
      },
      xAxis: across ? value : category,
      yAxis: across ? category : value,
      series: drawn.map(function (s, k) {
        var colour = colours[k];
        // The marks take no pointer of their own: the band is the hit target (wire() below),
        // so a band lights the same wherever in it the pointer is, and only the legend's
        // preview steps the other series back.
        var common = {
          name: s.name, type: kind, stack: stacked ? 'whole' : undefined, silent: true,
          emphasis: { focus: 'series', blurScope: 'coordinateSystem' }
        };
        if (kind === 'bar') {
          return Object.assign(common, {
            barMaxWidth: 20, barCategoryGap: '36%',
            data: stacked ? s.data.map(function (v, i) {
              return v == null ? null : { value: v, itemStyle: { borderRadius: top[i] === k ? ends : 0 } };
            }) : s.data,
            // a stack's segments are parted by a hairline of the ground
            itemStyle: { color: colour, borderRadius: stacked ? 0 : ends, borderColor: bg, borderWidth: stacked ? 1 : 0 },
            blur: { itemStyle: { opacity: FADED } },
            animationDelay: function (idx) { return Math.min(idx * m / 8, m); }
          });
        }
        return Object.assign(common, {
          data: s.data,
          lineStyle: { color: colour, width: 2 },
          itemStyle: { color: colour, borderColor: bg, borderWidth: 2 },
          symbol: 'circle', symbolSize: 7, showSymbol: count <= 24,
          // monotone: the curve never overshoots a recorded point
          smooth: 0.25, smoothMonotone: 'x',
          blur: { lineStyle: { opacity: FADED }, itemStyle: { opacity: FADED }, areaStyle: { opacity: FADED } },
          areaStyle: drawn.length === 1 ? {
            color: new lib.graphic.LinearGradient(0, 0, 0, 1, [{ offset: 0, color: ink(s, 28) }, { offset: 1, color: bg }])
          } : undefined,
          endLabel: labelled ? { show: true, formatter: '{a}', color: body, fontFamily: family, fontSize: size, distance: 8 } : undefined,
          labelLayout: labelled ? { moveOverlap: 'shiftY' } : undefined
        });
      })
    };
  }

  // The category under a point of the frame, or -1 outside the plot.
  function at(state, x, y) {
    var chart = state.chart;
    if (!chart.containPixel({ gridIndex: 0 }, [x, y])) { return -1; }
    var p = chart.convertFromPixel({ gridIndex: 0 }, [x, y]);
    var i = p ? Math.round(horizontal(state) ? p[1] : p[0]) : -1;
    return i >= 0 && i < state.data.categories.length ? i : -1;
  }

  // A long line's window (percent of its length): moved to show category `i`, or narrowed and
  // widened about its middle, or (no factor) opened to the whole. Every window is asked for in
  // percent and never narrower than ECharts allows, so the 'datazoom' event carries the window
  // as drawn and the state is read from the event alone.
  function reveal(state, i) {
    if (!state.zoomable) { return; }
    var last = state.data.categories.length - 1;
    var from = Math.round(state.zoom[0] / 100 * last), to = Math.round(state.zoom[1] / 100 * last);
    if (i >= from && i <= to) { return; }
    var span = to - from;
    from = i < from ? i : i - span;
    state.chart.dispatchAction({ type: 'dataZoom', start: from / last * 100, end: (from + span) / last * 100 });
  }
  function zoom(state, factor) {
    if (!state.zoomable) { return; }
    if (!factor) { state.chart.dispatchAction({ type: 'dataZoom', start: 0, end: 100 }); return; }
    var mid = (state.zoom[0] + state.zoom[1]) / 2;
    var least = 50 * SPAN / (state.data.categories.length - 1);
    var half = Math.min(50, Math.max(least, (state.zoom[1] - state.zoom[0]) * factor / 2));
    var start = Math.max(0, Math.min(100 - 2 * half, mid - half));
    state.chart.dispatchAction({ type: 'dataZoom', start: start, end: start + 2 * half });
  }

  // One category lit, or none (-1): its marks in their emphasis and the band behind them in
  // the axis pointer's shadow. Nothing else steps back: a line's whole course is what it says,
  // and the axis pointer lights and unlights the same marks as it moves, which would undo a
  // fade at every step. The marks are `silent`, so pointing at one lights no more and no less
  // than pointing at the empty end of its band. Stepping the other series back, to FADED, is
  // the legend's preview alone.
  function light(state, i) {
    var chart = state.chart;
    chart.dispatchAction({ type: 'downplay' });
    if (i >= 0) { chart.dispatchAction({ type: 'highlight', dataIndex: i, notBlur: true }); }
  }

  // The cursor: one category lit, its tooltip open and its sentence in the readout. The
  // tooltip is placed by the category's own position, so it opens on any category, one with
  // a gap in a series included.
  function point(state, i) {
    var chart = state.chart, across = horizontal(state);
    state.cursor = i;
    reveal(state, i);
    light(state, i);
    var pos = chart.convertToPixel(across ? { yAxisIndex: 0 } : { xAxisIndex: 0 }, i);
    chart.dispatchAction({ type: 'showTip', x: across ? state.box.clientWidth / 2 : pos, y: across ? pos : state.box.clientHeight / 2 });
    state.readout.textContent = sentence(state, i);
  }

  // The cursor let go, and with it the card, however the card was opened: by the cursor or
  // by the pointer.
  function release(state) {
    state.cursor = -1;
    state.hover = -1;
    state.chart.dispatchAction({ type: 'hideTip' });
    // the band's shadow is the axis pointer's, which a hidden card leaves standing
    state.chart.dispatchAction({ type: 'updateAxisPointer', currTrigger: 'leave' });
    state.chart.dispatchAction({ type: 'downplay' });
  }

  // The keys of a focused chart; `/` and anything with a modifier stay the page's.
  function keys(state, event) {
    if (!state.chart || event.altKey || event.metaKey || event.ctrlKey) { return; }
    var across = horizontal(state), n = state.data.categories.length, i = state.cursor;
    var act = function (f) { if (!state.zoomable) { return; } event.preventDefault(); f(); };
    switch (event.key) {
      case across ? 'ArrowUp' : 'ArrowLeft': i = i < 0 ? 0 : Math.max(0, i - 1); break;
      case across ? 'ArrowDown' : 'ArrowRight': i = i < 0 ? 0 : Math.min(n - 1, i + 1); break;
      case 'Home': i = 0; break;
      case 'End': i = n - 1; break;
      case 'Enter':
        if (i >= 0 && state.data.links[i]) { event.preventDefault(); window.location.assign(state.data.links[i]); }
        return;
      case 'Escape':
        // only a cursor is the chart's to let go of; a card the pointer opened closes too
        if (i >= 0) { event.preventDefault(); }
        release(state);
        return;
      case '+': case '=': act(function () { zoom(state, 0.8); }); return;
      case '-': case '_': act(function () { zoom(state, 1.25); }); return;
      case '0': act(function () { zoom(state, 0); }); return;
      default: return;
    }
    event.preventDefault();
    // every series taken out: there is nothing at any category to say or to show
    if (!shown(state).length) { state.readout.textContent = 'Nothing matches'; return; }
    point(state, i);
  }

  // Draw again in place: the theme, the contrast preference or the frame's width changed.
  // The instance stays, so nothing replays its entrance.
  function restyle(state) {
    if (!state.chart) { return; }
    state.chart.setOption(Object.assign(option(state), { animation: false }), { notMerge: true });
    state.figure.setAttribute('data-chart-rendered', isDark() ? 'dark' : 'light');
  }

  function wire(state) {
    var chart = state.chart, zr = chart.getZr();
    // the whole band of a category is its hit target: it lights as one and opens its row
    zr.on('mousemove', function (event) {
      var i = at(state, event.offsetX, event.offsetY);
      zr.setCursorStyle(state.data.links[i] ? 'pointer' : 'default');
      if (i === state.hover) { return; }
      state.hover = i;
      light(state, i);
    });
    zr.on('globalout', function () {
      if (state.hover < 0) { return; }
      state.hover = -1;
      light(state, state.cursor);
    });
    zr.on('click', function (event) {
      var href = state.data.links[at(state, event.offsetX, event.offsetY)];
      if (href) { window.location.assign(href); }
    });
    chart.on('legendselectchanged', function (event) {
      state.drawn.forEach(function (s) { state.hidden[s.name] = event.selected[s.name] === false; });
      state.keys.forEach(function (key) { key.setAttribute('aria-pressed', String(!state.hidden[key.getAttribute('data-series')])); });
      var none = !shown(state).length;
      if (none) { state.box.setAttribute('data-chart-empty', 'Nothing matches'); } else { state.box.removeAttribute('data-chart-empty'); }
      // the stack's rounded end moves to whichever segment is now the outermost
      chart.setOption(option(state));
      if (state.cursor >= 0) {
        if (none) { release(state); state.readout.textContent = 'Nothing matches'; }
        else { state.readout.textContent = sentence(state, state.cursor); }
      }
    });
    // read from the event: getOption() would copy every series' data at each step of a pan
    chart.on('datazoom', function (event) {
      var dz = event.batch ? event.batch[0] : event;
      if (!dz || dz.start == null || dz.end == null) { return; }
      state.zoom = [dz.start, dz.end];
      if (state.reset) { state.reset.hidden = dz.start <= 0 && dz.end >= 100; }
    });
  }

  // What is the drawing's and stands beside it: the legend, the note of the keys, the
  // readout and the note of the series not drawn. Shown once there is a drawing to read,
  // and hidden again with the frame when there will be none.
  function beside(state, hidden) {
    state.box.hidden = hidden;
    state.beside.forEach(function (el) { el.hidden = hidden; });
  }

  // The frame takes the focus once it holds a drawing, not before: an empty frame that
  // announced keys doing nothing would be a control that is not one. No markup declares the
  // frame, so `data-chart-frame` is what tells a behaviour spec that it is a chart's and not a
  // scroll region's to drive (I2654).
  function focusable(state) {
    var box = state.box;
    box.tabIndex = 0;
    box.setAttribute('data-chart-frame', '');
    box.setAttribute('role', 'application');
    box.setAttribute('aria-roledescription', 'chart');
    box.setAttribute('aria-label', state.label);
    box.setAttribute('aria-describedby', state.help.id);
  }

  function draw(state) {
    return loadECharts().then(function (echarts) {
      lib = echarts;
      beside(state, false);
      // one instance for the chart's life; SVG keeps text and lines sharp at any pixel
      // density, page zoom and in print
      if (!state.chart) {
        state.chart = echarts.init(state.box, null, { renderer: 'svg' });
        wire(state);
        focusable(state);
      }
      state.chart.setOption(option(state), { notMerge: true });
      state.figure.setAttribute('data-chart-rendered', isDark() ? 'dark' : 'light');
      address();
    }).catch(function (error) {
      // the table is the chart's fallback and it never left the page
      beside(state, true);
      state.figure.setAttribute('data-chart-error', String(error && error.message || error));
      if (window.console) { window.console.error('[charts]', state.figure.id, error); }
    });
  }

  // The legend: a button per drawn series with its swatch, its name and its figure (the sum
  // of a stack's part, the latest recorded value of a line). Pressed is shown. A long line's
  // zoom is reset from the same row.
  function controls(state) {
    var holder = document.createElement('div');
    holder.className = 'mj-chart-keys';
    holder.setAttribute('role', 'group');
    holder.setAttribute('aria-label', state.drawn.length > 1 ? 'Series' : 'Chart controls');
    if (state.drawn.length > 1) {
      state.drawn.forEach(function (s) {
        var recorded = s.data.filter(function (v) { return v != null; });
        var sum = state.stacked ? recorded.reduce(function (a, b) { return a + b; }, 0) : recorded[recorded.length - 1];
        var key = document.createElement('button');
        key.type = 'button';
        key.className = 'mj-chart-key';
        key.setAttribute('aria-pressed', 'true');
        key.setAttribute('data-series', s.name);
        key.innerHTML = swatch(s) + '<span class="mj-chart-key-name"></span><span class="mj-chart-key-count"></span>';
        key.querySelector('.mj-chart-key-name').textContent = s.name;
        key.querySelector('.mj-chart-key-count').textContent = sum == null ? '' : state.unit.length <= 3 ? amount(sum, state.unit) : numeral(sum);
        key.addEventListener('click', function () {
          if (!state.chart) { return; }
          // the preview lets go first, or the series left behind would stay stepped back
          state.chart.dispatchAction({ type: 'downplay' });
          state.chart.dispatchAction({ type: 'legendToggleSelect', name: s.name });
        });
        // pointing at a key, or focusing it, steps every other series back; leaving always
        // restores, whatever the key was switched to in between
        var preview = function (on) {
          return function () {
            if (!state.chart) { return; }
            if (!on) { state.chart.dispatchAction({ type: 'downplay' }); return; }
            if (!state.hidden[s.name]) { state.chart.dispatchAction({ type: 'highlight', seriesName: s.name }); }
          };
        };
        key.addEventListener('pointerenter', preview(true));
        key.addEventListener('pointerleave', preview(false));
        key.addEventListener('focus', preview(true));
        key.addEventListener('blur', preview(false));
        holder.appendChild(key);
        state.keys.push(key);
      });
    }
    if (state.zoomable) {
      var reset = document.createElement('button');
      reset.type = 'button';
      reset.className = 'mj-button mj-chart-reset';
      reset.textContent = 'Reset zoom';
      reset.hidden = true;
      reset.addEventListener('click', function () { if (state.chart) { zoom(state, 0); } });
      holder.appendChild(reset);
      state.reset = reset;
    }
    return holder;
  }

  figures.forEach(function (figure, n) {
    var data = read(figure);
    if (!data) { return; }
    var id = figure.id || 'mj-chart-' + n;
    var label = (figure.getAttribute('data-chart-label') || 'Chart') + '. The same figures are in the table that follows.';
    var state = {
      figure: figure, data: data, box: null, help: null, readout: null, beside: [], chart: null, requested: false, label: label,
      unit: figure.getAttribute('data-chart-unit') || '', stacked: figure.hasAttribute('data-chart-stack'),
      drawn: data.series.filter(function (s) { return s.slot <= SERIES_DRAWN; }),
      hidden: {}, keys: [], reset: null, controls: null, cursor: -1, hover: -1, zoom: [0, 100], narrow: false
    };
    state.zoomable = figure.getAttribute('data-chart') === 'line' && data.categories.length > LONG;
    // the frame keeps its place while ECharts loads, empty and out of the tab order
    var box = document.createElement('div');
    box.className = 'mj-chart';
    if (horizontal(state)) {
      // one readable row per category, however many there are
      box.style.height = Math.max(320, data.categories.length * 26 + 72) + 'px';
    }
    box.addEventListener('keydown', function (event) { keys(state, event); });
    // the cursor, its band and its card go with the focus
    box.addEventListener('blur', function () { if (state.chart && state.cursor >= 0) { release(state); } });
    if (state.zoomable) {
      // A plain wheel over the frame scrolls the page, as it does over a graph: the event is
      // stopped on its way down to ECharts and its default, the scroll, is left alone. A pinch
      // arrives as a wheel with ctrlKey, so it still zooms, and so do Ctrl and Cmd with the wheel.
      box.addEventListener('wheel', function (event) {
        if (!event.ctrlKey && !event.metaKey) { event.stopPropagation(); }
      }, { capture: true, passive: true });
    }
    // what the keys do, which the frame is described by, and apart from it the polite readout
    // of what the cursor is on: a description the cursor overwrote would never be read again
    var help = document.createElement('p');
    help.className = 'mj-chart-help';
    help.id = id + '-keys';
    help.textContent = (horizontal(state) ? 'Up and down' : 'Left and right') + ' arrows step through the ' +
      data.categories.length + ' rows, Home and End go to the ends, Enter opens a linked row, Escape lets go' +
      (state.zoomable ? '; plus, minus and 0 zoom' : '') + '.';
    var readout = document.createElement('p');
    readout.className = 'mj-chart-readout';
    readout.setAttribute('aria-live', 'polite');
    state.box = box;
    state.help = help;
    state.readout = readout;
    var before = figure.querySelector('figcaption');
    var anchor = before && before === figure.firstElementChild ? before.nextSibling : figure.firstChild;
    if (state.drawn.length > 1 || state.zoomable) {
      state.controls = controls(state);
      figure.insertBefore(state.controls, anchor);
      state.beside.push(state.controls);
    }
    figure.insertBefore(box, anchor);
    figure.insertBefore(help, anchor);
    figure.insertBefore(readout, anchor);
    state.beside.push(help, readout);
    if (state.drawn.length < data.series.length) {
      var note = document.createElement('p');
      note.className = 'mj-chart-note';
      note.textContent = (data.series.length - state.drawn.length) + ' more series are in the table and not drawn: ' +
        'six colours are as many as the chart keeps apart.';
      figure.insertBefore(note, anchor);
      state.beside.push(note);
    }
    // nothing beside the frame shows before there is a drawing to read
    state.beside.forEach(function (el) { el.hidden = true; });
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
      if (row >= 0) { point(state, row); }
    });
  }
  window.addEventListener('hashchange', address);
  address();

  if ('IntersectionObserver' in window) {
    var seen = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (!entry.isIntersecting) { return; }
        seen.unobserve(entry.target);
        for (var i = 0; i < states.length; i++) { if (states[i].figure === entry.target) { request(states[i]); } }
      });
    }, { rootMargin: '300px 0px' });
    states.forEach(function (state) { seen.observe(state.figure); });
  } else {
    states.forEach(request);
  }

  // Restyle in the other palette when the theme changes, with patterns when more contrast is
  // asked for, and to the new width on resize: in place, never replaying the entrance.
  new MutationObserver(function () {
    var theme = isDark() ? 'dark' : 'light';
    states.forEach(function (state) {
      if (state.chart && state.figure.getAttribute('data-chart-rendered') !== theme) { restyle(state); }
    });
  }).observe(root, { attributes: true, attributeFilter: ['class'] });
  if (window.matchMedia) {
    var contrast = window.matchMedia(CONTRAST);
    var again = function () { states.forEach(restyle); };
    if (contrast.addEventListener) { contrast.addEventListener('change', again); } else if (contrast.addListener) { contrast.addListener(again); }
  }

  if ('ResizeObserver' in window) {
    var resized = new ResizeObserver(function (entries) {
      entries.forEach(function (entry) {
        for (var i = 0; i < states.length; i++) {
          var state = states[i];
          if (state.box !== entry.target || !state.chart) { continue; }
          state.chart.resize({ animation: { duration: 0 } });
          // across 480px the end labels come and go and the category labels change width
          if ((state.box.clientWidth < 480) !== state.narrow) { restyle(state); }
        }
      });
    });
    states.forEach(function (state) { resized.observe(state.box); });
  }

  // What a test or a probe may ask: the charts by id and what each one read from its table.
  window.mjCharts = {
    ids: function () { return states.map(function (s) { return s.figure.id; }); },
    state: function (id) {
      for (var i = 0; i < states.length; i++) {
        var s = states[i];
        if (s.figure.id === id) {
          return { id: id, rendered: Boolean(s.chart), categories: s.data.categories.slice(),
                   series: s.data.series.map(function (x) { return { name: x.name, data: x.data.slice() }; }),
                   links: s.data.links.slice(),
                   drawn: s.drawn.map(function (x) { return x.name; }),
                   hidden: Object.keys(s.hidden).filter(function (k) { return s.hidden[k]; }),
                   cursor: s.cursor };
        }
      }
      return null;
    }
  };
})();

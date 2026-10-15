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
// Every colour is a token of the page, read back from the page.
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

  function isDark() { return root.classList.contains(root.dataset.themeClass); }

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
  function token(name) {
    if (!getComputedStyle(root).getPropertyValue(name).trim()) { return ''; }
    probe.style.color = 'var(' + name + ')';
    document.body.appendChild(probe);
    var resolved = getComputedStyle(probe).color;
    probe.remove();
    return hex(resolved);
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

  // The table as the chart reads it: categories, the marked series and each row's link.
  function read(figure) {
    var table = figure.querySelector('table');
    if (!table || !table.tHead || !table.tBodies.length) { return null; }
    var heads = Array.prototype.slice.call(table.tHead.rows[table.tHead.rows.length - 1].cells);
    var series = [];
    heads.forEach(function (th, column) {
      if (!th.hasAttribute('data-series')) { return; }
      var marked = th.querySelector('.mj-badge, .mj-status');
      series.push({
        column: column,
        name: th.getAttribute('data-series') || th.textContent.trim(),
        colour: marked ? hex(getComputedStyle(marked).color) : '',
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

  function option(state) {
    var figure = state.figure, data = state.data;
    var kind = figure.getAttribute('data-chart') === 'line' ? 'line' : 'bar';
    var horizontal = figure.hasAttribute('data-chart-horizontal') && kind === 'bar';
    var stacked = figure.hasAttribute('data-chart-stack');
    var unit = figure.getAttribute('data-chart-unit') || '';
    var fg = token('--mj-fg'), muted = token('--mj-muted'), line = token('--mj-line'), raised = token('--mj-raised');
    var palette = SERIES_TOKENS.map(token).filter(Boolean);
    var text = { color: muted, fontFamily: getComputedStyle(document.body).fontFamily, fontSize: 12 };
    var narrow = state.box.clientWidth < 480;
    var category = {
      type: 'category', data: data.categories, inverse: horizontal,
      axisLine: { lineStyle: { color: line } }, axisTick: { lineStyle: { color: line } },
      axisLabel: Object.assign({ hideOverlap: true, width: horizontal ? (narrow ? 96 : 200) : undefined, overflow: 'truncate' }, text)
    };
    var value = {
      type: 'value', scale: kind === 'line',
      name: unit, nameLocation: 'middle', nameGap: horizontal ? 28 : 44, nameTextStyle: text,
      axisLabel: text, splitLine: { lineStyle: { color: line } }
    };
    var next = 0;
    return {
      animation: !(window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches),
      backgroundColor: 'transparent',
      textStyle: text,
      color: palette,
      aria: { enabled: true },
      grid: { left: 8, right: 16, top: data.series.length > 1 ? 40 : 16, bottom: 8, containLabel: true },
      legend: data.series.length > 1 ? { type: 'scroll', top: 4, left: 8, right: 8, textStyle: text, pageTextStyle: text, inactiveColor: line } : undefined,
      tooltip: {
        trigger: 'axis', axisPointer: { type: kind === 'line' ? 'line' : 'shadow' }, confine: true,
        backgroundColor: raised, borderColor: line, textStyle: { color: fg, fontSize: 12 },
        valueFormatter: function (v) { return v == null ? 'not recorded' : v + (unit ? ' ' + unit : ''); }
      },
      xAxis: horizontal ? value : category,
      yAxis: horizontal ? category : value,
      series: data.series.map(function (s) {
        var colour = s.colour || palette[next++ % Math.max(palette.length, 1)];
        return {
          name: s.name, type: kind, data: s.data, stack: stacked ? 'whole' : undefined,
          itemStyle: colour ? { color: colour } : undefined, lineStyle: colour ? { color: colour } : undefined,
          symbolSize: 8, barMaxWidth: 28, emphasis: { focus: 'series' }
        };
      })
    };
  }

  function draw(state) {
    return loadECharts().then(function (echarts) {
      if (state.chart) { state.chart.dispose(); }
      state.box.hidden = false;
      state.chart = echarts.init(state.box);
      state.chart.setOption(option(state));
      state.figure.setAttribute('data-chart-rendered', isDark() ? 'dark' : 'light');
      // a mark whose row is a link goes where the row's link goes
      state.chart.on('click', function (params) {
        var href = state.data.links[params.dataIndex];
        if (href) { window.location.assign(href); }
      });
      state.chart.getZr().on('mousemove', function (event) {
        var at = state.chart.convertFromPixel({ gridIndex: 0 }, [event.offsetX, event.offsetY]);
        var i = at ? Math.round(state.figure.hasAttribute('data-chart-horizontal') ? at[1] : at[0]) : -1;
        state.chart.getZr().setCursorStyle(state.data.links[i] ? 'pointer' : 'default');
      });
      address();
    }).catch(function (error) {
      // the table is the chart's fallback and it never left the page
      state.box.hidden = true;
      state.figure.setAttribute('data-chart-error', String(error && error.message || error));
      if (window.console) { window.console.error('[charts]', state.figure.id, error); }
    });
  }

  figures.forEach(function (figure) {
    var data = read(figure);
    if (!data) { return; }
    var box = document.createElement('div');
    box.className = 'mj-chart';
    box.setAttribute('role', 'img');
    box.setAttribute('aria-label', (figure.getAttribute('data-chart-label') || 'Chart') + '. The same figures are in the table that follows.');
    if (figure.hasAttribute('data-chart-horizontal')) {
      // one readable row per category, however many there are
      box.style.height = Math.max(320, data.categories.length * 26 + 72) + 'px';
    }
    var before = figure.querySelector('figcaption');
    figure.insertBefore(box, before && before === figure.firstElementChild ? before.nextSibling : figure.firstChild);
    states.push({ figure: figure, data: data, box: box, chart: null, requested: false });
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
        state.chart.dispatchAction({ type: 'showTip', seriesIndex: 0, dataIndex: row });
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
        for (var i = 0; i < states.length; i++) { if (states[i].figure === entry.target) { request(states[i]); } }
      });
    }, { rootMargin: '300px 0px' });
    states.forEach(function (state) { seen.observe(state.figure); });
  } else {
    states.forEach(request);
  }

  // Draw again in the other palette when the theme changes, and to the new width on resize.
  new MutationObserver(function () {
    var theme = isDark() ? 'dark' : 'light';
    states.forEach(function (state) {
      if (state.chart && state.figure.getAttribute('data-chart-rendered') !== theme) { draw(state); }
    });
  }).observe(root, { attributes: true, attributeFilter: ['class'] });

  if ('ResizeObserver' in window) {
    var resized = new ResizeObserver(function (entries) {
      entries.forEach(function (entry) {
        for (var i = 0; i < states.length; i++) {
          if (states[i].box === entry.target && states[i].chart) { states[i].chart.resize(); }
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
                   links: s.data.links.slice() };
        }
      }
      return null;
    }
  };
})();

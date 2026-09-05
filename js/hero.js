// The hero animation: work arriving from several sessions, meeting the finish contract, and
// being accepted or refused. It is the product's one sentence, drawn.
//
// No library. The contract's five checks and the outcome vocabulary are read from the page's
// generated data, so the picture cannot describe a program this repository does not have.
//
// Progressive enhancement: the figure already contains the contract as a list, which is what a
// reader without JavaScript keeps. The canvas is added over it and hidden from assistive
// technology. `prefers-reduced-motion` draws one still frame instead of running the loop.
(function () {
  var host = document.querySelector('[data-hero]');
  if (!host || !window.requestAnimationFrame) { return; }
  var canvas = host.querySelector('canvas');
  if (!canvas || !canvas.getContext) { return; }

  var checks, outcomes;
  try {
    checks = JSON.parse(host.getAttribute('data-hero-checks'));
    outcomes = JSON.parse(host.getAttribute('data-hero-outcomes'));
  } catch (e) { return; }
  if (!checks || !checks.length) { return; }

  var reduced = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  var ctx = canvas.getContext('2d');
  var W = 0, H = 0, dpr = 1;

  // A fixed seed: the animation is the same on every load and in every screenshot, which is
  // the same property the rest of the build has.
  var seed = 20260904;
  function rnd() { seed = (seed * 1103515245 + 12345) & 0x7fffffff; return seed / 0x7fffffff; }

  function css(name, fallback) {
    var v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    return v || fallback;
  }
  var theme;
  function readTheme() {
    var dark = document.documentElement.classList.contains('dark');
    theme = {
      dark: dark,
      line: dark ? 'rgba(148,163,184,0.35)' : 'rgba(100,116,139,0.28)',
      gate: dark ? '#818cf8' : '#4f46e5',
      pass: dark ? '#34d399' : '#059669',
      fail: dark ? '#fbbf24' : '#d97706',
      item: dark ? 'rgba(226,232,240,0.85)' : 'rgba(51,65,85,0.8)',
      label: dark ? 'rgba(203,213,225,0.75)' : 'rgba(71,85,105,0.75)'
    };
  }
  readTheme();

  var LANES = 4;
  var items = [];
  var accepted = 0, refused = 0;
  var refusedOutcome = outcomes.indexOf('blocked') !== -1 ? 'blocked' : outcomes[outcomes.length - 1];

  function spawn(t) {
    var lane = Math.floor(rnd() * LANES);
    // roughly one in four is refused; the point of the picture is that refusal is normal
    var ok = rnd() > 0.28;
    items.push({
      lane: lane, x: 0, t0: t, ok: ok,
      check: checks[Math.floor(rnd() * checks.length)],
      state: 'travel', settle: 0
    });
  }

  function resize() {
    var rect = host.getBoundingClientRect();
    dpr = Math.min(window.devicePixelRatio || 1, 2);
    W = Math.max(240, rect.width);
    H = Math.max(200, rect.height);
    canvas.width = Math.round(W * dpr);
    canvas.height = Math.round(H * dpr);
    canvas.style.width = W + 'px';
    canvas.style.height = H + 'px';
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }

  function laneY(i) { return H * (0.22 + i * 0.185); }
  function gateX() { return W * 0.58; }

  function draw(t) {
    ctx.clearRect(0, 0, W, H);
    var gx = gateX();

    ctx.font = '500 11px ui-monospace, SFMono-Regular, Menlo, monospace';
    ctx.textBaseline = 'middle';

    // lanes: one per session, unlabelled on purpose — how many there are is the reader's problem
    for (var i = 0; i < LANES; i++) {
      var y = laneY(i);
      ctx.strokeStyle = theme.line;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.setLineDash([2, 6]);
      ctx.moveTo(W * 0.06, y);
      ctx.lineTo(gx - 10, y);
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.fillStyle = theme.item;
      ctx.globalAlpha = 0.55;
      ctx.beginPath(); ctx.arc(W * 0.05, y, 2.5, 0, Math.PI * 2); ctx.fill();
      ctx.globalAlpha = 1;
    }
    ctx.fillStyle = theme.label;
    ctx.textAlign = 'left';
    ctx.fillText('sessions', W * 0.045, laneY(0) - 26);

    // the gate: one rung per contract check
    var top = laneY(0) - 14, bot = laneY(LANES - 1) + 14;
    ctx.strokeStyle = theme.gate;
    ctx.lineWidth = 2;
    ctx.beginPath(); ctx.moveTo(gx, top); ctx.lineTo(gx, bot); ctx.stroke();
    ctx.fillStyle = theme.gate;
    ctx.textAlign = 'center';
    ctx.fillText('finish', gx, top - 14);
    ctx.font = '400 9px ui-monospace, SFMono-Regular, Menlo, monospace';
    for (var c = 0; c < checks.length; c++) {
      var cy = top + ((bot - top) * (c + 0.5)) / checks.length;
      ctx.strokeStyle = theme.gate;
      ctx.globalAlpha = 0.55;
      ctx.beginPath(); ctx.moveTo(gx - 5, cy); ctx.lineTo(gx + 5, cy); ctx.stroke();
      ctx.globalAlpha = 0.8;
      ctx.fillStyle = theme.gate;
      ctx.fillText(String(c + 1), gx + 11, cy);
      ctx.globalAlpha = 1;
    }
    ctx.textAlign = 'left';

    // items
    for (var k = items.length - 1; k >= 0; k--) {
      var it = items[k];
      var y = laneY(it.lane);
      var colour = it.state === 'travel' ? theme.item : (it.ok ? theme.pass : theme.fail);
      ctx.fillStyle = colour;
      ctx.globalAlpha = it.state === 'gone' ? Math.max(0, 1 - it.settle) : 1;
      var px = W * 0.05 + it.x * (gx - W * 0.07);
      var py = y;
      if (it.state === 'passed') { px = gx + 6 + it.settle * (W * 0.28); py = y - it.settle * 8; }
      if (it.state === 'refused') { px = gx - 10 - it.settle * (W * 0.16); py = y + it.settle * 10; }
      ctx.fillRect(px - 4.5, py - 4.5, 9, 9);
      if (it.state === 'refused') {
        ctx.globalAlpha = Math.max(0, 1 - it.settle);
        ctx.fillStyle = theme.fail;
        ctx.font = '500 10px ui-monospace, SFMono-Regular, Menlo, monospace';
        ctx.textAlign = 'right';
        ctx.fillText('FAIL ' + it.check, px - 8, py);
        ctx.font = '400 9px ui-monospace, SFMono-Regular, Menlo, monospace';
        ctx.textAlign = 'left';
      }
      ctx.globalAlpha = 1;
    }

    // tally
    ctx.font = '500 10px ui-monospace, SFMono-Regular, Menlo, monospace';
    ctx.textAlign = 'right';
    ctx.fillStyle = theme.pass;
    ctx.fillText('completed  ' + accepted, W - 12, 16);
    ctx.fillStyle = theme.fail;
    ctx.fillText(refusedOutcome + '  ' + refused, W - 12, 31);
    ctx.textAlign = 'left';
  }

  function step(dt, t) {
    for (var k = items.length - 1; k >= 0; k--) {
      var it = items[k];
      if (it.state === 'travel') {
        it.x += dt * 0.00022;
        if (it.x >= 1) {
          it.state = it.ok ? 'passed' : 'refused';
          if (it.ok) { accepted++; } else { refused++; }
        }
      } else if (it.state !== 'gone') {
        it.settle += dt * 0.0011;
        if (it.settle >= 1) { it.state = 'gone'; it.settle = 0; }
      } else {
        it.settle += dt * 0.002;
        if (it.settle >= 1) { items.splice(k, 1); }
      }
    }
    if (items.length < 9 && rnd() < dt * 0.004) { spawn(t); }
  }

  // the pipeline starts populated: an empty canvas that fills over ten seconds says nothing
  function preseed() {
    for (var i = 0; i < 7; i++) {
      var it = { lane: i % LANES, x: rnd() * 0.9, t0: 0, ok: rnd() > 0.28, check: checks[Math.floor(rnd() * checks.length)], state: 'travel', settle: 0 };
      items.push(it);
    }
  }

  resize();
  window.addEventListener('resize', function () { resize(); if (reduced) { draw(0); } });
  var toggle = document.getElementById('theme-toggle');
  if (toggle) { toggle.addEventListener('click', function () { setTimeout(function () { readTheme(); draw(0); }, 0); }); }

  if (reduced) {
    // one still frame: work in every lane, one item refused at the gate, the tally already run
    for (var i = 0; i < LANES; i++) { items.push({ lane: i, x: 0.25 + i * 0.16, ok: true, state: 'travel', check: checks[i % checks.length], settle: 0 }); }
    items.push({ lane: 2, x: 1, ok: false, state: 'refused', check: 'blockers', settle: 0.25 });
    accepted = 3; refused = 1;
    draw(0);
    return;
  }

  preseed();
  var last = 0, running = true;
  document.addEventListener('visibilitychange', function () { running = !document.hidden; if (running) { last = 0; requestAnimationFrame(frame); } });
  function frame(t) {
    if (!running) { return; }
    var dt = last ? Math.min(t - last, 64) : 16;
    last = t;
    step(dt, t);
    draw(t);
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
})();

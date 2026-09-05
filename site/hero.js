// The hero animation: one task, the finish contract, and the verdict. It is the product's
// one sentence, drawn slowly enough to be read by somebody who is not going to read the page.
//
// Written for a reader at altitude, and that decides the shape: one task on screen at a time,
// one idea per beat, every check named in full, and the refusal held long enough to land —
// because the refusal is the product. Any frozen frame is a legible picture, which is what a
// screenshot in a deck needs.
//
// No library. The contract's checks and the outcome vocabulary are read from the page's
// generated data, so the picture cannot describe a program this repository does not have.
//
// Progressive enhancement: the figure already contains the contract as a list, which is what
// a reader without JavaScript keeps. The canvas is added over it and hidden from assistive
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
  if (!checks || !checks.length || !outcomes || !outcomes.length) { return; }

  var reduced = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  var ctx = canvas.getContext('2d');
  var W = 0, H = 0, dpr = 1, scale = 1;

  // Both verdict words come from the outcome vocabulary, never from a word written here.
  var acceptedName = outcomes.indexOf('completed') !== -1 ? 'completed' : outcomes[0];
  var refusedName = outcomes.indexOf('blocked') !== -1 ? 'blocked' : outcomes[outcomes.length - 1];

  // The order of verdicts is fixed rather than random: the same picture on every load and in
  // every screenshot, and a refusal inside the first two cycles, because a viewer who watches
  // for fifteen seconds has to see the thing the tool is for.
  var VERDICTS = [true, false, true, false, true];
  var cycle = 0, accepted = 0, refused = 0;

  var MONO = 'ui-monospace, SFMono-Regular, Menlo, monospace';
  var SANS = 'ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif';

  var theme;
  function readTheme() {
    var dark = document.documentElement.classList.contains('dark');
    theme = {
      rule: dark ? 'rgba(148,163,184,0.22)' : 'rgba(100,116,139,0.20)',
      faint: dark ? 'rgba(148,163,184,0.45)' : 'rgba(100,116,139,0.45)',
      ink: dark ? 'rgba(226,232,240,0.92)' : 'rgba(30,41,59,0.92)',
      quiet: dark ? 'rgba(203,213,225,0.62)' : 'rgba(71,85,105,0.62)',
      pass: dark ? '#34d399' : '#059669',
      fail: dark ? '#fbbf24' : '#b45309',
      card: dark ? 'rgba(148,163,184,0.16)' : 'rgba(100,116,139,0.10)'
    };
  }
  readTheme();

  // The canvas fills its parent, so the parent is what is measured. Measuring the whole
  // figure drew the picture over the check list on a phone.
  function resize() {
    var box = canvas.parentNode && canvas.parentNode.getBoundingClientRect
      ? canvas.parentNode.getBoundingClientRect() : { width: 320, height: 256 };
    dpr = Math.min(window.devicePixelRatio || 1, 2);
    W = Math.max(240, box.width || 320);
    H = Math.max(180, box.height || 256);
    scale = Math.max(0.82, Math.min(1.3, H / 272));
    canvas.width = Math.round(W * dpr);
    canvas.height = Math.round(H * dpr);
    canvas.style.width = W + 'px';
    canvas.style.height = H + 'px';
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }

  // ---------------------------------------------------------------- the beats
  // One cycle, in milliseconds. Every number here is a reading speed, not a frame rate: a
  // check lands, the eye moves, the next check lands; the verdict holds for three seconds
  // because it is the only thing a viewer must leave with.
  var ARRIVE = 700;
  var FIRST = 1000;
  var PER_CHECK = 620;
  var VERDICT_IN = 420;
  var VERDICT_HOLD = 3000;
  var LEAVE = 700;
  function cycleLength() { return FIRST + PER_CHECK * checks.length + VERDICT_IN + VERDICT_HOLD + LEAVE; }

  function ease(x) { return x < 0 ? 0 : x > 1 ? 1 : 1 - Math.pow(1 - x, 3); }

  // Which check refuses on a refusing cycle: a different one each time, so the picture says
  // "any line can refuse", not "line four is the flaky one".
  function failingIndex(c) { return checks.length > 1 ? (Math.floor(c / 2) + 1) % checks.length : 0; }

  function frameState(ms) {
    var ok = VERDICTS[cycle % VERDICTS.length];
    var kf = ok ? -1 : failingIndex(cycle);
    var s = { ok: ok, failing: kf, arrive: ease(ms / ARRIVE), evaluated: [], verdict: 0, leave: 0 };
    for (var i = 0; i < checks.length; i++) {
      var at = FIRST + PER_CHECK * i;
      s.evaluated.push(ms < at ? 0 : (i === kf ? -1 : 1));
    }
    var vAt = FIRST + PER_CHECK * checks.length + VERDICT_IN;
    if (ms >= vAt) { s.verdict = ease((ms - vAt) / 260); }
    var lAt = vAt + VERDICT_HOLD;
    if (ms >= lAt) { s.leave = ease((ms - lAt) / LEAVE); }
    return s;
  }

  // ---------------------------------------------------------------- drawing
  function roundRect(x, y, w, h, r) {
    ctx.beginPath();
    ctx.moveTo(x + r, y);
    ctx.lineTo(x + w - r, y); ctx.quadraticCurveTo(x + w, y, x + w, y + r);
    ctx.lineTo(x + w, y + h - r); ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
    ctx.lineTo(x + r, y + h); ctx.quadraticCurveTo(x, y + h, x, y + h - r);
    ctx.lineTo(x, y + r); ctx.quadraticCurveTo(x, y, x + r, y);
    ctx.closePath();
  }
  function tick(x, y, r, colour) {
    ctx.strokeStyle = colour; ctx.lineWidth = Math.max(1.6, 2 * scale);
    ctx.lineCap = 'round'; ctx.lineJoin = 'round';
    ctx.beginPath();
    ctx.moveTo(x - r * 0.55, y); ctx.lineTo(x - r * 0.1, y + r * 0.5); ctx.lineTo(x + r * 0.6, y - r * 0.5);
    ctx.stroke();
  }
  function cross(x, y, r, colour) {
    ctx.strokeStyle = colour; ctx.lineWidth = Math.max(1.6, 2 * scale);
    ctx.lineCap = 'round';
    ctx.beginPath();
    ctx.moveTo(x - r * 0.45, y - r * 0.45); ctx.lineTo(x + r * 0.45, y + r * 0.45);
    ctx.moveTo(x + r * 0.45, y - r * 0.45); ctx.lineTo(x - r * 0.45, y + r * 0.45);
    ctx.stroke();
  }

  function draw(ms) {
    var s = frameState(ms);
    ctx.clearRect(0, 0, W, H);
    ctx.textBaseline = 'middle';

    var padX = Math.round(14 * scale);
    var fCheck = Math.round(13 * scale);
    var fSmall = Math.round(10.5 * scale);
    var fVerdict = Math.round(19 * scale);

    var rowsTop = Math.round(H * 0.30);
    var rowsBottom = Math.round(H * 0.70);
    var rowH = (rowsBottom - rowsTop) / checks.length;
    var gutter = Math.round(Math.min(96, Math.max(64, W * 0.24)));
    var railX = padX + gutter;

    // the top line: the tally always, and what this is when the two fit side by side. On a
    // narrow canvas the count is the half that carries the story, so the kicker is the half
    // that goes; neither is ever drawn over the other.
    var topY = Math.round(H * 0.11);
    ctx.font = '500 ' + fSmall + 'px ' + MONO;
    var kicker = 'one task \u00b7 ' + checks.length + ' checks';
    var passText = acceptedName + ' ' + accepted;
    var failText = refusedName + ' ' + refused;
    var tallyW = ctx.measureText(passText).width + ctx.measureText(failText).width + Math.round(12 * scale);
    ctx.textAlign = 'right';
    ctx.fillStyle = theme.fail;
    ctx.fillText(failText, W - padX, topY);
    ctx.fillStyle = theme.pass;
    ctx.fillText(passText, W - padX - ctx.measureText(failText).width - Math.round(12 * scale), topY);
    if (padX + ctx.measureText(kicker).width + Math.round(16 * scale) + tallyW < W - padX) {
      ctx.textAlign = 'left';
      ctx.fillStyle = theme.quiet;
      ctx.fillText(kicker, padX, topY);
    }

    // the gate: one rule the task has to cross, with the checks hanging off it
    ctx.strokeStyle = theme.rule;
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(railX - Math.round(12 * scale), rowsTop - Math.round(10 * scale));
    ctx.lineTo(railX - Math.round(12 * scale), rowsBottom + Math.round(10 * scale));
    ctx.stroke();

    // the task: one card, arriving, waiting, then leaving by the side its verdict sends it
    var cardW = Math.round(Math.min(88, gutter - 16));
    var cardH = Math.round(34 * scale);
    var cardY = Math.round((rowsTop + rowsBottom) / 2 - cardH / 2);
    var cardX = padX - (1 - s.arrive) * (cardW + padX + 12);
    var alpha = 1;
    if (s.leave > 0) {
      cardX = padX + (s.ok ? 1 : -1) * s.leave * (W * 0.5);
      alpha = 1 - s.leave;
    }
    ctx.globalAlpha = Math.max(0, alpha);
    ctx.fillStyle = theme.card;
    roundRect(cardX, cardY, cardW, cardH, Math.round(5 * scale)); ctx.fill();
    ctx.strokeStyle = s.verdict > 0 ? (s.ok ? theme.pass : theme.fail) : theme.faint;
    ctx.lineWidth = s.verdict > 0 ? 1.8 : 1;
    roundRect(cardX, cardY, cardW, cardH, Math.round(5 * scale)); ctx.stroke();
    ctx.font = '500 ' + fSmall + 'px ' + MONO;
    ctx.fillStyle = theme.quiet;
    ctx.textAlign = 'center';
    ctx.fillText('task', cardX + cardW / 2, cardY + cardH / 2);
    ctx.globalAlpha = 1;

    // the checks, named in full, one at a time
    ctx.textAlign = 'left';
    for (var i = 0; i < checks.length; i++) {
      var y = rowsTop + rowH * (i + 0.5);
      var state = s.evaluated[i];
      var glyphX = railX + Math.round(6 * scale);
      var r = Math.round(5.5 * scale);

      ctx.strokeStyle = theme.rule;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(railX - Math.round(12 * scale), y);
      ctx.lineTo(glyphX - Math.round(4 * scale), y);
      ctx.stroke();

      if (state === 0) {
        ctx.strokeStyle = theme.faint; ctx.lineWidth = 1;
        ctx.beginPath(); ctx.arc(glyphX + r * 0.1, y, r * 0.8, 0, Math.PI * 2); ctx.stroke();
        ctx.fillStyle = theme.quiet;
      } else if (state === 1) {
        tick(glyphX + r * 0.1, y, r, theme.pass);
        ctx.fillStyle = theme.ink;
      } else {
        cross(glyphX + r * 0.1, y, r, theme.fail);
        ctx.fillStyle = theme.fail;
      }
      ctx.font = (state === -1 ? '600 ' : '400 ') + fCheck + 'px ' + MONO;
      ctx.fillText(checks[i], glyphX + Math.round(16 * scale), y + 0.5);
    }

    // the verdict: the one line a reader leaves with
    if (s.verdict > 0) {
      var vy = Math.round(H * 0.855);
      ctx.globalAlpha = Math.max(0, Math.min(1, s.verdict) * (s.leave > 0 ? 1 - s.leave : 1));
      ctx.textAlign = 'left';
      ctx.font = '700 ' + fVerdict + 'px ' + SANS;
      ctx.fillStyle = s.ok ? theme.pass : theme.fail;
      var word = s.ok ? acceptedName : refusedName;
      ctx.fillText(word, padX, vy);
      var wordW = ctx.measureText(word).width;
      ctx.font = '400 ' + fSmall + 'px ' + MONO;
      ctx.fillStyle = theme.quiet;
      // the tail beside the verdict where there is room, under it where there is not, and
      // shorter before it is ever cut: the failing check's name is the part that must survive
      var tails = s.ok
        ? ['every line passed']
        : ['failed: ' + checks[s.failing] + ' \u00b7 the task stays open', 'failed: ' + checks[s.failing], checks[s.failing]];
      var beside = padX + wordW + Math.round(10 * scale);
      var tail = tails[tails.length - 1], x = padX, y = vy + Math.round(15 * scale);
      for (var ti = 0; ti < tails.length; ti++) {
        if (beside + ctx.measureText(tails[ti]).width < W - padX) { tail = tails[ti]; x = beside; y = vy + 1; break; }
        if (padX + ctx.measureText(tails[ti]).width < W - padX) { tail = tails[ti]; x = padX; y = vy + Math.round(15 * scale); break; }
      }
      ctx.fillText(tail, x, y);
      ctx.globalAlpha = 1;
    }
  }

  // ---------------------------------------------------------------- the loop
  var still = false, stillMs = 0, lastMs = 0;
  resize();
  window.addEventListener('resize', function () { resize(); draw(still ? stillMs : lastMs); });
  var toggle = document.getElementById('theme-toggle');
  if (toggle) {
    toggle.addEventListener('click', function () {
      setTimeout(function () { readTheme(); draw(still ? stillMs : lastMs); }, 0);
    });
  }

  // The still frame is the refusal, mid-contract: the checks evaluated, one of them failing,
  // the verdict named. One picture, the whole sentence.
  if (reduced) {
    still = true;
    cycle = 1;
    accepted = 2; refused = 1;
    stillMs = FIRST + PER_CHECK * checks.length + VERDICT_IN + 200;
    draw(stillMs);
    return;
  }

  // `null`, not 0: the first animation frame's timestamp is 0 in a headless browser, and a
  // falsy check reset the clock on every frame, which froze the picture on its first beat.
  var start = null, running = true;
  document.addEventListener('visibilitychange', function () {
    running = !document.hidden;
    if (running) { start = null; requestAnimationFrame(frame); }
  });
  function frame(t) {
    if (!running) { return; }
    if (start === null) { start = t - lastMs; }
    var ms = t - start;
    if (ms >= cycleLength()) {
      // the count is the run's, incremented once, as the cycle it belongs to ends
      if (VERDICTS[cycle % VERDICTS.length]) { accepted++; } else { refused++; }
      cycle++; start = t; ms = 0; lastMs = 0;
    }
    lastMs = ms;
    draw(ms);
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
})();

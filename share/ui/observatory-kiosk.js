// The Observatory's scenes that read what the repository proves (ADR 0160): the proof
// lattice, the claim ledger, the surface atlas, the wave front and the release matrix.
//
// They are registered with the director in observatory.js, which owns the stage, the
// rotation, the status words and the page's projection. A scene here is a function of its
// model and nothing else: it asks the server for nothing and decides nothing about what the
// repository proves. Every cell, bar and arc is drawn from a number or a word a source
// answered; the breathing of the lattice and the sweep of an arc are decoration and the
// legend says so.
//
// They are drawn with p5, in instance mode, onto a surface of their own that the director
// then composes onto its stage, so that one clock, one fade and one set of status words
// serve every scene. p5 is the pinned vendored library `scripts/cockpit-assets` copies in;
// it is not committed. A checkout without it draws these scenes as unavailable, says so on
// the stage, and the same facts stay in the table beside the stage: nothing is drawn in its
// place.
(function () {
  'use strict';

  const registry = window.mjObservatory;
  if (!registry || !registry.kit) return;
  const { token, tone, withAlpha, unit, label, TAU } = registry.kit;

  // ---------------------------------------------------------------- words and colours
  // The tone of each state word the sources use here. A word not listed is drawn in the
  // muted tone and named, never in a tone that says something it was not told.
  const WORDS = {
    gated: 'green',
    reviewed: 'cyan',
    not_run: 'blue',
    stale: 'orange',
    unproven: 'rose',
    no_test: 'rose',
    ok: 'green',
    unknown: null,
    failed: 'rose',
    DONE: 'cyan',
    ACTIVE: 'violet',
    READY: 'blue',
    PLANNED: 'blue',
    BLOCKED: 'orange',
    VERIFY: 'orange',
  };
  const PLAIN = {
    not_run: 'not run',
    no_test: 'no test',
  };
  const plain = (w) => PLAIN[w] || String(w).replace(/_/g, ' ');
  function paint(word, alpha) {
    const t = WORDS[word];
    const hex = t ? tone(t) : token('--mj-muted', '#889');
    return alpha === undefined ? hex : withAlpha(hex, alpha);
  }
  const ORDER = ['gated', 'reviewed', 'not_run', 'stale', 'unproven', 'no_test'];
  const rank = (w) => {
    const i = ORDER.indexOf(w);
    return i < 0 ? ORDER.length : i;
  };

  // ---------------------------------------------------------------- the p5 surface
  // One p5 instance per scene, never drawing by itself: the director asks for a frame and
  // gets the surface to compose. The instance is made when its scene is first shown and
  // kept, so rotating back costs no setup.
  let loading = null;
  function p5Library() {
    if (window.p5) return Promise.resolve(window.p5);
    if (loading) return loading;
    const holder = document.querySelector('[data-p5-lib]');
    const src = holder && holder.dataset.p5Lib;
    if (!src) return Promise.reject(new Error('the page names no p5 library'));
    loading = new Promise((resolve, reject) => {
      const tag = document.createElement('script');
      tag.src = src;
      tag.async = true;
      tag.onload = () => (window.p5 ? resolve(window.p5) : reject(new Error('p5 did not register')));
      tag.onerror = () => reject(new Error('p5 is not built into this checkout'));
      document.head.append(tag);
    });
    return loading;
  }
  let p5State = 'loading'; // loading | ready | missing
  p5Library().then(
    () => { p5State = 'ready'; },
    () => { p5State = 'missing'; },
  );

  const surfaces = new Map();
  function surface(id, box, sketch) {
    let s = surfaces.get(id);
    if (!s) {
      const host = document.createElement('div');
      host.hidden = true;
      host.setAttribute('aria-hidden', 'true');
      document.body.append(host);
      // `snap` holds the last frame p5 finished: the pinned p5 draws asynchronously, so the
      // director composes from here and never from a surface that is half drawn.
      s = { frame: { box, model: null, t: 0, moving: true, fact: '' }, snap: document.createElement('canvas') };
      s.p5 = new window.p5((p) => {
        p.setup = () => {
          p.createCanvas(Math.max(1, box.w), Math.max(1, box.h));
          p.pixelDensity(Math.min(window.devicePixelRatio || 1, 2));
          p.noLoop();
        };
        p.draw = () => {
          p.clear();
          s.frame.fact = sketch(p, s.frame) || '';
          const done = p.canvas;
          if (s.snap.width !== done.width || s.snap.height !== done.height) {
            s.snap.width = done.width;
            s.snap.height = done.height;
          }
          const g = s.snap.getContext('2d');
          g.clearRect(0, 0, s.snap.width, s.snap.height);
          g.drawImage(done, 0, 0);
          s.ready = true;
        };
      }, host);
      surfaces.set(id, s);
    }
    return s;
  }

  // The director's painter for a p5 scene: set the frame, draw it, compose it.
  function scene(id, sketch) {
    return (c, box, model, t, moving) => {
      if (p5State !== 'ready') {
        const missing = p5State === 'missing';
        label(c, box, missing ? 'p5 IS NOT BUILT HERE' : 'LOADING THE RENDERER', box.cx, box.cy - box.u * 1.2, 1.3);
        label(
          c,
          box,
          missing
            ? 'scripts/cockpit-assets copies it in; the table below holds the same facts'
            : 'the table below holds the same facts',
          box.cx,
          box.cy + box.u * 1.2,
          0.62,
        );
        return missing ? 'this scene has no renderer in this checkout' : 'the renderer is loading';
      }
      const s = surface(id, box, sketch);
      s.frame.box = box;
      s.frame.model = model;
      s.frame.t = t;
      s.frame.moving = moving;
      if (s.p5.width !== box.w || s.p5.height !== box.h) s.p5.resizeCanvas(box.w, box.h, true);
      s.p5.redraw();
      if (s.ready) c.drawImage(s.snap, 0, 0, box.w, box.h);
      return s.frame.fact || 'the scene is being drawn';
    };
  }

  // ---------------------------------------------------------------- shared drawing
  const mono = () => getComputedStyle(document.body).getPropertyValue('--font-mono').trim() || 'monospace';
  function text(p, box, str, x, y, scale, align, alpha) {
    // Written to the surface's own 2D context: the pinned p5 ignores `textSize` once a font
    // stack has been named, and the context is what its own text() draws through anyway.
    const g = p.drawingContext;
    g.save();
    g.font = `${Math.max(9, Math.round(box.u * scale * 1.4))}px ${mono()}`;
    g.fillStyle = withAlpha(token('--mj-fg', '#dde'), alpha === undefined ? 0.92 : alpha);
    g.textAlign = align === 'left' ? 'left' : align === 'right' ? 'right' : 'center';
    g.textBaseline = 'middle';
    g.fillText(String(str).slice(0, 90), x, y);
    g.restore();
  }
  function dim(p, box, str, x, y, scale, align) {
    text(p, box, str, x, y, scale, align, 0.55);
  }
  const grouped = (obj) =>
    Object.entries(obj || {})
      .filter(([, n]) => typeof n === 'number')
      .sort((a, b) => rank(a[0]) - rank(b[0]) || b[1] - a[1] || a[0].localeCompare(b[0]));

  // ---------------------------------------------------------------- the scenes
  // 1. Proof Lattice: one cell per rule, grouped by what the proof report decided.
  registry.scenes.proof = scene('proof', (p, f) => {
    const { box, model: m, t, moving } = f;
    const rules = [...(m.rules || [])].sort((a, b) => rank(a.state) - rank(b.state) || a.id.localeCompare(b.id));
    const left = box.w * 0.05;
    const top = box.h * 0.17;
    const area = { w: box.w * 0.6, h: box.h * 0.66 };
    const n = Math.max(rules.length, 1);
    const cols = Math.max(1, Math.round(Math.sqrt((n * area.w) / area.h)));
    const rows = Math.ceil(n / cols);
    const cell = Math.min(area.w / cols, area.h / rows);
    const gap = cell * 0.14;
    p.noStroke();
    rules.forEach((r, i) => {
      const x = left + (i % cols) * cell;
      const y = top + Math.floor(i / cols) * cell;
      // the breathing is decoration: it is a function of the rule's name, not of its state
      const breathe = moving ? 0.5 + 0.5 * Math.sin(t * 0.7 + unit(r.id, 11) * TAU) : 0.5;
      const alpha = r.state === 'gated' || r.state === 'reviewed' ? 0.95 : 0.55 + 0.2 * breathe;
      p.fill(paint(r.state, alpha));
      p.rect(x + gap / 2, y + gap / 2, cell - gap, cell - gap, cell * 0.22);
      if (r.class === 'advisory') {
        // an advisory rule carries a pin; a blocking one is the plain cell
        p.fill(withAlpha(token('--mj-bg', '#000'), 0.7));
        p.circle(x + cell / 2, y + cell / 2, cell * 0.22);
      }
    });
    // the counts, as bars: the width is the number of rules in the state
    const states = grouped(m.states);
    const most = Math.max(1, ...states.map(([, v]) => v));
    const bx = box.w * 0.7;
    const bw = box.w * 0.22;
    states.forEach(([word, count], i) => {
      const y = box.h * 0.22 + i * box.u * 2.4;
      p.fill(paint(word, 0.9));
      p.rect(bx, y, Math.max(2, (count / most) * bw), box.u * 0.9, box.u * 0.2);
      text(p, box, count, bx - box.u * 0.5, y + box.u * 0.45, 0.9, 'right');
      text(p, box, plain(word), bx + Math.max(2, (count / most) * bw) + box.u * 0.6, y + box.u * 0.45, 0.62, 'left');
    });
    dim(p, box, `${m.total} rule(s) · each cell is one · a pin marks an advisory rule`, left, box.h * 0.1, 0.62, 'left');
    text(p, box, `verdict of the report: ${m.verdict || 'unstated'}`, bx, box.h * 0.1, 0.62, 'left');
    const cov = m.coverage || {};
    const named = typeof cov.named_proof === 'number' ? `${cov.named_proof} name a proof` : null;
    const shown = rules.length < m.total ? ` · ${rules.length} of ${m.total} drawn` : '';
    return `${m.total} rule(s) · ${states.map(([w, c]) => `${c} ${plain(w)}`).join(' · ')}${named ? ` · ${named}` : ''}${shown}`;
  });

  // 2. Claim Ledger: one bar per kind of claim, split by what the evidence says of it.
  registry.scenes.claims = scene('claims', (p, f) => {
    const { box, model: m } = f;
    const groups = [...(m.groups || [])].sort((a, b) => b.total - a.total || a.status.localeCompare(b.status));
    const most = Math.max(1, ...groups.map((g) => g.total));
    const left = box.w * 0.2;
    const width = box.w * 0.62;
    const top = box.h * 0.26;
    const row = Math.min(box.u * 4.4, (box.h * 0.5) / Math.max(groups.length, 1));
    const seen = {};
    groups.forEach((g, i) => {
      const y = top + i * row;
      let x = left;
      grouped(g.states).forEach(([word, count]) => {
        const w = (count / most) * width;
        seen[word] = (seen[word] || 0) + count;
        p.noStroke();
        p.fill(paint(word, 0.88));
        p.rect(x, y, Math.max(1.5, w - 2), row * 0.62, row * 0.12);
        if (w > box.u * 3) text(p, box, count, x + w / 2, y + row * 0.31, 0.7, 'center', 1);
        x += w;
      });
      text(p, box, g.status, left - box.u, y + row * 0.31, 0.7, 'right');
      dim(p, box, `${g.total}`, x + box.u * 0.5, y + row * 0.31, 0.62, 'left');
    });
    const keys = grouped(seen);
    keys.forEach(([word, count], i) => {
      const x = box.w * 0.14 + i * box.w * 0.18;
      p.noStroke();
      p.fill(paint(word, 0.9));
      p.rect(x, box.h * 0.84, box.u * 0.7, box.u * 0.7, box.u * 0.15);
      text(p, box, `${plain(word)} · ${count}`, x + box.u * 1.0, box.h * 0.84 + box.u * 0.35, 0.62, 'left');
    });
    dim(p, box, `${m.total} claim(s) · a bar is one kind of claim, split by the evidence's word`, box.w * 0.14, box.h * 0.14, 0.62, 'left');
    const state = m.complete === false ? ' · the examination is not complete' : '';
    return `${m.total} claim(s) in ${groups.length} kind(s) · ${keys.map(([w, c]) => `${c} ${plain(w)}`).join(' · ')}${state}`;
  });

  // 3. Surface Atlas: the counts the quality report measured, as arcs.
  registry.scenes.surface = scene('surface', (p, f) => {
    const { box, model: m, t, moving } = f;
    const ops = m.operations || {};
    const canonical = typeof ops.canonical === 'number' && ops.canonical > 0 ? ops.canonical : null;
    const cx = box.w * 0.3;
    const cy = box.h * 0.52;
    const r0 = Math.min(box.w, box.h) * 0.12;
    const step = Math.min(box.w, box.h) * 0.065;
    p.noFill();
    const surfaces_ = [
      ['cli', 'cli', 'cyan'],
      ['http', 'http', 'violet'],
      ['openapi', 'openapi', 'blue'],
      ['mcp', 'mcp', 'green'],
    ];
    surfaces_.forEach(([key, name, colour], i) => {
      const n = ops[key];
      if (typeof n !== 'number' || !canonical) return;
      const r = r0 + i * step;
      p.strokeWeight(step * 0.5);
      p.stroke(withAlpha(token('--mj-line', '#445'), 0.45));
      p.arc(cx, cy, r * 2, r * 2, -p.HALF_PI, -p.HALF_PI + TAU);
      // the sweep is decoration: the arc ends where the count says, and only the glint moves
      p.stroke(withAlpha(tone(colour), 0.92));
      p.arc(cx, cy, r * 2, r * 2, -p.HALF_PI, -p.HALF_PI + (n / canonical) * TAU);
      if (moving) {
        const a = -p.HALF_PI + (n / canonical) * TAU * (0.5 + 0.5 * Math.sin(t * 0.4 + i));
        p.noStroke();
        p.fill(withAlpha(token('--mj-fg', '#fff'), 0.7));
        p.circle(cx + Math.cos(a) * r, cy + Math.sin(a) * r, step * 0.22);
        p.noFill();
      }
      text(p, box, `${name} ${n}`, cx + r0 * 0.1, cy - r - 0, 0.0001, 'left', 0);
    });
    text(p, box, canonical === null ? '—' : canonical, cx, cy - box.u * 0.5, 1.9);
    dim(p, box, 'operations', cx, cy + box.u * 1.2, 0.62);
    // the legend of arcs, in the order they are drawn
    surfaces_.forEach(([key, name, colour], i) => {
      const y = box.h * 0.2 + i * box.u * 1.9;
      p.noStroke();
      p.fill(tone(colour));
      p.rect(box.w * 0.52, y - box.u * 0.4, box.u * 0.8, box.u * 0.8, box.u * 0.15);
      text(p, box, `${name}  ${typeof ops[key] === 'number' ? ops[key] : '—'} of ${canonical === null ? '—' : canonical}`, box.w * 0.52 + box.u * 1.3, y, 0.7, 'left');
    });
    // the same counts for modules and the public API, as bars against their own totals
    const bars = [];
    const mods = m.modules || {};
    if (typeof mods.total === 'number') {
      bars.push(['modules documented', mods.documented, mods.total, 'cyan']);
      bars.push(['modules with an example', mods.exampled, mods.total, 'violet']);
      bars.push(['modules behaviourally tested', mods.tested, mods.total, 'green']);
    }
    const api = m.api || {};
    if (typeof api.items === 'number') {
      bars.push(['public items documented', api.documented, api.items, 'cyan']);
      if (typeof api.owe_example === 'number') bars.push(['items that owe an example, shown', api.exampled, api.owe_example, 'blue']);
    }
    bars.forEach(([name, have, of, colour], i) => {
      const y = box.h * 0.5 + i * box.u * 2.2;
      const w = box.w * 0.3;
      const x = box.w * 0.52;
      p.noStroke();
      p.fill(withAlpha(token('--mj-line', '#445'), 0.45));
      p.rect(x, y, w, box.u * 0.8, box.u * 0.2);
      if (typeof have === 'number' && of > 0) {
        p.fill(withAlpha(tone(colour), 0.92));
        p.rect(x, y, Math.max(2, (have / of) * w), box.u * 0.8, box.u * 0.2);
      }
      text(p, box, `${name}  ${have === undefined || have === null ? '—' : have} of ${of}`, x, y - box.u * 0.8, 0.58, 'left');
    });
    dim(p, box, `measured on ${m.target || 'the crate'}; every arc is a count, none is estimated`, box.w * 0.52, box.h * 0.9, 0.55, 'left');
    const sums = surfaces_.map(([k, n]) => `${n} ${typeof ops[k] === 'number' ? ops[k] : '—'}`).join(' · ');
    return `${canonical === null ? 'no' : canonical} canonical operation(s) · ${sums}`;
  });

  // 4. Wave Front: one column per wave of the plan, split by the issues' recorded status.
  registry.scenes.waves = scene('waves', (p, f) => {
    const { box, model: m } = f;
    const waves = [...(m.waves || [])].sort((a, b) => a.wave - b.wave);
    const most = Math.max(1, ...waves.map((w) => w.issues));
    const left = box.w * 0.08;
    const base = box.h * 0.8;
    const height = box.h * 0.58;
    const slot = (box.w * 0.84) / Math.max(waves.length, 1);
    const seen = {};
    p.stroke(withAlpha(token('--mj-line', '#445'), 0.8));
    p.strokeWeight(1);
    p.line(left, base, left + slot * waves.length, base);
    p.noStroke();
    waves.forEach((w, i) => {
      const x = left + i * slot + slot * 0.12;
      const width = slot * 0.76;
      let y = base;
      grouped(w.by_status).forEach(([word, count]) => {
        const h = (count / most) * height;
        seen[word] = (seen[word] || 0) + count;
        p.fill(paint(word, 0.9));
        p.rect(x, y - h, width, Math.max(1, h - 1), 2);
        y -= h;
      });
      text(p, box, w.issues, x + width / 2, y - box.u * 0.9, 0.8);
      dim(p, box, `wave ${w.wave}`, x + width / 2, base + box.u * 1.1, 0.58);
    });
    grouped(seen).forEach(([word, count], i) => {
      const x = box.w * 0.08 + i * box.w * 0.15;
      p.noStroke();
      p.fill(paint(word, 0.9));
      p.rect(x, box.h * 0.9, box.u * 0.7, box.u * 0.7, box.u * 0.15);
      text(p, box, `${plain(word)} · ${count}`, x + box.u * 1.0, box.h * 0.9 + box.u * 0.35, 0.6, 'left');
    });
    dim(p, box, 'issues that may run at the same time, once the waves before are done', box.w * 0.08, box.h * 0.12, 0.62, 'left');
    const total = waves.reduce((n, w) => n + w.issues, 0);
    return `${waves.length} wave(s) · ${total} issue(s) · ${m.serialised} scope(s) hold their issues in series`;
  });

  // 5. Release Matrix: one tile per check of the published installation.
  registry.scenes.matrix = scene('matrix', (p, f) => {
    const { box, model: m } = f;
    const checks = m.checks || [];
    const cols = Math.min(3, Math.max(1, checks.length));
    const rows = Math.ceil(checks.length / cols);
    const left = box.w * 0.08;
    const top = box.h * 0.2;
    const cw = (box.w * 0.84) / cols;
    const ch = Math.min(box.h * 0.42 / rows, box.u * 9);
    checks.forEach((c, i) => {
      const x = left + (i % cols) * cw + cw * 0.04;
      const y = top + Math.floor(i / cols) * ch + ch * 0.04;
      p.noStroke();
      p.fill(paint(c.state, c.state === 'unknown' ? 0.18 : 0.28));
      p.rect(x, y, cw * 0.92, ch * 0.92, box.u * 0.5);
      if (c.state === 'unknown') {
        p.noFill();
        p.stroke(paint(c.state, 0.7));
        p.drawingContext.setLineDash([box.u * 0.5, box.u * 0.4]);
        p.strokeWeight(box.u * 0.12);
        p.rect(x, y, cw * 0.92, ch * 0.92, box.u * 0.5);
        p.drawingContext.setLineDash([]);
        p.noStroke();
      } else {
        p.fill(paint(c.state, 0.95));
        p.rect(x, y, box.u * 0.35, ch * 0.92, box.u * 0.17);
      }
      text(p, box, String(c.id).toUpperCase(), x + box.u * 1.0, y + ch * 0.28, 0.9, 'left');
      text(p, box, plain(c.state), x + box.u * 1.0, y + ch * 0.55, 0.7, 'left');
      if (c.observed) dim(p, box, c.observed, x + box.u * 1.0, y + ch * 0.78, 0.5, 'left');
    });
    // the targets: one square per required platform, filled when an artifact is published
    const need = typeof m.required_targets === 'number' ? m.required_targets : 0;
    const have = typeof m.published_artifacts === 'number' ? m.published_artifacts : 0;
    const sq = Math.min(box.u * 2.2, (box.w * 0.6) / Math.max(need, 1));
    for (let i = 0; i < need; i += 1) {
      const x = box.w * 0.08 + i * sq * 1.2;
      const y = box.h * 0.8;
      if (i < have) {
        p.noStroke();
        p.fill(paint('ok', 0.9));
      } else {
        p.noFill();
        p.stroke(paint('failed', 0.9));
        p.strokeWeight(2);
      }
      p.rect(x, y, sq, sq, sq * 0.2);
    }
    p.noStroke();
    text(p, box, `${have} of ${need} required target(s) have a published artifact`, box.w * 0.08 + Math.max(need, 1) * sq * 1.2 + box.u, box.h * 0.8 + sq / 2, 0.66, 'left');
    dim(p, box, `stable ${m.stable_tag || 'unstated'} · this checkout runs ${m.local_version || 'unstated'}`, box.w * 0.08, box.h * 0.12, 0.62, 'left');
    const words = {};
    checks.forEach((c) => { words[c.state] = (words[c.state] || 0) + 1; });
    return `${checks.length} check(s): ${Object.keys(words).sort().map((w) => `${words[w]} ${plain(w)}`).join(' · ')} · ${m.installable === true ? 'installable' : m.installable === false ? 'not installable' : 'installability unstated'}`;
  });
})();

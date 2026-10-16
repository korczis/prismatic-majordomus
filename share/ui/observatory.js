// The Observatory's renderer: `/ui/observatory` and its kiosk preset `/ui/kiosk` (apps/majordomus-cli/src/ui/observatory.rs).
//
// It draws the projection the page carries (`data-observatory`) and decides nothing about
// it. Which scenes exist, what each requires, whether it is available and why not, what a
// mark means and how long a scene is shown are the server's; this file turns each scene's
// model into marks on a canvas, rotates the scenes, and reads the page again for a newer
// projection. It asks the server for nothing but the page it is on.
//
// A mark is a fact or it is decoration. Every node, ring, bar and stroke below is drawn from
// a value in a scene's model. The drifting dust, the breathing and the orbital motion are
// decoration: they move whether or not anything happened, and the display says so.
//
// On a snapshot of the UI (ADR 0141) nothing is read again: the projection is the one reading
// taken at the published commit, and the status says SNAPSHOT.
(function () {
  'use strict';

  // The page is read again every POLL_MS for every scene; the journal's events are followed
  // between those readings from a cursor every STREAM_MS, through observatory.events.
  const POLL_MS = 60000;
  const STREAM_MS = 3000;
  // The events the river holds at most, newest first: the server's bound on one scene.
  const HELD = 200;
  const POLL_MAX_MS = 120000;
  const LIVE_WITHIN_MS = 90000;
  const FADE_S = 1.2;
  const MANUAL_HOLD_S = 300;
  // New events of the journal interrupt the rotation for the river, but only after the scene
  // shown has had MIN_DWELL_S, and at most once per INTERRUPT_COOLDOWN_S, so a busy journal
  // cannot hold the display on one scene. A reader's choice is never interrupted.
  const MIN_DWELL_S = 12;
  const INTERRUPT_COOLDOWN_S = 120;
  const TRACE = 24;
  const SEEN = 4000;
  // The marks one frame registers at most for a reader to point at: a scene is bounded by
  // the server, and this bounds what one frame keeps of it.
  const MARKS = 1200;
  // The colour role each kind of mark is filed under (share/design/tokens.yaml, `obs-*`): the
  // ring around a mark a reader points at and the detail it opens wear it. A kind not filed
  // here wears the accent.
  const KIND_ROLE = {
    machine: 'instance', runtime: 'instance', checkout: 'instance',
    'mesh session': 'peer', worker: 'peer', claim: 'peer',
    execution: 'task', 'recorded run': 'task',
    'issues of a wave': 'issue', 'claims of one kind': 'issue',
    milestone: 'milestone',
    'health dimension': 'state', 'release check': 'state', rule: 'state', 'rules in one state': 'state', source: 'state',
    event: 'event',
    release: 'release', 'declared version': 'release', 'module of the registry': 'release', surface: 'release', 'measured share': 'release',
  };
  const kindRole = (kind) => (KIND_ROLE[kind] ? `--mj-obs-${KIND_ROLE[kind]}` : '--mj-accent');

  // ---------------------------------------------------------------- colour and number
  const probe = document.createElement('canvas').getContext('2d', { willReadFrequently: true });
  const tokenCache = new Map();
  function token(name, fallback) {
    if (tokenCache.has(name)) return tokenCache.get(name);
    let out = fallback;
    const v = getComputedStyle(document.body).getPropertyValue(name).trim();
    if (v && probe) {
      probe.clearRect(0, 0, 1, 1);
      probe.fillStyle = '#000';
      probe.fillStyle = v;
      probe.fillRect(0, 0, 1, 1);
      const [r, g, b, a] = probe.getImageData(0, 0, 1, 1).data;
      const hex = (n) => n.toString(16).padStart(2, '0');
      if (a !== 0) out = `#${hex(r)}${hex(g)}${hex(b)}`;
    }
    tokenCache.set(name, out);
    return out;
  }
  const tone = (t) => token(`--mj-tone-${t}-fill`, token('--mj-accent-fill', '#8aa'));
  // The tone a state word is drawn in. A word this does not know is drawn as unknown.
  const TONES = {
    ok: 'cyan', live: 'cyan', active: 'cyan', DONE: 'cyan', answered: 'cyan', held: 'cyan',
    warn: 'orange', stale: 'orange', BLOCKED: 'orange', VERIFY: 'orange', suspect: 'orange',
    fail: 'rose', failed: 'rose', conflicted: 'rose',
    ACTIVE: 'violet', READY: 'blue', PLANNED: 'blue', open: 'violet',
  };
  function colour(word) {
    const t = TONES[word];
    return t ? tone(t) : token('--mj-muted', '#889');
  }
  // A token's colour at an opacity: the same colour with its alpha channel written after it.
  // A colour that did not resolve to six digits is left as it is.
  function withAlpha(colour_, a) {
    if (colour_.length !== 7) return colour_;
    return `${colour_}${Math.round(Math.max(0, Math.min(1, a)) * 255).toString(16).padStart(2, '0')}`;
  }
  // A stable number in [0, 1) from a string: where a mark sits is a function of its identity.
  function unit(text, salt) {
    let h = 2166136261 ^ (salt || 0);
    for (let i = 0; i < text.length; i += 1) {
      h ^= text.charCodeAt(i);
      h = Math.imul(h, 16777619);
    }
    return ((h >>> 0) % 100000) / 100000;
  }
  const TAU = Math.PI * 2;

  // ---------------------------------------------------------------- the scenes
  // Each takes the drawing context, the stage's box, the scene's model, the time in seconds
  // and whether decoration may move, and returns the one line of fact shown under the title.
  const draw = {
    mesh(c, box, m, t, moving) {
      const machines = m.machines || [];
      let live = 0;
      let runtimes = 0;
      machines.forEach((machine, i) => {
        const local = machine.local === true;
        const angle = unit(machine.id, 1) * TAU;
        const reach = local ? 0 : 0.2 + 0.16 * unit(machine.id, 2) + 0.05 * (i % 3);
        const x = box.cx + Math.cos(angle) * reach * box.w;
        const y = box.cy + Math.sin(angle) * reach * box.h * 0.9;
        const r = box.u * (1.1 + Math.min(machine.runtimes.length, 12) * 0.12);
        c.beginPath();
        c.arc(x, y, r, 0, TAU);
        c.fillStyle = withAlpha(tone('violet'), 0.22);
        c.fill();
        c.lineWidth = box.u * 0.08;
        c.strokeStyle = local ? token('--mj-fg', '#fff') : tone('violet');
        c.stroke();
        machine.runtimes.forEach((runtime, k) => {
          runtimes += 1;
          if (runtime.liveness === 'live') live += 1;
          const orbit = r + box.u * (1.1 + 0.8 * (k % 5));
          const speed = moving ? 0.05 + 0.03 * unit(runtime.id, 3) : 0;
          const a = unit(runtime.id, 4) * TAU + t * speed;
          const sx = x + Math.cos(a) * orbit;
          const sy = y + Math.sin(a) * orbit;
          c.beginPath();
          c.arc(sx, sy, box.u * (runtime.this ? 0.34 : 0.22) + box.u * 0.04 * Math.min(runtime.sessions, 6), 0, TAU);
          c.fillStyle = colour(runtime.liveness);
          c.globalAlpha = runtime.liveness === 'live' ? 1 : 0.45;
          c.fill();
          c.globalAlpha = 1;
          box.mark({
            x: sx, y: sy, r: box.u * 0.5, id: runtime.id, kind: 'runtime', state: runtime.liveness, truth: 'observed',
            fields: [['machine', machine.label], ['sessions', runtime.sessions], ['this runtime', runtime.this ? 'yes' : 'no']],
          });
        });
        box.mark({
          x, y, r, id: machine.id, kind: 'machine', state: local ? 'this machine' : 'known', truth: 'observed',
          fields: [['name', machine.label], ['runtimes', machine.runtimes.length]],
        });
        label(c, box, `${machine.label}${local ? (m.replayed ? ' · where it was recorded' : ' · this machine') : ''}`, x, y + r + box.u * 2.6, 0.62);
      });
      const tense = m.replayed ? 'were live when recorded' : 'live';
      return `${m.known} machine(s) · ${live} of ${runtimes} runtime(s) ${tense} · ${m.refused} refused link(s)`;
    },

    activity(c, box, m, t, moving, born) {
      const events = m.events || [];
      if (!events.length) {
        label(c, box, 'the journal holds no event in this window', box.cx, box.cy, 0.9);
        return `0 event(s) · journal clock ${m.lamport === null ? 'unknown' : m.lamport}`;
      }
      const kinds = [...new Set(events.map((e) => e.kind))].sort();
      const stamps = events.map((e) => e.at).filter((n) => typeof n === 'number');
      const from = Math.min(...stamps);
      const to = Math.max(...stamps);
      const span = Math.max(to - from, 1);
      const top = box.y + box.h * 0.2;
      const band = (box.h * 0.6) / kinds.length;
      const left = box.x + box.w * 0.2;
      const width = box.w * 0.74;
      kinds.forEach((kind, i) => {
        const y = top + band * (i + 0.5);
        c.strokeStyle = withAlpha(token('--mj-line', '#445'), 0.6);
        c.lineWidth = 1;
        c.beginPath();
        c.moveTo(left, y);
        c.lineTo(left + width, y);
        c.stroke();
        label(c, box, kind.replace(/_/g, ' '), box.x + box.w * 0.1, y + box.u * 0.25, 0.6);
      });
      const now = performance.now();
      for (const e of events) {
        if (typeof e.at !== 'number') continue;
        const i = kinds.indexOf(e.kind);
        const x = left + ((e.at - from) / span) * width;
        const y = top + band * (i + 0.5) + (unit(e.id, 5) - 0.5) * band * 0.6;
        c.fillStyle = tone(['cyan', 'violet', 'blue', 'green', 'orange', 'rose'][i % 6]);
        c.globalAlpha = 0.85;
        c.fillRect(x - box.u * 0.06, y - box.u * 0.5, box.u * 0.12, box.u);
        c.globalAlpha = 1;
        box.mark({
          x: x - box.u * 0.3, y: y - box.u * 0.5, w: box.u * 0.6, h: box.u, id: e.id, kind: 'event', state: e.kind, truth: 'observed',
          fields: [['node', e.node], ['logical clock', e.lamport], ['at', new Date(e.at * 1000).toISOString()]],
        });
        // an event read after the display started arrives with one finite ring
        const age = born.has(e.id) ? (now - born.get(e.id)) / 1000 : 99;
        if (age < 3 && moving) {
          c.beginPath();
          c.arc(x, y, box.u * (0.4 + age * 1.4), 0, TAU);
          c.strokeStyle = withAlpha(token('--mj-fg', '#fff'), 1 - age / 3);
          c.lineWidth = box.u * 0.06;
          c.stroke();
        }
      }
      const latest = new Date(to * 1000).toISOString().replace('T', ' ').replace(/\.\d+Z$/, 'Z');
      return `the newest ${events.length} event(s) · ${kinds.length} kind(s) · latest ${latest} · journal clock ${m.lamport}`;
    },

    intents(c, box, m) {
      const list = m.milestones || [];
      const at = new Map();
      list.forEach((ms, i) => {
        const a = i * 2.39996;
        const reach = Math.sqrt((i + 0.6) / list.length) * 0.42;
        at.set(ms.id, [box.cx + Math.cos(a) * reach * box.w * 0.95, box.cy + Math.sin(a) * reach * box.h]);
      });
      c.lineWidth = 1;
      c.strokeStyle = withAlpha(token('--mj-line', '#445'), 0.8);
      for (const ms of list) {
        for (const dep of ms.depends_on || []) {
          if (!at.has(dep)) continue;
          c.beginPath();
          c.moveTo(...at.get(ms.id));
          c.lineTo(...at.get(dep));
          c.stroke();
        }
      }
      let done = 0;
      for (const ms of list) {
        const [x, y] = at.get(ms.id);
        const r = box.u * (0.5 + Math.sqrt(ms.total || 0) * 0.16);
        const share = ms.total ? (ms.done || 0) / ms.total : null;
        if (ms.status === 'DONE') done += 1;
        c.beginPath();
        c.arc(x, y, r, 0, TAU);
        c.fillStyle = withAlpha(colour(ms.status), 0.16);
        c.fill();
        c.lineWidth = box.u * 0.12;
        c.strokeStyle = withAlpha(colour(ms.status), 0.35);
        c.stroke();
        if (share !== null) {
          c.beginPath();
          c.arc(x, y, r, -Math.PI / 2, -Math.PI / 2 + share * TAU);
          c.strokeStyle = colour(ms.status);
          c.stroke();
        }
        box.mark({
          x, y, r, id: ms.id, kind: 'milestone', state: ms.status, truth: 'observed', href: ms.href,
          fields: [
            ['title', ms.label],
            ['issues done', share === null ? 'no issue counted' : `${ms.done || 0} of ${ms.total}`],
            ['depends on', (ms.depends_on || []).join(', ') || 'nothing'],
            ['plan', ms.id === m.now ? 'now' : ms.id === m.next ? 'next' : '—'],
          ],
        });
        if (ms.id === m.now || ms.id === m.next || ms.status === 'ACTIVE') {
          label(c, box, `${ms.id}${ms.id === m.now ? ' · now' : ms.id === m.next ? ' · next' : ''}`, x, y + r + box.u * 0.9, 0.55);
        }
      }
      return `${list.length} milestone(s) · ${done} done · now ${m.now || 'none'} · next ${m.next || 'none'}`;
    },

    tokens(c, box, m) {
      const runs = m.runs || [];
      const width = (box.w * 0.7) / Math.max(runs.length, 1);
      const amounts = runs.map((r) => (typeof r.total_tokens === 'number' ? r.total_tokens : null));
      const most = Math.max(1, ...amounts.filter((n) => n !== null));
      runs.forEach((run, i) => {
        const amount = amounts[i];
        if (amount === null) return;
        const h = (amount / most) * box.h * 0.5;
        c.fillStyle = tone(['violet', 'cyan', 'blue'][i % 3]);
        c.fillRect(box.x + box.w * 0.15 + i * width, box.cy + box.h * 0.25 - h, width * 0.7, h);
        // a run without an identity of its own is named by its place in the list, and says so
        const named = run.id || run.run_id || run.run;
        box.mark({
          x: box.x + box.w * 0.15 + i * width, y: box.cy + box.h * 0.25 - h, w: width * 0.7, h: Math.max(h, box.u),
          id: named || `run ${i + 1} of the list`, kind: 'recorded run', state: 'recorded', truth: 'observed',
          fields: [['tokens', amount], ['model', run.model || 'unstated'], ['recorded', run.at || run.recorded_at || 'unstated']],
        });
      });
      const stated = amounts.filter((n) => n !== null).length;
      return `${runs.length} recorded run(s) · ${stated} state a token total`;
    },

    heartbeat(c, box, m, t, moving) {
      const checks = m.checks || [];
      const cx = box.x + box.w * 0.36;
      const tally = {};
      checks.forEach((check, i) => {
        tally[check.status] = (tally[check.status] || 0) + 1;
        const breathe = moving ? Math.sin(t * 0.6 + i * 0.7) * box.u * 0.08 : 0;
        const r = box.u * (1.6 + i * 0.95) + breathe;
        c.beginPath();
        c.arc(cx, box.cy, r, 0, TAU);
        c.lineWidth = box.u * 0.32;
        c.strokeStyle = withAlpha(colour(check.status), check.status === 'unknown' ? 0.4 : 0.9);
        if (check.status === 'unknown') c.setLineDash([box.u * 0.5, box.u * 0.5]);
        c.stroke();
        c.setLineDash([]);
        const y = box.y + box.h * 0.2 + i * box.u * 1.7;
        c.fillStyle = colour(check.status);
        c.fillRect(box.x + box.w * 0.66, y - box.u * 0.5, box.u * 0.5, box.u * 0.5);
        label(c, box, `${check.label} · ${check.status}`, box.x + box.w * 0.66 + box.u * 1, y, 0.6, 'left');
        box.mark({
          x: box.x + box.w * 0.66, y: y - box.u * 0.8, w: box.w * 0.3, h: box.u * 1.1, id: check.id, kind: 'health dimension',
          state: check.status, truth: 'observed', fields: [['title', check.label], ['ring', `${i + 1} from the centre`]],
        });
      });
      const words = Object.keys(tally).sort().map((w) => `${tally[w]} ${w}`).join(' · ');
      return `${checks.length} dimension(s): ${words} · the report as a whole: ${m.status || 'unknown'}`;
    },

    delivery(c, box, m) {
      const list = [...(m.releases || [])].reverse();
      const base = box.y + box.h * 0.86;
      const tip = box.y + box.h * 0.16;
      c.strokeStyle = tone('cyan');
      c.lineWidth = box.u * 0.22;
      c.beginPath();
      c.moveTo(box.cx, base);
      c.lineTo(box.cx, tip);
      c.stroke();
      const every = Math.ceil(list.length / 14);
      list.forEach((release, i) => {
        const y = base - ((i + 1) / (list.length + 1)) * (base - tip);
        const side = i % 2 ? 1 : -1;
        const reach = box.w * (0.08 + 0.16 * unit(release.version, 6));
        c.strokeStyle = release.yanked ? colour('fail') : withAlpha(tone('cyan'), 0.75);
        c.lineWidth = box.u * 0.1;
        c.beginPath();
        c.moveTo(box.cx, y);
        c.quadraticCurveTo(box.cx + side * reach * 0.5, y, box.cx + side * reach, y - box.u * 1.2);
        c.stroke();
        c.beginPath();
        c.arc(box.cx + side * reach, y - box.u * 1.2, box.u * 0.26, 0, TAU);
        c.fillStyle = release.yanked ? colour('fail') : tone('cyan');
        c.fill();
        box.mark({
          x: box.cx + side * reach, y: y - box.u * 1.2, r: box.u * 0.6, id: release.version, kind: 'release',
          state: release.yanked ? 'yanked' : 'published', truth: 'observed',
          fields: [['published', release.published_at || 'unstated'], ['order', `${i + 1} of ${list.length}`]],
        });
        if (i % every === 0 || i === list.length - 1) {
          label(c, box, release.version, box.cx + side * (reach + box.u * 0.8), y - box.u * 1.0, 0.55, side > 0 ? 'left' : 'right');
        }
      });
      if (m.next) {
        c.beginPath();
        c.arc(box.cx, tip, box.u * 0.5, 0, TAU);
        c.strokeStyle = tone('violet');
        c.lineWidth = box.u * 0.12;
        c.stroke();
        label(c, box, `${m.next} · declared, not published`, box.cx, tip - box.u * 1.1, 0.6);
        box.mark({
          x: box.cx, y: tip, r: box.u * 0.8, id: m.next, kind: 'declared version', state: 'not published', truth: 'observed',
          fields: [['changes since the last release', typeof m.changes === 'number' ? m.changes : 'unstated']],
        });
      }
      const last = list[list.length - 1];
      const pending = typeof m.changes === 'number' ? ` · ${m.changes} change(s) since` : '';
      return `${m.count} published release(s) · latest ${last.version} on ${String(last.published_at).slice(0, 10)}${pending}`;
    },

    coop(c, box, m, t, moving) {
      const sessions = m.sessions || [];
      const nodes = [...new Set(sessions.map((s) => s.node))].sort();
      let active = 0;
      sessions.forEach((s, i) => {
        if (s.state === 'active') active += 1;
        const sector = nodes.indexOf(s.node) / nodes.length;
        const a = sector * TAU + (i / sessions.length) * (TAU / nodes.length) * 0.9;
        const sway = moving && s.state === 'active' ? Math.sin(t * 0.5 + i) * box.u * 0.25 : 0;
        const r = Math.min(box.w, box.h) * 0.32 + sway;
        const x = box.cx + Math.cos(a) * r * 1.5;
        const y = box.cy + Math.sin(a) * r;
        for (let k = 0; k < Math.min(s.claims, 8); k += 1) {
          c.beginPath();
          c.arc(x, y, box.u * (0.6 + k * 0.22), a - 0.9, a + 0.9);
          c.strokeStyle = withAlpha(tone('violet'), 0.6);
          c.lineWidth = box.u * 0.07;
          c.stroke();
        }
        c.beginPath();
        c.arc(x, y, box.u * 0.3, 0, TAU);
        c.fillStyle = colour(s.state);
        c.globalAlpha = s.state === 'active' ? 1 : 0.4;
        c.fill();
        c.globalAlpha = 1;
        box.mark({
          x, y, r: box.u * 0.8, id: s.id, kind: 'mesh session', state: s.state, truth: 'observed',
          fields: [['machine', s.node], ['claims held', s.claims], ['reviews asked', s.reviews]],
        });
        if (s.reviews) {
          c.beginPath();
          c.arc(x, y, box.u * 0.5, 0, TAU);
          c.strokeStyle = tone('orange');
          c.lineWidth = box.u * 0.06;
          c.stroke();
        }
      });
      label(c, box, `${m.claims} claim(s)`, box.cx, box.cy - box.u * 1.2, 0.9);
      label(c, box, `${m.handovers} handover(s) · ${m.reviews} review(s) · ${m.overlaps} overlap(s)`, box.cx, box.cy + box.u * 0.6, 0.62);
      return `${sessions.length} session(s) on ${nodes.length} machine(s) · ${active} active`;
    },

    ambient(c, box, m, t, moving) {
      for (let i = 0; i < 5; i += 1) {
        const r = box.u * (2 + i * 1.6) + (moving ? Math.sin(t * 0.25 + i) * box.u * 0.3 : 0);
        c.beginPath();
        c.arc(box.x + box.w * 0.34, box.cy, r, 0, TAU);
        c.strokeStyle = withAlpha(tone('violet'), 0.12 + 0.04 * i);
        c.lineWidth = box.u * 0.05;
        c.stroke();
      }
      const sources = m.sources || [];
      let answered = 0;
      sources.forEach((s, i) => {
        if (s.state === 'answered') answered += 1;
        const y = box.y + box.h * 0.22 + i * box.u * 1.6;
        c.fillStyle = colour(s.state);
        c.fillRect(box.x + box.w * 0.62, y - box.u * 0.45, box.u * 0.45, box.u * 0.45);
        label(c, box, `${s.capability} · ${s.state}`, box.x + box.w * 0.62 + box.u * 0.9, y, 0.6, 'left');
        box.mark({
          x: box.x + box.w * 0.62, y: y - box.u * 0.7, w: box.w * 0.34, h: box.u * 1.0, id: s.capability, kind: 'source',
          state: s.state, truth: 'observed', fields: [],
        });
      });
      return `${answered} of ${sources.length} source(s) answered · the motion on this scene is decoration`;
    },
  };

  function label(c, box, text, x, y, scale, align) {
    c.font = `${Math.round(box.u * scale * 1.4)}px ${box.font}`;
    c.textAlign = align || 'center';
    c.fillStyle = token('--mj-fg', '#dde');
    c.globalAlpha = 0.9;
    c.fillText(String(text).slice(0, 90), x, y);
    c.globalAlpha = 1;
  }

  // ---------------------------------------------------------------- the display
  function mount(holder, options) {
    const snapshot = Boolean(options && options.snapshot);
    const canvas = holder.querySelector('canvas');
    if (!canvas || !canvas.getContext) return null;
    const c = canvas.getContext('2d');
    const query = new URLSearchParams(window.location.search);
    const still =
      query.get('motion') === 'reduced' ||
      window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    let data = read(holder.dataset.observatory);
    if (!data) return null;

    // The kiosk preset is this display filling the window on the dark theme, unattended: the
    // same projection and the same scenes. A query never grants it anything.
    const kiosk = holder.dataset.observatoryMode === 'kiosk';
    if (kiosk) {
      holder.classList.add('mj-obs--kiosk');
      document.documentElement.classList.add(document.body.dataset.themeClass || 'dark');
    }

    // the scenes of ADR 0160 are another file's: it registers them here, so this one is
    // not edited for each. A scene it has not registered yet is drawn as unavailable.
    const companion = holder.dataset.observatoryCompanion;
    if (companion && !registry.companion) {
      registry.companion = true;
      const tag = document.createElement('script');
      tag.src = companion;
      tag.async = true;
      document.head.append(tag);
    }

    const note = holder.querySelector('.mj-obs-noscript');
    if (note) note.remove();
    canvas.hidden = false;
    holder.classList.add('mj-obs--drawn');

    // the heads-up display: text the server gave, set as text
    const make = (tag, cls, text) => {
      const node = document.createElement(tag);
      node.className = cls;
      if (text) node.textContent = text;
      return node;
    };
    const head = make('div', 'mj-obs-head');
    const title = make('span', 'mj-obs-title', 'OBSERVATORY');
    const status = make('span', 'mj-obs-status');
    head.append(make('span', 'mj-obs-brand', 'PRISMATIC / MAJORDOMUS'), title, status);
    const foot = make('div', 'mj-obs-foot');
    const name = make('span', 'mj-obs-scene');
    const fact = make('span', 'mj-obs-fact');
    const dots = make('span', 'mj-obs-dots');
    foot.append(name, dots, fact);
    const legend = make('p', 'mj-obs-legend');
    legend.hidden = true;
    // a mark's detail: read-only words about the record a mark was drawn from, and the link
    // to the screen that holds it as a row. Nothing in it acts on anything.
    const detail = make('aside', 'mj-obs-detail');
    detail.hidden = true;
    detail.setAttribute('aria-live', 'polite');
    detail.setAttribute('aria-label', 'The mark a reader chose');
    const controls = make('div', 'mj-obs-controls');
    const button = (text, hint, act) => {
      const b = make('button', 'mj-obs-button', text);
      b.type = 'button';
      b.title = hint;
      b.setAttribute('aria-label', hint);
      b.addEventListener('click', act);
      controls.append(b);
      return b;
    };
    holder.append(head, foot, legend, detail, controls);
    // the stage takes the keyboard's focus, so a reader can walk the marks without a pointer
    if (!holder.hasAttribute('tabindex')) holder.tabIndex = 0;

    // ------------------------------------------------------------ the director
    // The rotation is a function of the projection and the clock: the available scenes, in
    // the server's order, each for the seconds the server gave it. A reader's choice holds
    // for MANUAL_HOLD_S and then the rotation resumes. Every choice is kept, bounded, with
    // its reason.
    const trace = [];
    let current = 0;
    let previous = -1;
    let since = performance.now();
    let auto = kiosk || query.get('auto') !== '0';
    let held = 0;
    // A scene whose renderer threw is quarantined for the rest of this display's life: the
    // rotation skips it, the others go on, and the scene says why when a reader asks for it.
    const quarantined = new Map(); // scene id -> what it threw
    const eligible = () => data.scenes
      .map((s, i) => (s.available && !quarantined.has(s.id) ? i : -1))
      .filter((i) => i >= 0);
    function select(index, reason) {
      if (index === current && reason !== 'start') return;
      previous = current;
      current = index;
      since = performance.now();
      trace.push({ at: new Date().toISOString(), scene: data.scenes[index].id, reason });
      if (trace.length > TRACE) trace.shift();
      holder.dataset.scene = data.scenes[index].id;
    }
    function step(by, reason) {
      const list = reason === 'rotation' ? eligible() : data.scenes.map((_, i) => i);
      if (!list.length) return;
      const here = list.indexOf(current);
      select(list[(here + by + list.length) % list.length], reason);
    }
    const asked = holder.dataset.observatoryScene || query.get('scene');
    const first = data.scenes.findIndex((s) => s.id === asked);
    if (first >= 0) {
      held = performance.now();
      select(first, 'start');
    } else {
      select(eligible()[0] || 0, 'start');
    }

    button('◀', 'Previous scene', () => { held = performance.now(); step(-1, 'reader'); });
    const autoButton = button('AUTO', 'Rotate the scenes automatically', () => {
      auto = !auto;
      held = 0;
    });
    button('▶', 'Next scene', () => { held = performance.now(); step(1, 'reader'); });
    button('i', 'What the marks mean', () => { legend.hidden = !legend.hidden; });
    button('⛶', 'Fullscreen', () => {
      if (document.fullscreenElement) document.exitFullscreen();
      else if (holder.requestFullscreen) holder.requestFullscreen().catch(() => {});
    });
    // ------------------------------------------------------------ pointing at a mark
    // Every painter registers the marks it drew with box.mark: where the mark is, the record
    // it stands for and what the scene read of it. A reader points at one (a click, a tap, or
    // the arrow keys and Enter) and the detail says what it is and where it came from. The
    // detail holds the rotation while it is open and closes itself after MANUAL_HOLD_S
    // without a reader, so an unattended display goes back to rotating.
    let marks = []; // the last frame's marks
    let marksScene = ''; // the scene they belong to
    let hover = null; // the mark under the pointer
    let focused = -1; // the index the keyboard walks
    let chosen = null; // { scene, id, kind, mark, gone }
    let touched = 0;
    const same = (a, b) => a && b && a.id === b.id && a.kind === b.kind;
    function hit(px, py) {
      let best = null;
      let bestD = Infinity;
      const slack = Math.min(width, height) / 44 * 0.4;
      for (const m of marks) {
        let d;
        if (typeof m.w === 'number') {
          const inside = px >= m.x - slack && px <= m.x + m.w + slack && py >= m.y - slack && py <= m.y + m.h + slack;
          if (!inside) continue;
          d = Math.hypot(px - (m.x + m.w / 2), py - (m.y + m.h / 2)) / Math.max(m.w, m.h, 1);
        } else {
          const r = Math.max(m.r || 0, slack * 2);
          const dist = Math.hypot(px - m.x, py - m.y);
          if (dist > r + slack) continue;
          d = dist / r;
        }
        if (d <= bestD) { bestD = d; best = m; }
      }
      return best;
    }
    function point(e) {
      const rect = canvas.getBoundingClientRect();
      return [e.clientX - rect.left, e.clientY - rect.top];
    }
    function choose(mark, reason) {
      touched = performance.now();
      held = touched;
      if (!mark) { chosen = null; detail.hidden = true; shown = ''; return; }
      chosen = { scene: data.scenes[current].id, id: mark.id, kind: mark.kind, mark, gone: 0 };
      trace.push({ at: new Date().toISOString(), scene: chosen.scene, mark: `${mark.kind}:${mark.id}`, reason });
      if (trace.length > TRACE) trace.shift();
      detail.hidden = false;
      detailShown = '';
    }
    canvas.addEventListener('pointermove', (e) => {
      const [x, y] = point(e);
      hover = hit(x, y);
      canvas.style.cursor = hover ? 'pointer' : '';
    });
    canvas.addEventListener('pointerleave', () => { hover = null; canvas.style.cursor = ''; });
    canvas.addEventListener('click', (e) => {
      const [x, y] = point(e);
      const m = hit(x, y);
      if (m) choose(m, 'reader pointed at it');
      else if (chosen) choose(null, 'reader');
    });

    const onKey = (e) => {
      if (e.target instanceof HTMLInputElement || e.metaKey || e.ctrlKey || e.altKey) return;
      // the marks are walked only while the stage has the focus, or on the kiosk, so the
      // arrows still scroll a page the stage is a part of
      const here = kiosk || holder.contains(document.activeElement);
      if (e.key === 'ArrowRight') { held = performance.now(); choose(null); step(1, 'reader'); }
      else if (e.key === 'ArrowLeft') { held = performance.now(); choose(null); step(-1, 'reader'); }
      else if (e.key === ' ' && here) { auto = !auto; held = 0; e.preventDefault(); }
      else if (e.key === 'i') legend.hidden = !legend.hidden;
      else if (e.key === 'Escape') { focused = -1; choose(null, 'reader'); }
      else if (here && (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'j' || e.key === 'k') && marks.length) {
        e.preventDefault();
        const by = e.key === 'ArrowDown' || e.key === 'j' ? 1 : -1;
        focused = focused < 0 ? (by > 0 ? 0 : marks.length - 1) : (focused + by + marks.length) % marks.length;
        held = performance.now();
        touched = held;
        if (chosen) choose(marks[focused], 'reader walked to it');
      } else if (here && e.key === 'Enter' && focused >= 0 && marks[focused]) {
        e.preventDefault();
        choose(marks[focused], 'reader walked to it');
      }
    };
    document.addEventListener('keydown', onKey);

    // the detail's words, set as text and only when they change
    let detailShown = '';
    function sourceLine(capability) {
      const s = (data.sources || []).find((x) => x.capability === capability);
      if (!s) return `${capability} · not asked`;
      const age = typeof s.age_ms === 'number' ? ` · read ${Math.round(s.age_ms / 1000)} s before the page` : '';
      return `${capability} · ${s.state} · ${s.freshness || 'unstated'}${age}${s.reason ? ` · ${s.reason}` : ''}`;
    }
    function renderDetail(scene, word) {
      if (!chosen) return;
      // a mark has left when frames that did draw marks have gone on without it for a while:
      // a frame that drew none yet (a surface still drawing) says nothing about it
      const live = marks.find((m) => same(m, chosen)) || null;
      if (live) { chosen.mark = live; chosen.gone = 0; chosen.missing = 0; }
      else if (marks.length && !chosen.gone) {
        chosen.missing = chosen.missing || Date.now();
        if (Date.now() - chosen.missing > 1500) chosen.gone = chosen.missing;
      }
      const m = chosen.mark;
      const truth = scene.replay
        ? `REPLAY · recorded ${scene.replay.recorded_at}; not this process's reading`
        : m.truth === 'derived'
          ? 'DERIVED · counted from what the source answered'
          : 'OBSERVED · a value the source answered, drawn as it was read';
      const rows = [
        ['state', m.state === undefined || m.state === null || m.state === '' ? 'unstated' : m.state],
        ['truth', truth],
        ...scene.requires.map((cap) => ['source', sourceLine(cap)]),
        ['read', `${String(data.observed_at).replace('T', ' ')} · ${word}${data.revision ? ` · at ${data.revision}` : ''}`],
        ...(m.fields || []).map(([k, v]) => [k, v === null || v === undefined || v === '' ? 'unstated' : v]),
      ];
      const gone = chosen.gone ? `no longer in the projection since ${new Date(chosen.gone).toISOString().slice(11, 19)}Z: the last reading that held it is shown` : '';
      const text = JSON.stringify([m.kind, m.id, rows, gone, m.href || scene.href]);
      if (text === detailShown) return;
      detailShown = text;
      detail.textContent = '';
      detail.classList.toggle('mj-obs-detail--gone', Boolean(chosen.gone));
      // the kind's colour is a role of the declaration, named, never a value picked here
      detail.style.setProperty('--obs-kind', `var(${kindRole(m.kind)})`);
      const top = make('div', 'mj-obs-detail-head');
      top.append(make('span', 'mj-obs-detail-kind', m.kind), make('span', 'mj-obs-detail-id', String(m.id)));
      const close = make('button', 'mj-obs-button', '✕');
      close.type = 'button';
      close.title = 'Close the detail and let the display rotate again';
      close.setAttribute('aria-label', close.title);
      close.addEventListener('click', () => { choose(null, 'reader'); held = 0; holder.focus(); });
      top.append(close);
      detail.append(top);
      if (gone) detail.append(make('p', 'mj-obs-detail-gone', gone));
      const list = make('dl', 'mj-obs-detail-rows');
      for (const [k, v] of rows) list.append(make('dt', '', k), make('dd', '', String(v)));
      detail.append(list);
      const rowsLink = make('a', 'mj-obs-detail-link', 'Open the same facts as rows →');
      rowsLink.href = m.href || scene.href;
      detail.append(rowsLink);
    }

    // the scene dots: one button per scene, so a reader can go to any of them
    const dotButtons = [];
    function renderDots() {
      if (dotButtons.length !== data.scenes.length) {
        dots.textContent = '';
        dotButtons.length = 0;
        data.scenes.forEach((s, i) => {
          const b = make('button', 'mj-obs-dot');
          b.type = 'button';
          b.addEventListener('click', () => { held = performance.now(); choose(null); select(i, 'reader'); });
          dots.append(b);
          dotButtons.push(b);
        });
      }
      data.scenes.forEach((s, i) => {
        const b = dotButtons[i];
        b.textContent = i === current ? '◉' : s.available ? '●' : '○';
        b.title = `${s.label} · ${s.available ? 'available' : `unavailable: ${s.reason || 'no fact to draw'}`}`;
        b.setAttribute('aria-label', b.title);
        b.setAttribute('aria-current', String(i === current));
      });
    }

    // ------------------------------------------------------------ reading the page again
    const born = new Map(); // event id -> when this display first read it
    const seen = new Set();
    function ingest(next, first_) {
      const river = next.scenes.find((s) => s.id === 'activity');
      for (const e of (river && river.model && river.model.events) || []) {
        if (seen.has(e.id)) continue;
        seen.add(e.id);
        if (!first_) born.set(e.id, performance.now());
      }
      if (seen.size > SEEN) {
        for (const id of [...seen].slice(0, seen.size - SEEN)) { seen.delete(id); born.delete(id); }
      }
      const id = data.scenes[current] && data.scenes[current].id;
      data = next;
      const kept = data.scenes.findIndex((s) => s.id === id);
      current = kept >= 0 ? kept : 0;
    }
    ingest(data, true);
    let readAt = Date.now();
    let failures = 0;
    let timer = null;
    let controller = null;
    // A reading that fell due while the page was hidden is not asked, so as not to spend the
    // server on a display nobody sees; it is owed, and asked the moment the page is seen
    // again. Until then the reading's age is the display's silence, not the server's.
    let pollOwed = false;
    async function poll() {
      timer = null;
      pollOwed = document.visibilityState !== 'visible';
      if (!pollOwed) {
        controller = new AbortController();
        const give = setTimeout(() => controller.abort(), POLL_MS);
        try {
          const search = window.location.search ? `${window.location.search}&` : '?';
          const response = await fetch(`${window.location.pathname}${search}fragment`, {
            headers: { Accept: 'text/html' },
            signal: controller.signal,
          });
          if (!response.ok) throw new Error(String(response.status));
          const page = new DOMParser().parseFromString(await response.text(), 'text/html');
          const fresh = page.querySelector('[data-observatory]');
          const next = fresh && read(fresh.dataset.observatory);
          if (!next) throw new Error('no projection');
          ingest(next, false);
          syncCursor();
          readAt = Date.now();
          failures = 0;
        } catch (e) {
          failures += 1;
        } finally {
          clearTimeout(give);
          controller = null;
        }
      }
      const wait = Math.min(POLL_MS * 2 ** Math.min(failures, 3), POLL_MAX_MS);
      timer = setTimeout(poll, wait + Math.random() * 1000);
    }
    if (!snapshot) timer = setTimeout(poll, POLL_MS);

    // ------------------------------------------------------------ following the journal
    // Between readings of the page, the river follows the journal from its cursor: only the
    // events above it come back, each by its identity, so one delivered twice is one mark.
    // A cursor the journal never reached (another runtime, a restart) is expired: the page is
    // read again at once, and the status says the display resynchronised.
    let cursor = null;
    let resyncedAt = 0;
    let interrupt = null; // new events not yet shown: { events }
    let interruptedAt = -Infinity;
    let streamTimer = null;
    let streamController = null;
    let streamFailures = 0;
    let streamOwed = false;
    const river = () => data.scenes.find((s) => s.id === 'activity');
    function syncCursor() {
      const r = river();
      cursor = r && r.available && !r.replay && r.model && typeof r.model.lamport === 'number' ? r.model.lamport : null;
    }
    syncCursor();
    async function follow() {
      streamTimer = null;
      const r = river();
      const due = cursor !== null && r && r.available && !r.replay;
      streamOwed = due && document.visibilityState !== 'visible';
      if (due && !streamOwed) {
        streamController = new AbortController();
        const give = setTimeout(() => streamController.abort(), STREAM_MS * 3);
        try {
          const response = await fetch(`/api/v1/observatory/events?after=${cursor}&limit=${HELD}`, {
            headers: { Accept: 'application/json' },
            signal: streamController.signal,
          });
          if (!response.ok) throw new Error(String(response.status));
          const page = await response.json();
          if (page.expired) {
            resyncedAt = Date.now();
            if (timer) clearTimeout(timer);
            await poll();
          } else {
            const fresh = (page.events || []).filter((e) => !seen.has(e.id));
            for (const e of fresh) { seen.add(e.id); born.set(e.id, performance.now()); }
            if (fresh.length) {
              r.model.events = [...fresh, ...(r.model.events || [])].slice(0, HELD);
              interrupt = { events: (interrupt ? interrupt.events : 0) + fresh.length };
            }
            if (typeof page.cursor === 'number') { cursor = page.cursor; r.model.lamport = page.cursor; }
            readAt = Date.now();
          }
          streamFailures = 0;
        } catch (e) {
          streamFailures += 1;
        } finally {
          clearTimeout(give);
          streamController = null;
        }
      }
      const wait = Math.min(STREAM_MS * 2 ** Math.min(streamFailures, 5), POLL_MAX_MS);
      streamTimer = setTimeout(follow, wait + Math.random() * 500);
    }
    if (!snapshot) streamTimer = setTimeout(follow, STREAM_MS);

    // ------------------------------------------------------------ the frame
    let width = 0;
    let height = 0;
    function fit() {
      const ratio = Math.min(window.devicePixelRatio || 1, 2);
      const rect = holder.getBoundingClientRect();
      width = Math.max(320, Math.round(rect.width));
      height = Math.max(180, Math.round(rect.height));
      canvas.width = Math.round(width * ratio);
      canvas.height = Math.round(height * ratio);
      c.setTransform(ratio, 0, 0, ratio, 0, 0);
      tokenCache.clear();
    }
    const observer = new ResizeObserver(fit);
    observer.observe(holder);
    document.addEventListener('fullscreenchange', fit);
    fit();

    const dust = Array.from({ length: 70 }, (_, i) => ({ x: unit(`d${i}`, 7), y: unit(`d${i}`, 8), s: 0.2 + unit(`d${i}`, 9) }));
    let slow = 0;
    let last = performance.now();
    let frame = null;
    let shown = '';
    function paint(now) {
      frame = null;
      const dt = now - last;
      // a display that cannot keep up draws less decoration and every other frame
      slow = dt > 40 ? Math.min(slow + 1, 120) : Math.max(slow - 1, 0);
      const light = slow > 60;
      last = now;
      const t = now / 1000;
      const moving = !still;

      // an open detail holds the rotation; left alone for MANUAL_HOLD_S it closes itself, and
      // the display goes back to rotating as if nobody had touched it
      if (chosen) {
        if ((now - touched) / 1000 > MANUAL_HOLD_S) { choose(null); held = 0; }
        else held = now;
      }
      if (auto && (!held || (now - held) / 1000 > MANUAL_HOLD_S)) {
        held = 0;
        const river_ = data.scenes.findIndex((s) => s.id === 'activity');
        if (interrupt && current === river_) interrupt = null;
        if (interrupt && river_ >= 0 && !quarantined.has('activity')
            && (now - since) / 1000 >= MIN_DWELL_S && (now - interruptedAt) / 1000 >= INTERRUPT_COOLDOWN_S) {
          interruptedAt = now;
          select(river_, `interrupt: ${interrupt.events} new event(s)`);
          interrupt = null;
        }
        const scene = data.scenes[current];
        if ((now - since) / 1000 > scene.dwell_seconds || !scene.available || quarantined.has(scene.id)) step(1, 'rotation');
      }

      const u = Math.min(width, height) / 44;
      const drift = moving ? [Math.sin(t / 97) * u * 0.4, Math.cos(t / 131) * u * 0.4] : [0, 0];
      // what this frame's painter registers as marks a reader can point at
      const frameMarks = [];
      const box = {
        x: 0, y: 0, w: width, h: height, cx: width / 2, cy: height / 2, u,
        font: getComputedStyle(document.body).getPropertyValue('--font-mono').trim() || 'ui-monospace, monospace',
        mark(m) {
          if (frameMarks.length < MARKS && m && m.id !== undefined && m.id !== null && m.id !== '') frameMarks.push(m);
        },
      };
      // the stage's background is the stylesheet's; the canvas is cleared to show it
      c.clearRect(0, 0, width, height);
      // decoration: dust that drifts whatever the sources say
      c.fillStyle = token('--mj-muted', '#778');
      const count = light ? 20 : dust.length;
      for (let i = 0; i < count; i += 1) {
        const d = dust[i];
        const x = ((d.x + (moving ? t * 0.002 * d.s : 0)) % 1) * width;
        c.globalAlpha = 0.18 + 0.12 * d.s;
        c.fillRect(x, d.y * height, u * 0.07 * d.s + 0.6, u * 0.07 * d.s + 0.6);
      }
      c.globalAlpha = 1;

      const scene = data.scenes[current];
      const fade = Math.min((now - since) / 1000 / FADE_S, 1);
      let line = '';
      c.save();
      // a reading the server has stopped renewing is drawn dimmed and still: what it shows
      // was true when it was read, and nothing on it may look as if it were happening now
      const stale = !snapshot && !scene.replay && Date.now() - readAt >= LIVE_WITHIN_MS;
      c.globalAlpha = (still ? 1 : fade) * (stale ? 0.45 : 1);
      // a scene is the director's own, or one a companion script registered (ADR 0160)
      const painter = draw[scene.id] || registry.scenes[scene.id];
      if (quarantined.has(scene.id)) {
        label(c, box, 'QUARANTINED', box.cx, box.cy - u * 1.2, 1.5);
        label(c, box, `its renderer failed: ${quarantined.get(scene.id)}`, box.cx, box.cy + u * 1.2, 0.7);
        line = 'nothing is drawn: this scene\'s renderer failed, and the others go on';
      } else if (scene.available && painter) {
        // a replayed model says so to the scene that draws it, and its marks are not news
        const model = scene.replay ? { ...scene.model, replayed: true } : scene.model;
        try {
          line = painter(c, box, model, t, moving && !stale, scene.replay ? new Map() : born);
        } catch (e) {
          quarantined.set(scene.id, String((e && e.message) || e).slice(0, 120));
          trace.push({ at: new Date().toISOString(), scene: scene.id, reason: 'quarantined' });
          if (trace.length > TRACE) trace.shift();
          line = '';
        }
      } else {
        label(c, box, 'UNAVAILABLE', box.cx, box.cy - u * 1.2, 1.5);
        label(c, box, scene.reason || 'this display has no renderer for the scene', box.cx, box.cy + u * 1.2, 0.7);
        line = 'nothing is drawn: there is no fact to draw it from';
      }
      c.restore();
      // the marks this frame drew; a p5 scene registers its own when its surface draws, into
      // the same list, so it is kept by reference
      if (marks !== frameMarks) {
        const was = focused >= 0 ? marks[focused] : null;
        marks = frameMarks;
        focused = was ? marks.findIndex((m) => same(m, was)) : -1;
      }
      // the mark a reader points at, has walked to or has chosen wears a ring
      const ring = (m, strong) => {
        if (!m) return;
        c.save();
        c.strokeStyle = token(kindRole(m.kind), token('--mj-fg', '#fff'));
        c.globalAlpha = strong ? 0.95 : 0.6;
        c.lineWidth = Math.max(1.5, u * (strong ? 0.12 : 0.08));
        c.beginPath();
        if (typeof m.w === 'number') c.rect(m.x - u * 0.2, m.y - u * 0.2, m.w + u * 0.4, m.h + u * 0.4);
        else c.arc(m.x, m.y, Math.max(m.r || 0, u * 0.4) + u * 0.35, 0, TAU);
        c.stroke();
        c.restore();
      };
      if (marksScene !== scene.id) { marksScene = scene.id; hover = null; focused = -1; }
      ring(hover ? marks.find((m) => same(m, hover)) : null, false);
      if (focused >= 0) ring(marks[focused], false);
      if (chosen) ring(marks.find((m) => same(m, chosen)), true);

      // the words: set only when they change
      const age = Date.now() - readAt;
      // a replayed scene is a replay whatever the page is: it shows when it was recorded
      const resynced = Date.now() - resyncedAt < 10000;
      const word = scene.replay ? 'REPLAY' : snapshot ? 'SNAPSHOT' : resynced ? 'RESYNCED' : age < LIVE_WITHIN_MS ? 'LIVE' : 'STALE';
      // why a reading is old: only an asking that failed blames the server
      const silence = failures || streamFailures ? 'the server is not answering'
        : pollOwed || streamOwed ? 'not asked while the page was hidden'
          : 'asking again';
      const when = String(data.observed_at).replace('T', ' ');
      const recordedAt = scene.replay ? String(scene.replay.recorded_at).replace('T', ' ') : '';
      const text = [
        word,
        scene.replay ? `recorded ${recordedAt}${scene.replay.revision ? ` at ${scene.replay.revision}` : ''} · not live`
          : snapshot ? `read once at ${data.revision || 'an unknown commit'} · ${when}` : `read ${when}${word === 'STALE' ? ` · ${Math.round(age / 1000)} s ago, ${silence}` : ''}`,
        `${String(current + 1).padStart(2, '0')} / ${String(data.scenes.length).padStart(2, '0')}  ${scene.label.toUpperCase()}`,
        line,
        auto && !held ? 'auto' : 'held',
      ].join('|');
      if (text !== shown) {
        shown = text;
        status.textContent = `${word} · ${text.split('|')[1]}`;
        status.dataset.state = word.toLowerCase();
        name.textContent = text.split('|')[2];
        fact.textContent = line;
        legend.textContent = `${scene.meaning} Background dust and slow motion are decoration. Source: ${scene.requires.join(', ') || 'none'}. Point at a mark, or walk the marks with ↑ ↓ and Enter, to see the record it was drawn from; Esc closes it.`;
        autoButton.setAttribute('aria-pressed', String(auto));
        renderDots();
      }
      if (chosen) renderDetail(scene, word);
      head.style.transform = `translate(${drift[0]}px, ${drift[1]}px)`;
      foot.style.transform = `translate(${-drift[0]}px, ${-drift[1]}px)`;
      schedule();
    }
    function schedule() {
      if (frame !== null || document.visibilityState !== 'visible') return;
      if (still) frame = setTimeout(() => paint(performance.now()), 1000);
      else frame = requestAnimationFrame(paint);
    }
    // Seen again: what fell due while hidden is asked now, each reader at most once at a time —
    // one already asking is left to finish, and its own timer is replaced, not doubled.
    const onVisible = () => {
      last = performance.now();
      if (document.visibilityState === 'visible') {
        if (pollOwed && controller === null) {
          if (timer) clearTimeout(timer);
          poll();
        }
        if (streamOwed && streamController === null) {
          if (streamTimer) clearTimeout(streamTimer);
          follow();
        }
      }
      schedule();
    };
    document.addEventListener('visibilitychange', onVisible);
    schedule();

    const handle = {
      trace: () => trace.slice(),
      // the projection the display is drawing now, and the scenes it quarantined: what an
      // inspector or an evidence run reads, never a second model
      data: () => data,
      quarantined: () => Object.fromEntries(quarantined),
      policy: { min_dwell_seconds: MIN_DWELL_S, interrupt_cooldown_seconds: INTERRUPT_COOLDOWN_S, manual_hold_seconds: MANUAL_HOLD_S },
      // what a reader could point at now, and what the detail says: for a probe that must
      // prove a mark answers a click without reading pixels
      marks: () => marks.map(({ id, kind, state, truth, x, y, r, w, h }) => ({ id, kind, state, truth, x, y, r, w, h })),
      detail: () => (detail.hidden ? null : detail.textContent),
      dispose() {
        if (timer) clearTimeout(timer);
        if (controller) controller.abort();
        if (streamTimer) clearTimeout(streamTimer);
        if (streamController) streamController.abort();
        if (frame !== null) { cancelAnimationFrame(frame); clearTimeout(frame); }
        observer.disconnect();
        document.removeEventListener('keydown', onKey);
        document.removeEventListener('visibilitychange', onVisible);
        document.removeEventListener('fullscreenchange', fit);
        for (const node of [head, foot, legend, detail, controls]) node.remove();
        if (registry.display === handle) registry.display = null;
      },
    };
    registry.display = handle;
    return handle;
  }

  function read(text) {
    try {
      const data = JSON.parse(text || 'null');
      return data && data.schema === 'observatory/v1' && Array.isArray(data.scenes) && data.scenes.length ? data : null;
    } catch (e) {
      return null;
    }
  }

  // What a companion script needs to draw in this display's own look: the stylesheet's
  // tokens, the tone of a state word, a stable place from an identity, the text label.
  const registry = {
    mount,
    // the director's own painters, beside the companion's: a probe draws a scene's model
    // with them on a recording context and reads back the marks it registered
    painters: draw,
    scenes: {},
    companion: false,
    kit: { token, tone, colour, withAlpha, unit, label, TAU },
  };
  window.mjObservatory = registry;
})();

// The runtime activity plot: what this process is actually doing, sampled from
// /api/v1/perf while the page is on screen.
//
// Why a sketch rather than a table: the numbers are already on the page as a table, and
// the table answers "how many". What it cannot answer is "when, and in what proportion" —
// whether the executions arriving now are being answered from the cache or are running
// handlers, and whether that ratio is holding. That is a shape over time, it changes every
// second, and a DOM node per sample would be the wrong tool. Everything the plot shows is
// a delta of two counters the table also shows.
//
// Optional and lazy: p5 is vendored and loaded only here. Without it the frame says so and
// the tables stand.

import { api, vendor, palette, explainMissing, motionAllowed } from './cockpit.js';

const frame = document.querySelector('[data-mj-activity]');
if (frame) {
  draw(frame).catch((error) => explainMissing(frame, error));
}

/** How many samples the window keeps, and how often one is taken. */
const SAMPLES = 120;
const INTERVAL_MS = 1000;

async function draw(frame) {
  const p5 = await vendor('p5.min.js', 'p5');
  const source = frame.dataset.mjActivity;
  const history = [];
  let previous = null;
  let polling = 0;

  async function sample() {
    const answer = await api(source);
    if (!answer.ok) return;
    const now = {
      executions: answer.body.executions || 0,
      hits: answer.body.cache_hits || 0,
      handlers: answer.body.handler_invocations || 0,
    };
    if (previous) {
      history.push({
        executions: Math.max(now.executions - previous.executions, 0),
        hits: Math.max(now.hits - previous.hits, 0),
        handlers: Math.max(now.handlers - previous.handlers, 0),
      });
      while (history.length > SAMPLES) history.shift();
    }
    previous = now;
  }

  const start = () => {
    if (polling) return;
    sample();
    polling = window.setInterval(sample, INTERVAL_MS);
  };
  const stop = () => {
    window.clearInterval(polling);
    polling = 0;
  };

  // never poll a server for a plot nobody is looking at
  const observer = new IntersectionObserver(
    (entries) => (entries.some((e) => e.isIntersecting) && document.visibilityState === 'visible' ? start() : stop()),
    { threshold: 0 },
  );
  observer.observe(frame);
  document.addEventListener('visibilitychange', () =>
    document.visibilityState === 'visible' ? start() : stop(),
  );

  frame.textContent = '';
  // eslint-disable-next-line no-new
  new p5((sketch) => {
    const colours = palette();
    sketch.setup = () => {
      const canvas = sketch.createCanvas(frame.clientWidth || 800, frame.clientHeight || 200);
      canvas.parent(frame);
      // one frame per sample is enough: this is a plot, not an animation
      sketch.frameRate(motionAllowed() ? 8 : 1);
      sketch.noLoop();
      window.setInterval(() => sketch.redraw(), INTERVAL_MS);
    };

    sketch.windowResized = () => {
      sketch.resizeCanvas(frame.clientWidth || 800, frame.clientHeight || 200);
      sketch.redraw();
    };

    sketch.draw = () => {
      sketch.clear();
      const w = sketch.width;
      const h = sketch.height;
      const peak = Math.max(1, ...history.map((s) => s.executions));

      sketch.noStroke();
      sketch.fill(colours.muted);
      sketch.textSize(10);
      sketch.textFont('ui-monospace, SFMono-Regular, Menlo, monospace');
      if (!history.length) {
        sketch.text('waiting for the first sample of /api/v1/perf …', 8, 16);
        return;
      }
      sketch.text('executions per second · peak ' + peak, 8, 14);

      const step = w / SAMPLES;
      for (let i = 0; i < history.length; i += 1) {
        const s = history[i];
        const x = i * step;
        const total = (s.executions / peak) * (h - 30);
        const cached = s.executions ? (s.hits / s.executions) * total : 0;
        // the whole bar is the executions; the darker part is what the cache answered
        sketch.fill(colours.accent);
        sketch.rect(x, h - total, Math.max(step - 1, 1), total);
        sketch.fill(colours.border);
        sketch.rect(x, h - cached, Math.max(step - 1, 1), cached);
      }

      sketch.fill(colours.muted);
      sketch.text('lower band: answered from the cache', 8, h - 6);
    };
  });
}

// The worktrees page keeps itself current: the topology changes outside this process — a
// worktree is created, moved or removed in a terminal — and a page that showed the state
// of a minute ago would be the wrong page. While the page is on screen it asks
// /api/v1/worktrees again a few seconds after each answer, compares a digest of what
// matters (every worktree's path, standing, HEAD, uncommitted counts, and every
// diagnostic's code), and when that digest moves it reloads the page, so the server renders
// it exactly once, in one place.
//
// It is an enhancement: without JavaScript the page is complete and a refresh shows the
// same thing.

import { api } from './cockpit.js';

/** How long the page waits between one answer and the next question, while it is visible.
 * A gap, not a rate: see `watch` below for why the difference matters. Declared before the
 * first call below: a `const` is not hoisted, and a module that used it before this line
 * would throw at load. */
const INTERVAL_MS = 4000;

const frame = document.querySelector('[data-mj-worktrees]');
if (frame) {
  watch(frame);
}

function digest(topology) {
  const worktrees = (topology.worktrees || []).map((w) => [
    w.path,
    w.standing,
    w.head || '',
    w.dirty ? `${w.dirty.staged}/${w.dirty.unstaged}/${w.dirty.untracked}` : '-',
  ].join('|'));
  const diagnostics = (topology.diagnostics || []).map((d) => `${d.code}@${d.path || ''}`);
  const branches = (topology.branches || []).map((b) => `${b.name}:${b.worktree || ''}:${b.cleanup_eligible}`);
  return [...worktrees, ...diagnostics, ...branches].join('\n');
}

function watch(frame) {
  const source = frame.dataset.mjWorktrees;
  let baseline = frame.dataset.mjWorktreesDigest || null;
  let polling = 0;
  let asking = false;
  const note = frame.querySelector('[data-mj-worktrees-note]');

  async function sample() {
    const answer = await api(source);
    if (!answer.ok) {
      if (note) note.textContent = 'The topology could not be read; the page shows the last answer.';
      return;
    }
    const now = digest(answer.body);
    if (baseline === null) {
      baseline = now;
      return;
    }
    if (now !== baseline) {
      window.location.reload();
    } else if (note) {
      note.textContent = `Live: checked ${new Date().toLocaleTimeString()}; unchanged.`;
    }
  }

  // One question at a time, and the next one scheduled only once the last has answered.
  // `setInterval` would start a request every INTERVAL_MS whether or not the previous one
  // had come back: in a repository with a hundred worktrees the topology takes longer than
  // the interval, and the page would pile requests onto the server whose speed it is
  // reporting — each one a `git status` per worktree, each making the next slower. It also
  // leaves the network permanently busy, which is what a browser waits to become quiet
  // before it calls a page loaded; that is why this page, and only this page, timed out in
  // every instrument that drives a real browser.
  //
  // The first question waits an interval rather than being asked at once: the server has
  // just rendered this page from the topology, so there is nothing to compare against yet.
  async function tick() {
    polling = 0;
    asking = true;
    try {
      await sample();
    } finally {
      asking = false;
    }
    schedule();
  }
  const schedule = () => {
    if (polling || asking) return;
    if (document.visibilityState !== 'visible') return;
    polling = window.setTimeout(tick, INTERVAL_MS);
  };
  const stop = () => {
    window.clearTimeout(polling);
    polling = 0;
  };
  // never poll a server for a page nobody is looking at
  const decide = () => (document.visibilityState === 'visible' ? schedule() : stop());
  document.addEventListener('visibilitychange', decide);
  decide();
}

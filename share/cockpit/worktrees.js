// The worktrees page keeps itself current: the topology changes outside this process — a
// worktree is created, moved or removed in a terminal — and a page that showed the state
// of a minute ago would be the wrong page. While the page is on screen it asks
// /api/v1/worktrees every few seconds, compares a digest of what matters (every worktree's
// path, standing, HEAD, uncommitted counts, and every diagnostic's code), and when that
// digest moves it reloads the page, so the server renders it exactly once, in one place.
//
// It is an enhancement: without JavaScript the page is complete and a refresh shows the
// same thing.

import { api } from './cockpit.js';

/** How often the topology is asked for, while the page is visible. Declared before the
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

  const start = () => {
    if (polling) return;
    sample();
    polling = window.setInterval(sample, INTERVAL_MS);
  };
  const stop = () => {
    window.clearInterval(polling);
    polling = 0;
  };
  // never poll a server for a page nobody is looking at
  const decide = () => (document.visibilityState === 'visible' ? start() : stop());
  document.addEventListener('visibilitychange', decide);
  decide();
}

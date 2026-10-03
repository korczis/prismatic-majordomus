// The moves an issue page offers: start, verify, done.
//
// The page is complete without this. Every move is printed with the command that makes
// it, and the buttons are rendered disabled, so a page whose script did not load offers
// nothing it cannot do.
//
// A move is started as an execution of the capability the form names, on the route the
// form names, and followed on the route the form names: this module knows no capability
// id and no path by heart. Whether a move is legal is not decided here either. The one
// capability that moves an issue refuses an illegal move and names what is in the way;
// this module shows that answer as it came, and reads the page again so that every status
// on it is the server's rather than the one the button promised.

import { api } from './cockpit.js';

// a wait is bounded: an execution that has not finished in this many reads is reported
// as still running, with the link to its own page, and the page is not reloaded
const POLLS = 120;
const POLL_MS = 250;
const TERMINAL = ['succeeded', 'failed', 'cancelled'];

for (const form of document.querySelectorAll('[data-mj-transition]')) install(form);

function install(form) {
  const issue = form.dataset.mjTransition;
  const capability = form.dataset.mjCapability;
  const start = form.dataset.mjStart;
  const follow = form.dataset.mjFollow;
  // the question comes from the descriptor's own effect classification, never from a list
  // of capability ids in this file
  const effect = (form.dataset.mjEffect || 'the repository').replace(/_/g, ' ');
  const output = form.querySelector('[data-mj-result]');
  const buttons = Array.from(form.querySelectorAll('button[data-mj-move]'));
  if (!issue || !capability || !start || !follow || buttons.length === 0) return;

  form.addEventListener('submit', (event) => event.preventDefault());
  for (const button of buttons) {
    button.disabled = false;
    button.addEventListener('click', () => move(button.dataset.mjMove));
  }

  async function move(transition) {
    if (!window.confirm('Moving ' + issue + ' (' + transition + ') is a ' + effect + '. Continue?')) {
      return;
    }
    for (const button of buttons) button.disabled = true; // a double click must not move twice
    show({ pending: 'starting ' + capability + ' (' + transition + ')' });
    const started = await api(start, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ capability, input: { issue, transition } }),
    });
    if (!started.ok || !started.body || !started.body.id) {
      show({ failed: started });
      for (const button of buttons) button.disabled = false;
      return;
    }
    const id = started.body.id;
    const page = started.body.links && started.body.links.cockpit;
    let execution = null;
    for (let i = 0; i < POLLS; i++) {
      const answer = await api(follow + '?id=' + encodeURIComponent(id), { method: 'GET' });
      execution = answer.ok ? answer.body : null;
      if (execution && TERMINAL.includes(execution.state)) break;
      await new Promise((resolve) => window.setTimeout(resolve, POLL_MS));
    }
    if (!execution || !TERMINAL.includes(execution.state)) {
      show({ running: id, page });
      return;
    }
    show({ execution, page });
    if (execution.state === 'succeeded') {
      window.setTimeout(() => window.location.reload(), 900);
    } else {
      for (const button of buttons) button.disabled = false;
    }
  }

  function show(state) {
    if (!output) return;
    output.textContent = '';
    const line = document.createElement('p');
    let ok = false;
    if (state.pending) {
      line.className = 'mj-alert mj-alert--info';
      line.textContent = state.pending;
    } else if (state.failed) {
      const error = state.failed.body && state.failed.body.error;
      line.className = 'mj-alert mj-alert--fail';
      line.textContent = error
        ? error.code + ': ' + error.message
        : 'the execution was not started (' + state.failed.status + ')';
    } else if (state.running) {
      line.className = 'mj-alert mj-alert--warn';
      line.textContent = 'execution ' + state.running + ' is still running; its page follows it';
    } else {
      const execution = state.execution;
      const out = execution.output || {};
      ok = execution.state === 'succeeded';
      line.className = 'mj-alert mj-alert--' + (ok ? 'ok' : 'fail');
      // `to` is the status derived from the record after the write: a `done` whose evidence
      // is missing answers VERIFY, and that is what is shown
      line.textContent = ok
        ? out.issue + ': ' + out.from + ' → ' + out.to + ' (' + out.field + ' stamped at ' + out.at + ')'
        : (execution.error && (execution.error.code + ': ' + execution.error.message)) ||
          'the move was ' + execution.state;
    }
    line.setAttribute('role', ok || state.pending ? 'status' : 'alert');
    output.appendChild(line);
    const page = state.page;
    if (page) {
      const link = document.createElement('a');
      link.className = 'mj-link';
      link.href = page;
      link.textContent = 'The execution';
      output.appendChild(link);
    }
  }
}

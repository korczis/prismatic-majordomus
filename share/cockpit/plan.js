// The moves an issue page offers: start, verify, done.
//
// The page is complete without this. Every move is printed with the command that makes
// it, and the buttons are rendered disabled, so a page whose script did not load offers
// nothing it cannot do.
//
// Whether a move is legal is not decided here. The one capability that moves an issue
// refuses an illegal move and names what is in the way; this module sends the request,
// shows that answer as it came, and reads the page again so that every status on it is
// the server's rather than the one the button promised.

import { api } from './cockpit.js';

for (const form of document.querySelectorAll('[data-mj-transition]')) install(form);

function install(form) {
  const issue = form.dataset.mjTransition;
  const path = form.dataset.mjPath;
  // the question comes from the descriptor's own effect classification, never from a list
  // of capability ids in this file
  const effect = (form.dataset.mjEffect || 'the repository').replace(/_/g, ' ');
  const output = form.querySelector('[data-mj-result]');
  const buttons = Array.from(form.querySelectorAll('button[data-mj-move]'));
  if (!issue || !path || buttons.length === 0) return;

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
    const answer = await api(path, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ issue, transition }),
    });
    show(answer);
    if (answer.ok) {
      window.setTimeout(() => window.location.reload(), 900);
    } else {
      for (const button of buttons) button.disabled = false;
    }
  }

  function show(answer) {
    if (!output) return;
    output.textContent = '';
    const line = document.createElement('p');
    const body = answer.body || {};
    if (answer.ok) {
      line.className = 'mj-alert mj-alert--ok';
      // `to` is the status derived from the record after the write: a `done` whose evidence
      // is missing answers VERIFY, and that is what is shown
      line.textContent =
        body.issue + ': ' + body.from + ' → ' + body.to + ' (' + body.field + ' stamped at ' + body.at + ')';
    } else {
      const error = body.error;
      line.className = 'mj-alert mj-alert--fail';
      line.textContent = error
        ? error.code + ': ' + error.message
        : 'the move was not made (' + answer.status + ')';
    }
    line.setAttribute('role', answer.ok ? 'status' : 'alert');
    output.appendChild(line);
  }
}

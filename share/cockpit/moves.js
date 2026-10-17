// The moves an issue offers — start, verify, done — wherever a page renders the form: the
// Cockpit's issue page and the UI's intent workspace (I2781) load this one module. The
// workspace's proposal of an intent (I2807) is started and followed here the same way.
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

import { api } from './api.js';

// a wait is bounded: an execution that has not finished in this many reads is reported
// as still running, with the link to its own page, and the page is not reloaded
const POLLS = 120;
const POLL_MS = 250;
const TERMINAL = ['succeeded', 'failed', 'cancelled'];

/** Wire one move form: its buttons start the capability it names and follow it to the end. */
export function install(form) {
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

/** Start `capability` with `input` as an execution on `start`, and follow it on `follow` until
 * it ends or the wait is spent. Answers { failed } when it was not started, { running, page }
 * when it is still running, and { execution, page } when it ended. */
async function startAndFollow(start, follow, capability, input) {
  const started = await api(start, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ capability, input }),
  });
  if (!started.ok || !started.body || !started.body.id) return { failed: started };
  const id = started.body.id;
  const page = started.body.links && started.body.links.cockpit;
  let execution = null;
  for (let i = 0; i < POLLS; i++) {
    const answer = await api(follow + '?id=' + encodeURIComponent(id), { method: 'GET' });
    execution = answer.ok ? answer.body : null;
    if (execution && TERMINAL.includes(execution.state)) return { execution, page };
    await new Promise((resolve) => window.setTimeout(resolve, POLL_MS));
  }
  return { running: id, page };
}

/** Wire one proposal form: it starts the capability it names with the statement, title and id
 * the reader wrote, and once the record is written says where, that it is listed once staged,
 * and links to its workspace. What makes a proposal valid is the capability's to say; its
 * refusal is shown as it came. */
export function installProposal(form) {
  const capability = form.dataset.mjCapability;
  const start = form.dataset.mjStart;
  const follow = form.dataset.mjFollow;
  const open = form.dataset.mjOpen;
  const effect = (form.dataset.mjEffect || 'the repository').replace(/_/g, ' ');
  const output = form.querySelector('[data-mj-result]');
  const submit = form.querySelector('[data-mj-propose-submit]');
  if (!capability || !start || !follow || !submit || form.dataset.mjWired) return;
  form.dataset.mjWired = 'yes';
  submit.disabled = false;

  function say(kind, text, page) {
    if (!output) return;
    output.textContent = '';
    const line = document.createElement('p');
    line.className = 'mj-alert mj-alert--' + kind;
    line.setAttribute('role', kind === 'fail' ? 'alert' : 'status');
    line.textContent = text;
    output.appendChild(line);
    if (page) {
      const link = document.createElement('a');
      link.className = 'mj-link';
      link.href = page;
      link.textContent = 'The execution';
      output.appendChild(link);
    }
  }

  form.addEventListener('submit', async (event) => {
    event.preventDefault();
    const input = {};
    for (const name of ['statement', 'title', 'id']) {
      const control = form.elements[name];
      const value = control ? control.value.trim() : '';
      if (value !== '') input[name] = value;
    }
    if (!window.confirm('Proposing an intent is a ' + effect + ': it writes one record. Continue?')) return;
    submit.disabled = true; // a double click must not propose twice
    say('info', 'starting ' + capability);
    const answer = await startAndFollow(start, follow, capability, input);
    if (answer.failed) {
      const error = answer.failed.body && answer.failed.body.error;
      say('fail', error ? error.code + ': ' + error.message : 'the execution was not started (' + answer.failed.status + ')');
      submit.disabled = false;
      return;
    }
    if (answer.running) {
      say('warn', 'execution ' + answer.running + ' is still running; its page follows it', answer.page);
      return;
    }
    const execution = answer.execution;
    if (execution.state !== 'succeeded') {
      say('fail', (execution.error && (execution.error.code + ': ' + execution.error.message)) || 'the proposal was ' + execution.state, answer.page);
      submit.disabled = false;
      return;
    }
    const out = execution.output || {};
    // the index reads the records git tracks, so the proposal is listed once it is staged;
    // the page says so rather than opening a workspace that would not know it yet
    say('ok', 'proposed ' + out.id + ' in ' + out.path + '. The workspace reads the records git tracks: stage it (git add ' + out.path + ') and open it.', answer.page);
    if (open && out.id && output) {
      const link = document.createElement('a');
      link.className = 'mj-link';
      link.href = open + encodeURIComponent(out.id);
      link.textContent = 'Open ' + out.id;
      output.appendChild(document.createTextNode(' '));
      output.appendChild(link);
    }
    // the reader's text is in the record now: clear it so the screen may be read again
    form.reset();
    submit.disabled = false;
  });
}

/** Wire every move form and every proposal form under `root`. */
export function installAll(root = document) {
  for (const form of root.querySelectorAll('[data-mj-transition]')) install(form);
  for (const form of root.querySelectorAll('[data-mj-propose]')) installProposal(form);
}

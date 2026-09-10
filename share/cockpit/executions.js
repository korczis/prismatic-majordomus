// The execution views: the button that starts one, the list that follows every execution,
// and the page that follows one.
//
// Nothing here knows a capability, a route or an event type by heart. The markup the
// server rendered carries every URL this module uses, and every frame it renders is
// dispatched on the `type` the server sent — an event type this build does not know is
// shown as a line rather than dropped, which is what lets the server add one without
// breaking a browser that has not been reloaded.
//
// The page is complete before this runs. What this adds is that it stops being a
// photograph.

import { api } from './cockpit.js';
import { follow, LIVE, RECONNECTING, OFFLINE } from './socket.js';

installStartButton();
installExecutionPage();
installListPage();

// ------------------------------------------------------------------ start an execution

function installStartButton() {
  const form = document.querySelector('[data-mj-runner]');
  const button = document.querySelector('[data-mj-execute]');
  if (!form || !button) return;
  const capability = button.dataset.mjExecute;
  const startRoute = form.dataset.mjStart;
  const confirmFirst = form.dataset.mjConfirm === 'yes';

  button.addEventListener('click', async () => {
    if (confirmFirst) {
      const effect = form.dataset.mjEffect || 'something';
      // the question comes from the descriptor's own effect classification, never from a
      // list of capability ids in this file
      if (!window.confirm('Running ' + capability + ' changes ' + effect.replace(/_/g, ' ') + '. Start it?')) {
        return;
      }
    }
    button.disabled = true; // a double click must not start two
    const previous = button.textContent;
    button.textContent = 'Starting…';
    const answer = await api(startRoute, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ capability, input: collect(form) }),
    });
    if (answer.ok && answer.body && answer.body.links) {
      window.location.href = answer.body.links.cockpit;
      return;
    }
    button.disabled = false;
    button.textContent = previous;
    report(form, answer);
  });
}

/** The input object, from the generated form, typed as each control's schema said. */
function collect(form) {
  const input = {};
  for (const control of form.querySelectorAll('[data-mj-type]')) {
    const name = control.getAttribute('name');
    if (!name) continue;
    const type = control.dataset.mjType;
    if (type === 'boolean') {
      if (control.checked) input[name] = true;
      continue;
    }
    const raw = control.value;
    if (raw === '' || raw === null) continue;
    if (type === 'integer' || type === 'number') {
      const n = Number(raw);
      input[name] = Number.isFinite(n) ? n : raw;
    } else if (type === 'json') {
      try {
        input[name] = JSON.parse(raw);
      } catch (e) {
        input[name] = raw;
      }
    } else {
      input[name] = raw;
    }
  }
  return input;
}

/** Show a refusal where the runner shows its answers. */
function report(form, answer) {
  const output = form.querySelector('[data-mj-result]');
  if (!output) return;
  output.textContent = '';
  const error = answer.body && answer.body.error;
  const alert = document.createElement('p');
  alert.className = 'mj-alert mj-alert--fail';
  alert.textContent = error
    ? error.code + ': ' + error.message
    : 'the execution could not be started (' + answer.status + ')';
  output.appendChild(alert);
}

// ------------------------------------------------------------------ one execution

function installExecutionPage() {
  const page = document.querySelector('[data-mj-execution]');
  if (!page) return;
  const id = page.dataset.mjExecution;
  const socketPath = page.dataset.mjSocket;
  const snapshotPath = page.dataset.mjSnapshot;
  const eventsPath = page.dataset.mjEvents;
  const cancelRoute = page.dataset.mjCancelRoute;
  const from = Number(page.dataset.mjSequence) || 0;
  const log = page.querySelector('[data-mj-log]');
  const indicator = connectionIndicator(page);
  let seen = from;
  let final = false;

  const channel = follow(socketPath + '&after=' + from, {
    state: indicator,
    resync: () => resync(),
    message: (frame) => {
      if (typeof frame.sequence === 'number') {
        if (frame.sequence <= seen) return; // already rendered
        seen = frame.sequence;
      }
      render(frame);
    },
  });

  function render(frame) {
    if (frame.type === 'execution.log' && log) {
      appendLog(log, frame);
      return;
    }
    if (frame.type === 'execution.progress') {
      updateProgress(page, frame.data);
      return;
    }
    // everything else changes the shape of the page enough that the server's own
    // rendering is the right answer: read it back rather than reimplementing it here
    if (
      frame.type === 'execution.completed' ||
      frame.type === 'execution.failed' ||
      frame.type === 'execution.cancelled'
    ) {
      final = true;
      channel.close();
      window.setTimeout(() => window.location.reload(), 250);
      return;
    }
    if (frame.type === 'execution.step.started' || frame.type === 'execution.step.completed') {
      touchState(page, frame);
    }
  }

  async function resync() {
    if (final) return;
    const answer = await api(snapshotPath);
    if (!answer.ok || !answer.body) return;
    if (answer.body.state && isFinalState(answer.body.state)) {
      window.location.reload();
      return;
    }
    if (answer.body.progress) updateProgress(page, answer.body.progress);
    const history = await api(eventsPath + '&after=' + seen);
    if (history.ok && history.body && Array.isArray(history.body.events)) {
      for (const event of history.body.events) {
        if (typeof event.sequence === 'number' && event.sequence > seen) {
          seen = event.sequence;
          render(event);
        }
      }
    }
  }

  const cancel = page.querySelector('[data-mj-cancel]');
  if (cancel && cancelRoute) {
    cancel.addEventListener('click', async () => {
      cancel.disabled = true;
      cancel.textContent = 'Asking it to stop…';
      const answer = await api(cancelRoute, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ id }),
      });
      if (!answer.ok) {
        cancel.disabled = false;
        cancel.textContent = 'Cancel this execution';
      }
    });
  }
}

function isFinalState(state) {
  return state === 'succeeded' || state === 'failed' || state === 'cancelled';
}

/** Append one log line, escaped; the server already removed control characters. */
function appendLog(log, frame) {
  const empty = log.querySelector('.mj-empty');
  if (empty) empty.remove();
  const line = document.createElement('div');
  line.className = 'mj-log-line';
  const time = document.createElement('span');
  time.className = 'mj-log-time';
  time.textContent = frame.timestamp;
  const stream = document.createElement('span');
  stream.className = 'mj-log-stream mj-status--' + (frame.data.stream || 'handler');
  stream.textContent = frame.data.stream || 'handler';
  const text = document.createElement('span');
  text.className = 'mj-log-text';
  // textContent, never innerHTML: the message came from a process's output
  text.textContent = frame.data.message;
  line.appendChild(time);
  line.appendChild(stream);
  line.appendChild(text);
  log.appendChild(line);
  // keep the page bounded however long the execution talks
  while (log.children.length > 500) log.removeChild(log.firstChild);
  log.scrollTop = log.scrollHeight;
}

function updateProgress(page, data) {
  const holder = page.querySelector('[data-mj-progress]');
  if (!holder || !data) return;
  const percent =
    data.total && data.total > 0
      ? Math.min(100, Math.round((data.current / data.total) * 100))
      : null;
  holder.textContent = '';
  if (percent !== null) {
    const bar = document.createElement('div');
    bar.className = 'mj-progress';
    bar.setAttribute('role', 'progressbar');
    bar.setAttribute('aria-valuenow', String(percent));
    bar.setAttribute('aria-valuemin', '0');
    bar.setAttribute('aria-valuemax', '100');
    bar.setAttribute('aria-label', 'execution progress');
    const fill = document.createElement('div');
    fill.className = 'mj-progress-bar';
    fill.style.width = percent + '%';
    const label = document.createElement('span');
    label.className = 'mj-progress-text';
    label.textContent = percent + '%';
    bar.appendChild(fill);
    bar.appendChild(label);
    holder.appendChild(bar);
  }
  if (data.message) {
    const note = document.createElement('p');
    note.className = 'mj-note';
    note.textContent = data.message;
    holder.appendChild(note);
  }
}

/** A step changed: the server renders the table, so this only marks that it moved. */
function touchState(page, frame) {
  const heading = page.querySelector('[data-mj-progress]');
  if (!heading) return;
  heading.setAttribute('data-mj-last-step', frame.data.name || '');
}

// ------------------------------------------------------------------ the list

function installListPage() {
  const live = document.querySelector('[data-mj-live]');
  if (!live || document.querySelector('[data-mj-execution]')) return;
  const indicator = connectionIndicator(live);
  const socketPath = live.dataset.mjSocket;
  if (!socketPath) return;
  let pending = null;
  follow(socketPath, {
    state: indicator,
    // the list is a table the server renders; when anything happens, read it again rather
    // than keeping a second copy of the same rows in this file
    message: () => {
      if (pending) return;
      pending = window.setTimeout(() => {
        pending = null;
        window.location.reload();
      }, 750);
    },
  });
}

// ------------------------------------------------------------------ the indicator

/** The connection state, shown as a word and a shape rather than as a colour alone. */
function connectionIndicator(root) {
  const holder = document.createElement('p');
  holder.className = 'mj-connection';
  holder.setAttribute('role', 'status');
  root.prepend(holder);
  return (state) => {
    const shape = state === LIVE ? '●' : state === RECONNECTING ? '◐' : '○';
    const word =
      state === LIVE ? 'Live' : state === RECONNECTING ? 'Reconnecting' : 'Not connected';
    holder.textContent = '';
    const mark = document.createElement('span');
    mark.className = 'mj-connection-mark mj-status--' + state;
    mark.setAttribute('aria-hidden', 'true');
    mark.textContent = shape;
    holder.appendChild(mark);
    holder.append(' ' + word);
    if (state === OFFLINE) {
      holder.append(' — this page shows what it last knew; reload to read it again.');
    }
  };
}

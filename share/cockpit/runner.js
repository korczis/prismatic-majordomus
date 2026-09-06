// The generic capability runner. One implementation for every capability: the form was
// generated from the input schema, each control carries the JSON type its property has,
// and this module turns the controls into the request the capability's own binding
// expects — a query string for a GET, a JSON body for a POST.
//
// It calls the capability's real route. There is no runner endpoint, no proxy and no
// second validation: what the Cockpit sends is what any other HTTP client would send, and
// what comes back is what any other client would get, error shape included.

import { api } from './cockpit.js';

const form = document.querySelector('[data-mj-runner]');
if (form) {
  install(form);
}

function install(form) {
  const method = form.dataset.mjMethod;
  const path = form.dataset.mjPath;
  const preview = form.querySelector('[data-mj-preview]');
  const output = form.querySelector('[data-mj-result]');
  const submit = form.querySelector('button[type="submit"]');

  /** The input object, from the controls, typed as the schema said. */
  function collect() {
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
        if (Number.isFinite(n)) input[name] = n;
        else input[name] = raw; // let the server say what is wrong with it
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

  /** The exact request, as text, so a person can read it before sending it. */
  function describe(input) {
    if (method === 'GET') {
      const query = new URLSearchParams();
      for (const [key, value] of Object.entries(input)) {
        query.set(key, typeof value === 'string' ? value : JSON.stringify(value));
      }
      const text = query.toString();
      return { line: 'GET ' + path + (text ? '?' + text : ''), url: path + (text ? '?' + text : ''), body: null };
    }
    const body = JSON.stringify(input);
    return { line: 'POST ' + path + ' ' + body, url: path, body };
  }

  function refresh() {
    if (preview) preview.textContent = describe(collect()).line;
  }

  form.addEventListener('input', refresh);
  form.addEventListener('change', refresh);
  refresh();

  // the "load into the runner" buttons beside the capability's own benchmark cases
  for (const button of document.querySelectorAll('[data-mj-fill]')) {
    button.addEventListener('click', () => {
      let example = {};
      try {
        example = JSON.parse(button.dataset.mjFill);
      } catch (e) {
        return;
      }
      for (const control of form.querySelectorAll('[data-mj-type]')) {
        const name = control.getAttribute('name');
        const value = example[name];
        if (control.dataset.mjType === 'boolean') {
          control.checked = value === true;
        } else if (value === undefined || value === null) {
          control.value = '';
        } else if (typeof value === 'object') {
          control.value = JSON.stringify(value, null, 2);
        } else {
          control.value = String(value);
        }
      }
      refresh();
      form.scrollIntoView({ block: 'nearest' });
    });
  }

  form.addEventListener('submit', async (event) => {
    event.preventDefault();
    const request = describe(collect());
    if (submit) submit.disabled = true;
    show(output, { pending: true, line: request.line });
    const started = performance.now();
    const answer = await api(request.url, {
      method,
      headers: request.body ? { 'Content-Type': 'application/json' } : undefined,
      body: request.body || undefined,
    });
    const elapsed = performance.now() - started;
    if (submit) submit.disabled = false;
    show(output, {
      line: request.line,
      status: answer.status,
      ok: answer.ok,
      elapsed,
      body: answer.body,
    });
  });
}

function show(container, state) {
  if (!container) return;
  container.textContent = '';

  const status = document.createElement('div');
  status.className = 'mj-runner-status';

  const badge = document.createElement('span');
  badge.className =
    'mj-badge mj-badge--' + (state.pending ? 'info' : state.ok ? 'ok' : 'fail');
  const dot = document.createElement('span');
  dot.className = 'mj-badge-dot';
  badge.appendChild(dot);
  badge.append(state.pending ? 'running' : String(state.status));
  status.appendChild(badge);

  const line = document.createElement('code');
  line.className = 'mj-mono';
  line.textContent = state.line;
  status.appendChild(line);

  if (state.elapsed !== undefined) {
    const took = document.createElement('span');
    took.textContent = state.elapsed.toFixed(1) + ' ms';
    status.appendChild(took);
  }
  container.appendChild(status);

  if (state.body !== undefined && state.body !== null) {
    const pre = document.createElement('pre');
    pre.className = 'mj-pre';
    const code = document.createElement('code');
    code.textContent = JSON.stringify(state.body, null, 2);
    pre.appendChild(code);
    container.appendChild(pre);
  }
}

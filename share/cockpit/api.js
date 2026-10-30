// The one way a browser surface asks this server for JSON: same origin, no credentials, and a
// typed failure rather than a throw. Its own module, free of side effects, so that a page
// that is not the Cockpit — the UI's intent workspace — can load it without the Cockpit's
// theme, design check and component registration (cockpit.js re-exports it).

/**
 * Ask this server for JSON. Same origin, no credentials, typed failure.
 *
 * A failure is an answer, never a throw: a caller reads `ok` and `status` and has nothing to
 * catch. A request the network lost answers `status: 0` with an `unreachable` error. One such
 * loss is no fault of the page: the server closes a keep-alive connection that idled past its
 * deadline, and a browser reusing it in that instant gets an empty response
 * (ERR_EMPTY_RESPONSE). An idempotent request, GET or HEAD, is therefore asked once more
 * before the loss is returned; a write is never repeated.
 */
export async function api(path, options) {
  const method = ((options && options.method) || 'GET').toUpperCase();
  const idempotent = method === 'GET' || method === 'HEAD';
  const init = {
    credentials: 'omit',
    ...options,
    headers: { Accept: 'application/json', ...(options && options.headers) },
  };
  let response;
  for (let attempt = 0; ; attempt += 1) {
    try {
      response = await fetch(path, init);
      break;
    } catch (e) {
      if (!idempotent || attempt > 0) {
        const message = String((e && e.message) || e);
        return { ok: false, status: 0, body: { error: { code: 'unreachable', message } } };
      }
    }
  }
  const text = await response.text();
  let body = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch (e) {
    body = { error: { code: 'unreadable', message: text.slice(0, 400) } };
  }
  return { ok: response.ok, status: response.status, body };
}

// The one way a browser surface asks this server for JSON: same origin, no credentials, and a
// typed failure rather than a throw. Its own module, free of side effects, so that a page
// that is not the Cockpit — the UI's intent workspace — can load it without the Cockpit's
// theme, design check and component registration (cockpit.js re-exports it).

/** Ask this server for JSON. Same origin, no credentials, typed failure. */
export async function api(path, options) {
  const response = await fetch(path, {
    credentials: 'omit',
    ...options,
    headers: { Accept: 'application/json', ...(options && options.headers) },
  });
  const text = await response.text();
  let body = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch (e) {
    body = { error: { code: 'unreadable', message: text.slice(0, 400) } };
  }
  return { ok: response.ok, status: response.status, body };
}

/**
 * The idempotency key a click starts its execution under (ADR 0225). It is kept on the
 * element until the server has answered, so a click repeated after a lost request is the same
 * start and never a second one; once an answer came, `spendKey` makes the next click new.
 */
export function idempotencyKey(element) {
  if (!element.dataset.mjKey) {
    element.dataset.mjKey =
      globalThis.crypto && typeof globalThis.crypto.randomUUID === 'function'
        ? globalThis.crypto.randomUUID()
        : Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 12);
  }
  return element.dataset.mjKey;
}

/** The server answered the start this element's key was for: the next click is a new start. */
export function spendKey(element) {
  delete element.dataset.mjKey;
}

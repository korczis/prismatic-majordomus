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

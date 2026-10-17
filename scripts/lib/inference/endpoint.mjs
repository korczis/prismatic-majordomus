// Which endpoints an observation may reach (ADR 0174): the loopback interface by default,
// a host elsewhere only when the caller admits it outright. Nothing here scans: every
// endpoint is one the catalogue declares, one its declared variable names, or one the
// caller listed.

/** Normalise an endpoint the way runtimes' own variables spell it: `host:port`, a bare
 * host, `0.0.0.0` (a listen address, reached on loopback), a trailing `/v1`. Returns the
 * origin as a URL string, or throws a TypeError naming what is wrong. */
export function normalise(raw) {
  let text = String(raw ?? '').trim();
  if (!text) throw new TypeError('an empty endpoint');
  if (!/^[a-z][a-z0-9+.-]*:\/\//i.test(text)) text = `http://${text}`;
  const url = new URL(text);
  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    throw new TypeError(`${url.protocol} is not an HTTP endpoint`);
  }
  if (url.username || url.password) throw new TypeError('an endpoint may not carry a credential');
  if (url.hostname === '0.0.0.0') url.hostname = '127.0.0.1';
  if (url.hostname === '[::]') url.hostname = '[::1]';
  return url.origin;
}

/** Whether an origin is on this machine's loopback interface. */
export function isLoopback(origin) {
  const host = new URL(origin).hostname;
  return host === 'localhost' || host === '[::1]' || /^127\.\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(host);
}

/**
 * The endpoints to observe, in the catalogue's vendor order then the caller's. `vendors`
 * is `models list` vendors; `extra` is `[{ vendor, endpoint }]` the caller named.
 * Returns `{ admitted: [{ vendor, adapter, endpoint, source }], refused: [{ vendor,
 * endpoint, reason }] }`; a refusal is reported, never silently dropped.
 */
export function plan(vendors, { env = {}, extra = [], allowRemote = false } = {}) {
  const admitted = [];
  const refused = [];
  const seen = new Set();
  const consider = (vendor, adapter, raw, source) => {
    let endpoint;
    try {
      endpoint = normalise(raw);
    } catch (e) {
      refused.push({ vendor, endpoint: String(raw), reason: e.message });
      return;
    }
    if (!isLoopback(endpoint) && !allowRemote) {
      refused.push({ vendor, endpoint, reason: 'not on the loopback interface; admit it with --allow-remote' });
      return;
    }
    const key = `${vendor} ${endpoint}`;
    if (seen.has(key)) return;
    seen.add(key);
    admitted.push({ vendor, adapter, endpoint, source });
  };
  const runtimes = vendors.filter((v) => v.inference === 'local' && v.runtime);
  for (const v of runtimes) {
    const named = v.runtime.endpoint_env && env[v.runtime.endpoint_env];
    if (named) consider(v.id, v.runtime.adapter, named, `$${v.runtime.endpoint_env}`);
    else if (v.runtime.endpoint) consider(v.id, v.runtime.adapter, v.runtime.endpoint, 'catalogue');
  }
  for (const { vendor, endpoint } of extra) {
    const v = runtimes.find((r) => r.id === vendor);
    if (!v) {
      refused.push({ vendor, endpoint, reason: 'not a local runtime the catalogue declares' });
      continue;
    }
    consider(v.id, v.runtime.adapter, endpoint, '--endpoint');
  }
  return { admitted, refused };
}

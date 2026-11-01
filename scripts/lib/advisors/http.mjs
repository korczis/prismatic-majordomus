// One HTTP exchange for the API and local-runtime adapters: a request under the caller's
// signal, its failure classified the way the contract requires.

import { AdvisorFailure, httpFailure } from './contract.mjs';

/** POST or GET JSON; resolves to the parsed body, rejects with a typed failure. */
export async function requestJson(url, { method = 'GET', headers = {}, body, io = {} } = {}) {
  const f = io.fetch ?? globalThis.fetch;
  let res;
  try {
    res = await f(url, {
      method,
      headers: { 'content-type': 'application/json', ...headers },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: io.signal,
    });
  } catch (e) {
    if (io.signal?.aborted) throw e;
    throw new AdvisorFailure('unavailable', `could not reach ${new URL(url).host}`);
  }
  if (!res.ok) {
    // the error body is read only to tell an exhausted quota from a passing rate limit;
    // a body that is not JSON is no reason to lose the status
    let errorBody;
    try { errorBody = await res.json(); } catch { errorBody = undefined; }
    throw httpFailure(res.status, res.headers, errorBody);
  }
  try {
    return await res.json();
  } catch {
    throw new AdvisorFailure('malformed', 'the response is not JSON');
  }
}

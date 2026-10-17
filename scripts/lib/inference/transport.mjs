// One bounded HTTP exchange with a local runtime, on the advisors' own `requestJson` and
// failure classes (ADR 0098), with a deadline of its own: a runtime that does not answer
// in time is `timeout`, a caller that gave up is `cancelled`, and neither hangs the run.

import { AdvisorFailure } from '../advisors/contract.mjs';
import { requestJson } from '../advisors/http.mjs';

/** The deadline of one discovery request, milliseconds: metadata, never inference. */
export const DISCOVERY_MS = 1500;
/** The deadline of one probe, milliseconds: a probe may load a model. */
export const PROBE_MS = 60000;

/** GET or POST JSON under a deadline; resolves to the body, rejects with an AdvisorFailure. */
export async function call(url, { method = 'GET', body, deadlineMs = DISCOVERY_MS } = {}, io = {}) {
  // A timer of our own rather than AbortSignal.timeout(), which is unref'd: a deadline
  // must keep the process alive until it fires.
  const deadline = new AbortController();
  const timer = setTimeout(() => deadline.abort(), deadlineMs);
  const signal = io.signal ? AbortSignal.any([io.signal, deadline.signal]) : deadline.signal;
  try {
    return await requestJson(url, { method, body, io: { ...io, signal } });
  } catch (e) {
    // The advisors' classes, in this layer's words: what failed is a runtime.
    if (e instanceof AdvisorFailure) {
      throw new AdvisorFailure(e.status, e.diagnostic.replace('the advisor', 'the runtime'), {
        retryAfterSeconds: e.retryAfterSeconds,
      });
    }
    if (io.signal?.aborted) throw new AdvisorFailure('cancelled', 'the caller cancelled');
    if (deadline.signal.aborted) throw new AdvisorFailure('timeout', `no answer within ${deadlineMs} ms`);
    throw new AdvisorFailure('error', 'the request failed');
  } finally {
    clearTimeout(timer);
  }
}

/** Time one probe: `{ outcome, duration_ms, diagnostic? }`, never a throw. */
export async function timed(fn, now = () => Date.now()) {
  const started = now();
  try {
    await fn();
    return { outcome: 'passed', duration_ms: now() - started };
  } catch (e) {
    return {
      outcome: 'failed',
      duration_ms: now() - started,
      diagnostic: e instanceof AdvisorFailure ? `${e.status}: ${e.diagnostic}` : 'the probe failed',
    };
  }
}

/** A probe's prompt: one word back, a handful of tokens at most. */
export const PROBE_PROMPT = 'Reply with the single word: ok';

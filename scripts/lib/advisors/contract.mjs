// The advisor transport contract (ADR 0098): what every adapter under scripts/lib/advisors/
// promises, and the normalisation that turns whatever an advisor said into the typed
// consultation `majordomus reasoning record` admits.
//
// An adapter module exports one function:
//
//   export async function exchange(advisor, prompt, io) -> { text, usage? }
//
// `advisor` is one entry of `reasoning advisors --format json` (id, adapter, transport,
// capabilities); `prompt` is the text this module built; `io` carries the injected effects
// (`fetch`, `spawn`, `env`, `signal`, `timeoutMs`) so that every adapter runs against
// fakes in the contract tests and nothing in CI reaches a live model. An adapter throws
// an AdvisorFailure with a status from STATUSES for every way an exchange can fail, and
// never anything that carries a credential: diagnostics are a class and a sentence.
//
// This file and driver.mjs are provider-independent: `reasoning check` refuses them
// naming any advisor of the catalogue. Everything that knows one lives in its adapter.

/** The consultation statuses, as `majordomus reasoning record` accepts them. */
export const STATUSES = [
  'completed', 'timeout', 'rate_limited', 'auth_failed', 'malformed', 'empty',
  'unavailable', 'cancelled', 'error',
];

/** A failed exchange, typed. */
export class AdvisorFailure extends Error {
  constructor(status, diagnostic, extra = {}) {
    super(diagnostic);
    if (!STATUSES.includes(status) || status === 'completed') {
      throw new TypeError(`not a failure status: ${status}`);
    }
    this.status = status;
    this.diagnostic = scrub(String(diagnostic)).slice(0, 300);
    this.retryAfterSeconds = extra.retryAfterSeconds;
  }
}

/** Classify an HTTP status the way every HTTP adapter must. */
export function httpFailure(status, headers) {
  if (status === 401 || status === 403) {
    return new AdvisorFailure('auth_failed', `the credential was refused (HTTP ${status})`);
  }
  if (status === 429) {
    const retry = Number(headers?.get?.('retry-after'));
    return new AdvisorFailure('rate_limited', 'rate limited (HTTP 429)', {
      retryAfterSeconds: Number.isFinite(retry) && retry > 0 ? Math.ceil(retry) : undefined,
    });
  }
  if (status === 404) return new AdvisorFailure('unavailable', `not found (HTTP ${status})`);
  if (status >= 500) return new AdvisorFailure('unavailable', `the advisor failed (HTTP ${status})`);
  return new AdvisorFailure('error', `unexpected HTTP ${status}`);
}

// Secret shapes removed from anything that leaves the machine or reaches a record. The
// crate redacts again before writing; this pass keeps them out of the question itself.
const SECRET_SHAPES = [
  /sk-ant-[A-Za-z0-9_-]{16,}/g,
  /sk-(?:proj-)?[A-Za-z0-9_-]{20,}/g,
  /gh[pousr]_[A-Za-z0-9]{20,}/g,
  /github_pat_[A-Za-z0-9_]{20,}/g,
  /AKIA[0-9A-Z]{16}/g,
  /xox[abpr]-[A-Za-z0-9-]{10,}/g,
  /-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----/g,
  /(Bearer\s+)[A-Za-z0-9._~+/-]{16,}=*/g,
];

/** Remove secret shapes from a text. */
export function scrub(text) {
  let out = String(text ?? '');
  for (const shape of SECRET_SHAPES) {
    out = out.replace(shape, (m, bearer) =>
      typeof bearer === 'string' && m.startsWith(bearer) ? `${bearer}[redacted]` : '[redacted]');
  }
  return out;
}

const ANSWER_FIELDS = `{"conclusion": "one or two sentences",
 "stance": "supports" | "opposes" | "alternative" | "inconclusive",
 "assumptions": ["hidden assumption the hypothesis makes", ...],
 "risks": ["failure mode", ...],
 "actions": ["recommended next step", ...],
 "falsifiers": ["observation that would prove the conclusion wrong", ...],
 "confidence": "low" | "medium" | "high"}`;

/**
 * The question an advisor is asked: the distilled evidence, the hypothesis marked as the
 * executor's, any prior external opinion marked as opinion and never as fact, and a
 * request for the strongest counter-argument rather than agreement.
 */
export function buildPrompt({ subject, question, hypothesis, facts = [], assumptions = [],
  evidence = [], priorOpinions = [] }) {
  const lines = [];
  lines.push('You are an independent reviewer of an engineering decision. You do not own the',
    'decision: the executing engineer does, and weighs your answer against the evidence.',
    'Do not agree for its own sake. Argue against the hypothesis if the evidence allows it.', '');
  lines.push(`Subject: ${subject}`);
  if (question) lines.push(`Question: ${question}`);
  lines.push('');
  if (facts.length) lines.push('Facts (verified locally):', ...facts.map((f) => `- ${f}`), '');
  if (evidence.length) {
    lines.push('Evidence (references):',
      ...evidence.map((e) => `- [${e.kind}] ${e.reference}${e.note ? ` — ${e.note}` : ''}`), '');
  }
  if (assumptions.length) lines.push('Assumptions (not verified):', ...assumptions.map((a) => `- ${a}`), '');
  if (hypothesis) lines.push(`The executor's current hypothesis (an opinion, not a fact): ${hypothesis}`, '');
  if (priorOpinions.length) {
    lines.push('Prior external opinions (other reviewers; opinions, not facts, and possibly wrong):',
      ...priorOpinions.map((o) => `- ${o}`), '');
  }
  lines.push('Answer with: the strongest argument against the hypothesis, the hidden assumptions,',
    'what observation would falsify your conclusion, and the smallest discriminating experiment.',
    `Reply with ONE JSON object and nothing else, of this shape:\n${ANSWER_FIELDS}`);
  return scrub(lines.join('\n'));
}

const STANCES = ['supports', 'opposes', 'alternative', 'inconclusive'];
const CONFIDENCES = ['low', 'medium', 'high'];

function strings(v) {
  return Array.isArray(v) ? v.filter((x) => typeof x === 'string' && x.trim()).map((x) => scrub(x.trim()).slice(0, 400)) : [];
}

/**
 * Normalise an advisor's raw text into the consultation fields. The first JSON object in
 * the text is the answer; a text with none is `malformed`, an empty one `empty`. Never
 * returns the raw text: what an advisor said beyond the normalised fields is not kept.
 */
export function normalise(text) {
  const raw = String(text ?? '').trim();
  if (!raw) throw new AdvisorFailure('empty', 'the advisor returned nothing');
  const start = raw.indexOf('{');
  const end = raw.lastIndexOf('}');
  if (start < 0 || end <= start) {
    throw new AdvisorFailure('malformed', 'the answer carries no JSON object');
  }
  let parsed;
  try {
    parsed = JSON.parse(raw.slice(start, end + 1));
  } catch {
    throw new AdvisorFailure('malformed', 'the answer is not valid JSON');
  }
  const conclusion = typeof parsed.conclusion === 'string' ? scrub(parsed.conclusion.trim()) : '';
  if (!conclusion) throw new AdvisorFailure('malformed', 'the answer names no conclusion');
  return {
    conclusion: conclusion.slice(0, 1000),
    stance: STANCES.includes(parsed.stance) ? parsed.stance : 'inconclusive',
    assumptions: strings(parsed.assumptions),
    risks: strings(parsed.risks),
    actions: strings(parsed.actions),
    falsifiers: strings(parsed.falsifiers),
    confidence: CONFIDENCES.includes(parsed.confidence) ? parsed.confidence : undefined,
  };
}

/**
 * Run one exchange under a deadline and a cancellation signal, and return the typed
 * consultation fields: completed with the normalised answer, or a failure status with
 * its diagnostic. Never throws for an advisor's failure — failure is a result.
 */
export async function consultOne(adapter, advisor, prompt, io = {}) {
  const timeoutMs = io.timeoutMs ?? 90_000;
  const started = Date.now();
  const controller = new AbortController();
  const onAbort = () => controller.abort(io.signal.reason);
  if (io.signal) {
    if (io.signal.aborted) controller.abort(io.signal.reason);
    else io.signal.addEventListener('abort', onAbort, { once: true });
  }
  let timer;
  const deadline = new Promise((_, reject) => {
    timer = setTimeout(() => {
      controller.abort(new AdvisorFailure('timeout', `no answer within ${timeoutMs} ms`));
      reject(new AdvisorFailure('timeout', `no answer within ${timeoutMs} ms`));
    }, timeoutMs);
  });
  const cancelled = new Promise((_, reject) => {
    controller.signal.addEventListener('abort', () => {
      if (io.signal?.aborted) reject(new AdvisorFailure('cancelled', 'the session withdrew the request'));
    }, { once: true });
    if (io.signal?.aborted) reject(new AdvisorFailure('cancelled', 'the session withdrew the request'));
  });
  // The race observes both; these keep a rejection that arrives after the race settled
  // from surfacing as an unhandled one.
  deadline.catch(() => {});
  cancelled.catch(() => {});
  const base = { advisor: advisor.id };
  try {
    const answer = await Promise.race([
      adapter.exchange(advisor, prompt, { ...io, signal: controller.signal, timeoutMs }),
      deadline,
      cancelled,
    ]);
    const fields = normalise(answer?.text);
    const out = { ...base, status: 'completed', ...fields, duration_ms: Date.now() - started };
    if (answer?.usage) out.usage = answer.usage;
    return out;
  } catch (e) {
    // A failure is recognised by its shape, not its class: an adapter loaded from another
    // directory imports its own copy of this module, and its typed failure is still typed.
    const typed = e && typeof e.status === 'string' && STATUSES.includes(e.status) && e.status !== 'completed';
    const failure = typed
      ? new AdvisorFailure(e.status, e.diagnostic ?? e.message ?? e.status, { retryAfterSeconds: e.retryAfterSeconds })
      : new AdvisorFailure(controller.signal.aborted && io.signal?.aborted ? 'cancelled' : 'error',
        e?.message ?? 'the adapter failed');
    const out = { ...base, status: failure.status, diagnostic: failure.diagnostic,
      duration_ms: Date.now() - started };
    if (failure.retryAfterSeconds) out.retry_after_seconds = failure.retryAfterSeconds;
    return out;
  } finally {
    clearTimeout(timer);
    io.signal?.removeEventListener?.('abort', onAbort);
  }
}

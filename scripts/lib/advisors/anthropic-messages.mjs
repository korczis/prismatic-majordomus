// Adapter: an Anthropic Messages API advisor. The model and the credential variable's name
// come from the catalogue (`reasoning advisors`); the value is read here, at the moment of
// the request, and goes nowhere but the x-api-key header. Which balance the call draws on
// (the plan's API credits or metered use) is the vendor's billing, not this adapter's.

import { AdvisorFailure } from './contract.mjs';
import { requestJson } from './http.mjs';

export async function exchange(advisor, prompt, io = {}) {
  const env = io.env ?? process.env;
  const key = advisor.credential ? env[advisor.credential] : undefined;
  if (!key) throw new AdvisorFailure('unavailable', 'the credential variable is not set');
  if (!advisor.model) throw new AdvisorFailure('error', 'the catalogue names no model for this advisor');
  const base = env.ANTHROPIC_BASE_URL || 'https://api.anthropic.com';
  const j = await requestJson(`${base}/v1/messages`, {
    method: 'POST',
    headers: {
      'x-api-key': key,
      'anthropic-version': '2023-06-01',
      // a declined request is re-run on a model the vendor routes it to, in the same call
      'anthropic-beta': 'server-side-fallback-2026-07-01',
    },
    body: {
      model: advisor.model,
      max_tokens: 16000,
      fallbacks: 'default',
      messages: [{ role: 'user', content: prompt }],
    },
    io,
  });
  if (j?.stop_reason === 'refusal') {
    const category = j.stop_details?.category ?? 'unstated';
    throw new AdvisorFailure('error', `the model declined to answer (refusal, category ${category})`);
  }
  if (!Array.isArray(j?.content)) throw new AdvisorFailure('malformed', 'the response carries no content');
  // thinking and fallback blocks carry no answer; the answer is the text blocks, in order
  const text = j.content.filter((b) => b?.type === 'text').map((b) => b.text ?? '').join('');
  const usage = j.usage ? { input_tokens: j.usage.input_tokens ?? 0, output_tokens: j.usage.output_tokens ?? 0 } : undefined;
  return { text, usage };
}

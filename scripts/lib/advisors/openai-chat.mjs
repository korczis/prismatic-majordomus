// Adapter: an OpenAI chat-completions API advisor. The model and the credential variable's
// name come from the catalogue (`reasoning advisors`); the value is read here, at the
// moment of the request, and goes nowhere but the Authorization header.

import { AdvisorFailure } from './contract.mjs';
import { requestJson } from './http.mjs';

export async function exchange(advisor, prompt, io = {}) {
  const env = io.env ?? process.env;
  const key = advisor.credential ? env[advisor.credential] : undefined;
  if (!key) throw new AdvisorFailure('unavailable', 'the credential variable is not set');
  if (!advisor.model) throw new AdvisorFailure('error', 'the catalogue names no model for this advisor');
  const base = env.OPENAI_BASE_URL || 'https://api.openai.com/v1';
  const j = await requestJson(`${base}/chat/completions`, {
    method: 'POST',
    headers: { authorization: `Bearer ${key}` },
    body: {
      model: advisor.model,
      temperature: 0.2,
      response_format: { type: 'json_object' },
      messages: [{ role: 'user', content: prompt }],
    },
    io,
  });
  const text = j?.choices?.[0]?.message?.content;
  if (typeof text !== 'string') throw new AdvisorFailure('malformed', 'the response carries no message');
  const usage = j.usage ? { input_tokens: j.usage.prompt_tokens ?? 0, output_tokens: j.usage.completion_tokens ?? 0 } : undefined;
  return { text, usage };
}

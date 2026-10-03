// Adapter: LM Studio's local OpenAI-compatible server. The model is LMSTUDIO_MODEL, else
// the first model the server lists: which model a local runtime serves is the machine's.

import { AdvisorFailure } from './contract.mjs';
import { requestJson } from './http.mjs';

export async function exchange(advisor, prompt, io = {}) {
  const env = io.env ?? process.env;
  const base = env.LMSTUDIO_BASE_URL || 'http://127.0.0.1:1234/v1';
  let model = env.LMSTUDIO_MODEL;
  if (!model) {
    const list = await requestJson(`${base}/models`, { io });
    model = list?.data?.[0]?.id;
    if (!model) throw new AdvisorFailure('unavailable', 'the local server lists no loaded model');
  }
  const j = await requestJson(`${base}/chat/completions`, {
    method: 'POST',
    body: { model, temperature: 0.2, messages: [{ role: 'user', content: prompt }] },
    io,
  });
  const text = j?.choices?.[0]?.message?.content;
  if (typeof text !== 'string') throw new AdvisorFailure('malformed', 'the response carries no message');
  const usage = j.usage ? { input_tokens: j.usage.prompt_tokens ?? 0, output_tokens: j.usage.completion_tokens ?? 0 } : undefined;
  return { text, usage };
}

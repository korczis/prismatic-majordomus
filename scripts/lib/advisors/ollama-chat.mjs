// Adapter: an Ollama runtime on this machine. The model is OLLAMA_MODEL, else the first
// model the runtime has pulled.

import { AdvisorFailure } from './contract.mjs';
import { requestJson } from './http.mjs';

export async function exchange(advisor, prompt, io = {}) {
  const env = io.env ?? process.env;
  const host = (env.OLLAMA_HOST || 'http://127.0.0.1:11434').replace(/\/$/, '');
  const base = host.startsWith('http') ? host : `http://${host}`;
  let model = env.OLLAMA_MODEL;
  if (!model) {
    const tags = await requestJson(`${base}/api/tags`, { io });
    model = tags?.models?.[0]?.name;
    if (!model) throw new AdvisorFailure('unavailable', 'the runtime has no model pulled');
  }
  const j = await requestJson(`${base}/api/chat`, {
    method: 'POST',
    body: { model, stream: false, format: 'json', messages: [{ role: 'user', content: prompt }] },
    io,
  });
  const text = j?.message?.content;
  if (typeof text !== 'string') throw new AdvisorFailure('malformed', 'the response carries no message');
  return { text, usage: { input_tokens: j.prompt_eval_count ?? 0, output_tokens: j.eval_count ?? 0 } };
}

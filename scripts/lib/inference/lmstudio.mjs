// Runtime adapter: LM Studio. Identified by its native `/api/v0/models`, whose entries
// carry a model `type` and a load `state` no plain OpenAI-compatible server reports; a
// server answering only `/v1/models` is compatible, not identified.

import { AdvisorFailure } from '../advisors/contract.mjs';
import { call, PROBE_MS, PROBE_PROMPT } from './transport.mjs';

/** LM Studio's model types, as the catalogue's capability words. */
const TYPES = { llm: ['text'], vlm: ['text', 'vision'], embeddings: ['embeddings'] };

export async function observe(endpoint, io = {}) {
  const native = await call(`${endpoint}/api/v0/models`, {}, io).catch((e) => {
    if (e.status === 'unavailable' && /HTTP 404/.test(e.diagnostic)) return undefined;
    throw e;
  });
  const entries = Array.isArray(native?.data) ? native.data : undefined;
  if (!entries || !entries.every((m) => typeof m?.type === 'string' && typeof m?.state === 'string')) {
    const compatible = await call(`${endpoint}/v1/models`, {}, io).catch(() => undefined);
    return {
      unidentified: compatible
        ? 'answers the OpenAI-compatible GET /v1/models but not GET /api/v0/models'
        : 'GET /api/v0/models did not answer with typed models',
    };
  }
  const models = entries.map((m) => {
    const model = {
      native_id: m.id,
      family: m.arch,
      quantization: m.quantization,
      format: m.compatibility_type,
      loaded: m.state === 'loaded',
      context_window: {},
      capabilities: [],
    };
    if (Number.isFinite(m.max_context_length)) model.context_window.advertised = m.max_context_length;
    if (Number.isFinite(m.loaded_context_length)) model.context_window.configured = m.loaded_context_length;
    for (const word of TYPES[m.type] ?? []) {
      model.capabilities.push({ word, evidence: 'advertised', source: `GET /api/v0/models type: ${m.type}` });
    }
    if (Array.isArray(m.capabilities) && m.capabilities.includes('tool_use')) {
      model.capabilities.push({ word: 'tools', evidence: 'advertised', source: 'GET /api/v0/models capabilities: tool_use' });
    }
    return model;
  });
  return { identified_by: 'GET /api/v0/models answered models with a type and a load state', models };
}

export function candidates(model) {
  return model.capabilities.map((c) => c.word).filter((w) => w === 'text' || w === 'embeddings');
}

export async function probe(endpoint, model, word, io = {}) {
  if (word === 'embeddings') {
    const j = await call(`${endpoint}/v1/embeddings`, {
      method: 'POST', body: { model: model.native_id, input: 'probe' }, deadlineMs: PROBE_MS,
    }, io);
    if (!(j?.data?.[0]?.embedding?.length > 0)) {
      throw new AdvisorFailure('malformed', 'the answer carries no embedding');
    }
    return;
  }
  const j = await call(`${endpoint}/v1/chat/completions`, {
    method: 'POST',
    body: { model: model.native_id, max_tokens: 4, messages: [{ role: 'user', content: PROBE_PROMPT }] },
    deadlineMs: PROBE_MS,
  }, io);
  if (typeof j?.choices?.[0]?.message?.content !== 'string') {
    throw new AdvisorFailure('malformed', 'the answer carries no message');
  }
}

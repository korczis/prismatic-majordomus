// Runtime adapter: llama.cpp's llama-server. Identified by `/props`, whose slot count and
// default generation settings belong to llama-server alone; the served model is the one
// `/v1/models` lists. What the model can do is inferred, never advertised: llama-server
// says which modalities it accepts, and only a probe says whether it completes or embeds.

import path from 'node:path';
import { AdvisorFailure } from '../advisors/contract.mjs';
import { call, PROBE_MS, PROBE_PROMPT } from './transport.mjs';

export async function observe(endpoint, io = {}) {
  const props = await call(`${endpoint}/props`, {}, io);
  const settings = props?.default_generation_settings;
  if (!Number.isFinite(props?.total_slots) || typeof settings !== 'object' || settings === null) {
    return { unidentified: 'GET /props did not answer with slots and generation settings' };
  }
  const listed = await call(`${endpoint}/v1/models`, {}, io).catch(() => undefined);
  const entries = Array.isArray(listed?.data) ? listed.data : [];
  const ctx = settings.n_ctx ?? props.n_ctx;
  const models = entries.filter((m) => typeof m?.id === 'string').map((m) => {
    const status = m.status?.value;
    const model = {
      // A single-model server lists the weights' path, which names the machine's home
      // directory; it ignores the request's model field, so the file name serves as well.
      native_id: m.id.startsWith('/') ? path.basename(m.id) : m.id,
      format: 'gguf',
      size_bytes: Number.isFinite(m.meta?.size) ? m.meta.size : undefined,
      parameters: Number.isFinite(m.meta?.n_params) ? String(m.meta.n_params) : undefined,
      loaded: typeof status === 'string' ? status === 'loaded' : true,
      context_window: {},
      capabilities: [
        { word: 'text', evidence: 'inferred', source: 'served by llama-server; GET /props reports no task' },
      ],
    };
    if (Number.isFinite(m.meta?.n_ctx_train)) model.context_window.advertised = m.meta.n_ctx_train;
    if (Number.isFinite(ctx)) model.context_window.configured = ctx;
    if (props.modalities?.vision === true) {
      model.capabilities.push({ word: 'vision', evidence: 'advertised', source: 'GET /props modalities.vision' });
    }
    return model;
  });
  const version = typeof props.build_info === 'string' ? props.build_info : undefined;
  return { version, identified_by: 'GET /props answered slots and generation settings', models };
}

/** llama-server serves completion or embeddings by its launch flags; a probe tries both. */
export function candidates() {
  return ['text', 'embeddings'];
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

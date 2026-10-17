// The observation driver (ADR 0174): reach every admitted endpoint concurrently through
// the adapter its catalogue vendor names, and hand back one typed observation per
// endpoint in the plan's own order — never in the order answers arrived, so the recorded
// document does not depend on which runtime was faster. Runtime-independent: everything
// that knows one lives in its adapter.

import { pathToFileURL } from 'node:url';
import path from 'node:path';
import { AdvisorFailure } from '../advisors/contract.mjs';
import { timed } from './transport.mjs';

const HERE = path.dirname(new URL(import.meta.url).pathname);

/** The observation document's format version, as `models observe` admits it. */
export const VERSION = 1;

/** Load a runtime adapter by its catalogue name. */
export async function loadAdapter(name, dir = HERE) {
  if (!/^[a-z0-9][a-z0-9-]*$/.test(name)) throw new AdvisorFailure('error', `not an adapter name: ${name}`);
  let mod;
  try {
    mod = await import(pathToFileURL(path.join(dir, `${name}.mjs`)).href);
  } catch {
    throw new AdvisorFailure('unavailable', `adapter ${name} could not be loaded`);
  }
  if (typeof mod.observe !== 'function') throw new AdvisorFailure('error', `adapter ${name} exports no observe()`);
  return mod;
}

function prune(value) {
  if (Array.isArray(value)) return value.map(prune);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value)
      .filter(([, v]) => v !== undefined && !(v && typeof v === 'object' && !Array.isArray(v) && !Object.keys(v).length))
      .map(([k, v]) => [k, prune(v)]));
  }
  return value;
}

async function observeOne({ vendor, adapter: name, endpoint }, { probe, adapterDir, io, now }) {
  const started = now();
  const base = { vendor, endpoint };
  let adapter;
  try {
    adapter = await loadAdapter(name, adapterDir);
    const seen = await adapter.observe(endpoint, io);
    if (seen.unidentified) {
      return { ...base, state: 'unidentified', diagnostic: seen.unidentified, duration_ms: now() - started, models: [] };
    }
    const models = [...seen.models].sort((a, b) => (a.native_id < b.native_id ? -1 : a.native_id > b.native_id ? 1 : 0));
    if (probe && typeof adapter.probe === 'function') {
      for (const model of models) {
        if (model.loaded === false && probe !== 'all') continue;
        const words = typeof adapter.candidates === 'function' ? adapter.candidates(model) : [];
        model.probes = [];
        for (const word of words) {
          model.probes.push({ word, ...(await timed(() => adapter.probe(endpoint, model, word, io), now)) });
        }
      }
    }
    return {
      ...base,
      state: 'identified',
      version: seen.version,
      identified_by: seen.identified_by,
      duration_ms: now() - started,
      models,
    };
  } catch (e) {
    const failure = e instanceof AdvisorFailure ? e : new AdvisorFailure('error', 'the observation failed');
    const state = failure.status === 'unavailable' || failure.status === 'timeout' ? 'unreachable' : 'failed';
    return { ...base, state, diagnostic: `${failure.status}: ${failure.diagnostic}`, duration_ms: now() - started, models: [] };
  }
}

/**
 * Observe what a plan admitted. `probe` is false (metadata only, nothing is loaded),
 * true (probe loaded models) or 'all' (probe every model, which may load each one).
 */
export async function observeAll({ admitted, refused = [], probe = false, adapterDir, io = {}, now = () => Date.now() }) {
  const runtimes = await Promise.all(admitted.map((e) => observeOne(e, { probe, adapterDir, io, now })));
  return prune({
    version: VERSION,
    observed_at: new Date(now()).toISOString(),
    probe: probe === 'all' ? 'all' : probe ? 'loaded' : 'none',
    runtimes,
    refused,
  });
}

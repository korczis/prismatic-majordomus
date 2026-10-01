// The consultation driver (ADR 0098): ask what a recorded plan selected, concurrently,
// and hand back one typed result per advisor in the plan's own order — never in the order
// answers happened to arrive, so that the records and the report derived from them do not
// depend on which network was faster.
//
// Provider-independent by construction: an advisor is reached through the adapter its
// catalogue entry names, loaded by that name from this directory (or from the directory a
// caller supplies, which is how the tests substitute fakes). `reasoning check` refuses this
// file naming any advisor of the catalogue.

import { pathToFileURL } from 'node:url';
import path from 'node:path';
import { consultOne, AdvisorFailure } from './contract.mjs';

const HERE = path.dirname(new URL(import.meta.url).pathname);

/** Load an adapter module by its catalogue name. */
export async function loadAdapter(name, dir = HERE) {
  if (!/^[a-z0-9][a-z0-9-]*$/.test(name)) throw new AdvisorFailure('error', `not an adapter name: ${name}`);
  const file = path.join(dir, `${name}.mjs`);
  try {
    const mod = await import(pathToFileURL(file).href);
    if (typeof mod.exchange !== 'function') {
      throw new AdvisorFailure('error', `adapter ${name} exports no exchange()`);
    }
    return mod;
  } catch (e) {
    if (e instanceof AdvisorFailure) throw e;
    throw new AdvisorFailure('unavailable', `adapter ${name} could not be loaded`);
  }
}

/**
 * Consult every advisor a plan selected. `advisors` is the advisor list of
 * `reasoning advisors`; `plan` is a recorded plan's `plan` field. Returns one result per
 * selected advisor, in selection order. An empty selection returns an empty list: the
 * session decides locally, and that is not an error.
 */
export async function consultSelected({ plan, advisors, prompt, adapterDir, io = {} }) {
  const selected = plan?.selected ?? [];
  const tasks = selected.map(async ({ advisor: id }) => {
    const advisor = advisors.find((a) => a.id === id);
    if (!advisor) {
      return { advisor: id, status: 'unavailable', diagnostic: 'the advisor is no longer declared', duration_ms: 0 };
    }
    let adapter;
    try {
      adapter = await loadAdapter(advisor.adapter, adapterDir);
    } catch (e) {
      return { advisor: id, status: e.status ?? 'error', diagnostic: e.diagnostic ?? String(e), duration_ms: 0 };
    }
    return consultOne(adapter, advisor, prompt, io);
  });
  // Concurrent, but the result order is the plan's.
  return Promise.all(tasks);
}

/** The step report of one consultation, for a terminal: machine fields rendered briefly. */
export function stepReport({ subject, evidenceCount, result, recordId }) {
  const lines = [
    '[reasoning] consultation',
    `  issue      ${subject}`,
    `  advisor    ${result.advisor}`,
    `  evidence   ${evidenceCount} reference(s)`,
  ];
  if (result.status === 'completed') {
    lines.push(`  conclusion ${result.conclusion}`,
      `  stance     ${result.stance}${result.confidence ? ` · confidence ${result.confidence}` : ''}`);
    if (result.actions?.length) lines.push(`  next       ${result.actions[0]}`);
  } else {
    lines.push(`  status     ${result.status}: ${result.diagnostic ?? ''}`,
      '  next       continue on local evidence');
  }
  if (recordId) lines.push(`  record     ${recordId}`);
  return lines.join('\n');
}

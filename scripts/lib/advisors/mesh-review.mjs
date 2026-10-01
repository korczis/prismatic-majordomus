// Adapter: a linked mesh runtime carrying the reviews feature (ADR 0067). The request is a
// mesh review request addressed to that runtime through this checkout's shared server;
// the answer is the first review answer it replicates back. A peer answers when a person
// or a session there does, so an unanswered request is a timeout, not a failure of the
// peer: the request stays in the journal and can still be answered later.

import { AdvisorFailure } from './contract.mjs';
import { requestJson } from './http.mjs';

const VERDICT = { approved: 'supports', changes_requested: 'opposes', commented: 'inconclusive' };

export async function exchange(advisor, prompt, io = {}) {
  const env = io.env ?? process.env;
  const server = env.MAJORDOMUS_SERVER_URL;
  if (!server) throw new AdvisorFailure('unavailable', 'no shared server URL (MAJORDOMUS_SERVER_URL)');
  const runtime = advisor.id.replace(/^peer:/, '');
  const written = await requestJson(`${server}/api/v1/mesh/reviews`, {
    method: 'POST',
    body: { subject: prompt.split('\n').find((l) => l.startsWith('Subject:'))?.slice(9) || 'reasoning review', reviewer: runtime },
    io,
  });
  const key = written?.key;
  if (!key) throw new AdvisorFailure('malformed', 'the review request returned no key');
  const every = Number(env.MAJORDOMUS_MESH_POLL_MS || 2000);
  for (;;) {
    if (io.signal?.aborted) throw new AdvisorFailure('cancelled', 'the session withdrew the request');
    const state = await requestJson(`${server}/api/v1/mesh/state`, { io });
    const review = (state?.state?.reviews ?? []).find((r) => r.key === key);
    const answer = review?.answers?.[0];
    if (answer) {
      return {
        text: JSON.stringify({
          conclusion: answer.note || answer.verdict,
          stance: VERDICT[answer.verdict] ?? 'inconclusive',
        }),
      };
    }
    await new Promise((r) => setTimeout(r, every));
  }
}

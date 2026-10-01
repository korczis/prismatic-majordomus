// A scripted fixture adapter for the end-to-end cases: MAJORDOMUS_FIXTURE_ANSWERS is a JSON
// object from advisor id to what it does — {"answer": {...}} answers with that object,
// {"fail": "<status>"} fails with that status, {"hang": true} never answers (a timeout),
// {"text": "..."} returns raw text (for malformed answers).
import { AdvisorFailure } from '../../../scripts/lib/advisors/contract.mjs';

export async function exchange(advisor, prompt, io = {}) {
  const env = io.env ?? process.env;
  const script = JSON.parse(env.MAJORDOMUS_FIXTURE_ANSWERS || '{}')[advisor.id] ?? {};
  if (script.hang) return new Promise(() => {});
  if (script.fail) throw new AdvisorFailure(script.fail, `scripted ${script.fail}`, { retryAfterSeconds: script.retryAfter });
  if (script.text !== undefined) return { text: script.text };
  return { text: JSON.stringify(script.answer ?? { conclusion: 'no objection', stance: 'supports' }), usage: { input_tokens: prompt.length, output_tokens: 10 } };
}

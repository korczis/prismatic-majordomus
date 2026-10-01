// Adapter: the Gemini CLI in non-interactive prompt mode; its answer is standard output.

import { AdvisorFailure } from './contract.mjs';
import { runProcess } from './process.mjs';

export async function exchange(advisor, prompt, io = {}) {
  if (!advisor.executable) throw new AdvisorFailure('error', 'the catalogue names no executable');
  // Non-interactive: the CLI refuses an untrusted directory unless told the workspace is
  // trusted; the prompt mode it runs in asks for no tool approvals.
  const env = { ...(io.env ?? process.env), GEMINI_CLI_TRUST_WORKSPACE: 'true' };
  const run = await runProcess(advisor.executable, ['-p', prompt], { ...io, env });
  return { text: run.stdout };
}

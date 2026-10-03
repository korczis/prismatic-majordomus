// Adapter: the Codex CLI, run non-interactively and read-only against the checkout; its
// final message is written to a file the adapter reads and removes.

import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { AdvisorFailure } from './contract.mjs';
import { runProcess } from './process.mjs';

export async function exchange(advisor, prompt, io = {}) {
  if (!advisor.executable) throw new AdvisorFailure('error', 'the catalogue names no executable');
  const dir = mkdtempSync(path.join(io.tmpdir ?? tmpdir(), 'advisor-'));
  const out = path.join(dir, 'answer.txt');
  try {
    const run = await runProcess(advisor.executable,
      ['exec', '--skip-git-repo-check', '-s', 'read-only', '-o', out, prompt], io);
    let text;
    try { text = readFileSync(out, 'utf8'); } catch { text = run.stdout; }
    return { text };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

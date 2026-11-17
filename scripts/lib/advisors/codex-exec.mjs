// Adapter: the Codex CLI, run non-interactively and read-only against the checkout; its
// final message is held to the answer's schema (`--output-schema`) and written to a file
// the adapter reads and removes.
//
// The file is the answer and nothing else is. stdout carries Codex's progress and events,
// and a JSON-looking fragment in them is not a final message: an exit 0 that wrote no file
// is `empty`, never an answer cut out of the log.

import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { ANSWER_SCHEMA, AdvisorFailure } from './contract.mjs';
import { runProcess } from './process.mjs';

export async function exchange(advisor, prompt, io = {}) {
  if (!advisor.executable) throw new AdvisorFailure('error', 'the catalogue names no executable');
  const dir = mkdtempSync(path.join(io.tmpdir ?? tmpdir(), 'advisor-'));
  const out = path.join(dir, 'answer.txt');
  const schema = path.join(dir, 'answer.schema.json');
  try {
    writeFileSync(schema, JSON.stringify(ANSWER_SCHEMA));
    await runProcess(advisor.executable,
      ['exec', '--skip-git-repo-check', '-s', 'read-only', '--output-schema', schema, '-o', out, prompt], io);
    let text;
    try {
      text = readFileSync(out, 'utf8');
    } catch {
      throw new AdvisorFailure('empty', 'codex exited 0 and wrote no final message');
    }
    return { text };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

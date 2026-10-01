// Running an advisor's client tool as a process, for the CLI adapters: the arguments, a
// deadline and a cancellation signal in; its output, or a typed failure, out. The
// process group is killed on abort so that no client outlives its consultation.

import { spawn as nodeSpawn } from 'node:child_process';
import { AdvisorFailure } from './contract.mjs';

const AUTH = /\b(401|403)\b|unauthori[sz]ed|not logged in|please log ?in|authentication|invalid api key/i;
const RATE = /\b429\b|rate.?limit|quota exceeded|too many requests/i;

/** Classify a failed run from its stderr. */
export function classifyExit(code, stderr) {
  // eslint-disable-next-line no-control-regex
  const plain = String(stderr ?? '').replace(/\x1b\[[0-9;]*m/g, '');
  const tail = plain.split('\n').filter(Boolean).slice(-1)[0] ?? '';
  if (AUTH.test(plain)) return new AdvisorFailure('auth_failed', 'the client reports an authentication failure');
  if (RATE.test(plain)) return new AdvisorFailure('rate_limited', 'the client reports a rate limit');
  return new AdvisorFailure('error', `the client exited ${code}: ${tail.slice(0, 160)}`);
}

/** Run `executable args`, resolving to its stdout. */
export function runProcess(executable, args, io = {}) {
  const spawn = io.spawn ?? nodeSpawn;
  return new Promise((resolve, reject) => {
    let child;
    try {
      child = spawn(executable, args, {
        cwd: io.cwd, env: io.env ?? process.env, stdio: ['ignore', 'pipe', 'pipe'], detached: true,
      });
    } catch {
      reject(new AdvisorFailure('unavailable', `${executable} could not be started`));
      return;
    }
    let out = '';
    let err = '';
    child.stdout?.on('data', (d) => { out += d; });
    child.stderr?.on('data', (d) => { err += d; });
    const kill = () => {
      try { process.kill(-child.pid, 'SIGTERM'); } catch { try { child.kill('SIGTERM'); } catch { /* gone */ } }
    };
    io.signal?.addEventListener('abort', kill, { once: true });
    child.on('error', (e) => {
      reject(new AdvisorFailure('unavailable', e.code === 'ENOENT'
        ? `${executable} is not installed` : `${executable} could not be started`));
    });
    child.on('close', (code) => {
      io.signal?.removeEventListener('abort', kill);
      if (io.signal?.aborted) return; // the caller already has its verdict
      if (code === 0) resolve({ stdout: out, stderr: err });
      else reject(classifyExit(code, err));
    });
  });
}

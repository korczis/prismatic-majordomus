// Running an advisor's client tool as a process, for the CLI adapters: the arguments, a
// deadline and a cancellation signal in; its output, or a typed failure, out. The
// process group is killed on abort so that no client outlives its consultation.

import { spawn as nodeSpawn } from 'node:child_process';
import { AdvisorFailure } from './contract.mjs';

const AUTH = /\b(401|403)\b|unauthori[sz]ed|not logged in|please log ?in|authentication|invalid api key/i;
const RATE = /\b429\b|rate.?limit|quota exceeded|too many requests/i;
// What no retry fixes, typed `auth_failed`, which the circuit keeps out longest, rather than
// `rate_limited` (it recovers after its retry-after) or `error` (transient): an account whose
// quota or credit is spent, and a client the account's tier no longer admits
// (`IneligibleTierError: This client is no longer supported ...`). Read before RATE, because
// such a client may also print the 429 it was answered with.
const EXHAUSTED = /insufficient_quota|credit_balance_exhausted|credit balance is too low/i;
const INELIGIBLE = /Ineligible\w*Error|no longer supported for|is not eligible for/i;

/** Classify a failed run from its stderr. */
export function classifyExit(code, stderr) {
  // eslint-disable-next-line no-control-regex
  const plain = String(stderr ?? '').replace(/\x1b\[[0-9;]*m/g, '');
  // The line that names the cause: a client's own `error:` line when it printed one, else
  // its last. An argument parser ends with usage and "try '--help'", which names nothing.
  const lines = plain.split('\n').map((l) => l.trim()).filter(Boolean);
  const tail = lines.find((l) => /^error:/i.test(l)) ?? lines.slice(-1)[0] ?? '';
  if (INELIGIBLE.test(plain)) {
    return new AdvisorFailure('auth_failed', "the client reports that the account's tier no longer admits it");
  }
  if (EXHAUSTED.test(plain)) {
    return new AdvisorFailure('auth_failed', "the client reports that the account's quota or credit is exhausted");
  }
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

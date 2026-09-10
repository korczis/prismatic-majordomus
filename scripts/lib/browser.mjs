// The one browser this repository drives.
//
// Two consumers already launched Chrome through Playwright — the UI audit and the Cockpit
// probe — each with its own inline `chromium.launch({ channel: 'chrome' })`. Deliberately
// the *system* Chrome and never a downloaded one, so a CI runner and a laptop use the same
// browser and neither pulls a second. That decision was made twice and written down
// nowhere, which is how the third consumer would have made it differently.
//
// The third consumer is the workspace transport (ADR 0025), and it needs something the
// other two must never have: an authenticated profile. So the two launches are separated
// here by what they carry rather than by who calls them.
//
//   launchEphemeral()  no profile, no state, nothing to leak — what the audits use.
//   launchProfile()    an operator's own signed-in profile, named outside this tree.
//
// The second is the whole security boundary of the transport, so its refusals are the
// point of this module and not an afterthought in a caller.

import { chromium } from 'playwright';
import { existsSync, mkdirSync, readFileSync, statSync } from 'node:fs';
import { homedir, platform } from 'node:os';
import { resolve, sep } from 'node:path';

/** Header names never recorded, in any direction, at any support level. */
const SECRET_HEADERS = new Set([
  'authorization',
  'proxy-authorization',
  'cookie',
  'set-cookie',
  'x-api-key',
  'x-auth-token',
  'openai-organization',
  'openai-sentinel-chat-requirements-token',
]);

/**
 * The browser profiles Chrome itself keeps, which this tool refuses to drive.
 *
 * Two independent reasons, and either alone is enough. Chrome 136 stopped honouring
 * `--remote-debugging-port` and `--remote-debugging-pipe` on the default user-data-dir —
 * a hardening measure against malware reading cookies and passwords out of a live profile
 * — so Playwright cannot attach there and a caller that tries gets a confusing failure
 * rather than a clear one. And driving the operator's everyday profile would put every
 * site they are signed into inside the blast radius of a transport that is supposed to
 * reach exactly one.
 */
function osChromeProfiles() {
  const home = homedir();
  switch (platform()) {
    case 'darwin':
      return [
        `${home}/Library/Application Support/Google/Chrome`,
        `${home}/Library/Application Support/Chromium`,
      ];
    case 'win32':
      return [
        `${process.env.LOCALAPPDATA || `${home}\\AppData\\Local`}\\Google\\Chrome\\User Data`,
      ];
    default:
      return [`${home}/.config/google-chrome`, `${home}/.config/chromium`];
  }
}

/** Is `child` the same path as `parent`, or inside it? */
function within(child, parent) {
  const a = resolve(child);
  const b = resolve(parent);
  return a === b || a.startsWith(b.endsWith(sep) ? b : b + sep);
}

/**
 * Where the operator's own configuration maps a profile label to a directory.
 *
 * Outside the repository, always: a path under the tree would be committed by the first
 * person who ran `git add -A`, and a signed-in Chrome profile is a credential in every
 * sense that matters even though it is not a string anyone would recognise as one.
 */
export function profilesFile() {
  if (process.env.MAJORDOMUS_BROWSER_PROFILES) return process.env.MAJORDOMUS_BROWSER_PROFILES;
  const base = process.env.XDG_CONFIG_HOME || `${homedir()}/.config`;
  return `${base}/majordomus/browser-profiles.json`;
}

/**
 * Resolve `<vendor>:<label>` to a directory, or explain precisely what is missing.
 *
 * The declaration names a label and never a path — `access.browser_profile: default` is a
 * word the operator's own configuration resolves. This is that resolution, and it refuses
 * every way of guessing: no default directory, no search, no creation of a mapping that
 * was not written down. A transport that guesses where a signed-in profile lives is a
 * transport that will one day find the wrong one.
 */
export function resolveProfileDir(key) {
  const file = profilesFile();
  if (!existsSync(file)) {
    throw new Error(
      `no browser profile configuration at ${file}\n` +
        `  A workspace declaration names a profile label; this file is what turns that label into a\n` +
        `  directory, and it lives outside the repository on purpose. Create it:\n\n` +
        `    mkdir -p "$(dirname "${file}")"\n` +
        `    printf '{\\n  "${key}": "%s/.local/share/majordomus/chrome/${key.split(':')[0]}"\\n}\\n' "$HOME" > "${file}"\n`,
    );
  }
  let map;
  try {
    map = JSON.parse(readFileSync(file, 'utf8'));
  } catch (error) {
    throw new Error(`${file} does not parse as JSON: ${error.message}`);
  }
  const dir = map[key];
  if (!dir) {
    const known = Object.keys(map).sort();
    throw new Error(
      `${file} maps no profile "${key}"` +
        (known.length ? `; it maps ${known.map((k) => `"${k}"`).join(', ')}` : ' and is empty'),
    );
  }
  if (typeof dir !== 'string' || !dir.startsWith('/')) {
    throw new Error(`${file} maps "${key}" to ${JSON.stringify(dir)}, which is not an absolute path`);
  }
  for (const os of osChromeProfiles()) {
    if (within(dir, os)) {
      throw new Error(
        `${file} maps "${key}" into Chrome's own profile directory (${os}).\n` +
          `  Chrome 136 and later refuse remote debugging on that directory, so this cannot work; and\n` +
          `  driving the everyday profile would expose every site the operator is signed into to a\n` +
          `  transport that is authorised to read one. Point the label at a directory of its own and\n` +
          `  sign in there once.`,
      );
    }
  }
  const repo = process.env.MAJORDOMUS_HOME;
  if (repo && within(dir, repo)) {
    throw new Error(`${file} maps "${key}" inside the repository (${dir}); a signed-in profile is never kept in the tree`);
  }
  return dir;
}

/**
 * The browser for work that carries no identity: the UI audit, the Cockpit probe.
 *
 * A fresh context every time, nothing persisted, nothing signed in. Callers close it.
 */
export function launchEphemeral(options = {}) {
  return chromium.launch({ channel: 'chrome', ...options });
}

/**
 * The browser for one authenticated workspace.
 *
 * `launchPersistentContext` rather than `launch` + `newContext`, because the point is the
 * session on disk: the operator signs in once, by hand, and every later run finds that
 * session already there. Nothing here ever types a credential, and this function will not
 * run against a directory it was not explicitly told to use.
 *
 * Returns the context; the caller closes it.
 */
export async function launchProfile({ profileKey, headless = false, create = false } = {}) {
  if (!profileKey) throw new Error('launchProfile needs a profile key; it has no default and will not search for one');
  const dir = resolveProfileDir(profileKey);
  if (!existsSync(dir)) {
    if (!create) {
      throw new Error(
        `the profile directory for "${profileKey}" does not exist yet: ${dir}\n` +
          `  Run the login step once, which creates it and opens a browser for you to sign in.`,
      );
    }
    mkdirSync(dir, { recursive: true });
  } else if (!statSync(dir).isDirectory()) {
    throw new Error(`the profile for "${profileKey}" is not a directory: ${dir}`);
  }
  return chromium.launchPersistentContext(dir, {
    channel: 'chrome',
    headless,
    viewport: null,
    args: ['--no-first-run', '--no-default-browser-check'],
  });
}

/** The origin of a URL, or null where it has none this tool understands. */
function originOf(url) {
  try {
    const u = new URL(url);
    return `${u.protocol}//${u.host}`;
  } catch {
    return null;
  }
}

/** Every header except the ones that are never written down. */
function redact(headers) {
  const out = {};
  for (const [name, value] of Object.entries(headers || {})) {
    const key = name.toLowerCase();
    if (SECRET_HEADERS.has(key)) continue;
    out[key] = value;
  }
  return out;
}

/**
 * Record what the authenticated session sees, and only within the declared origins.
 *
 * The origin test happens in the listener, before anything is read off the response. That
 * ordering is the guarantee: traffic to anything the declaration does not name is never
 * recorded, rather than recorded and filtered afterwards — which is the version that
 * leaves a copy in a heap dump, a log line, or a crash report on the way to being dropped.
 *
 * Bodies are read only for JSON responses, because a mapper works from structure and an
 * HTML page tells it nothing a rendered read would not tell it better.
 */
export function observe(context, origins, { onRecord } = {}) {
  const allowed = new Set(origins.map((o) => originOf(o)).filter(Boolean));
  if (!allowed.size) throw new Error('observe needs at least one origin; an empty scope records nothing and should not be started');
  const records = [];

  context.on('response', async (response) => {
    const url = response.url();
    const origin = originOf(url);
    if (!origin || !allowed.has(origin)) return; // never read, never held

    const type = (response.headers()['content-type'] || '').split(';')[0].trim();
    if (type !== 'application/json') return;

    let body = null;
    try {
      body = await response.json();
    } catch {
      return; // a body that is not the JSON it claims is an observation about the vendor, not data
    }
    const record = {
      url: url.split('?')[0], // a query string carries identifiers and sometimes tokens
      status: response.status(),
      method: response.request().method(),
      headers: redact(response.headers()),
      body,
      observed_at: new Date().toISOString(),
    };
    records.push(record);
    if (onRecord) onRecord(record);
  });

  return records;
}

export const _internals = { osChromeProfiles, within, redact, originOf, SECRET_HEADERS };

// The workspace transport, driven from the command line. `scripts/workspace` is the entry.
//
// Four verbs, and the split between them is the security story: `list` and `status` touch
// no browser, `login` opens one and types nothing, `observe` reads and never acts.

import { existsSync, mkdirSync, readdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { launchProfile, observe, profilesFile, resolveProfileDir } from './browser.mjs';
import { ROOT, declaration, makeRecord, origins, profileKey, storeDir } from './workspace.mjs';

const [verb, id] = process.argv.slice(2);

/** Every workspace declared under .ai/repo/workspaces/, by id. */
function declared() {
  const dir = resolve(ROOT, '.ai/repo/workspaces');
  if (!existsSync(dir)) return [];
  return readdirSync(dir)
    .filter((f) => f.endsWith('.yaml'))
    .map((f) => f.replace(/\.yaml$/, ''))
    .sort();
}

/** What the configuration says about one workspace's profile, without opening anything. */
function profileState(decl) {
  const key = profileKey(decl);
  try {
    const dir = resolveProfileDir(key);
    return { key, dir, state: existsSync(dir) ? 'ready' : 'unconfigured', detail: existsSync(dir) ? dir : `${dir} (not created yet)` };
  } catch (error) {
    return { key, dir: null, state: 'unmapped', detail: error.message };
  }
}

function requireId() {
  if (!id) {
    console.error(`workspace ${verb}: which workspace? declared: ${declared().join(', ') || '(none)'}`);
    process.exit(2);
  }
  return declaration(id);
}

/** The vendor's front door: where a person signs in. */
function signInOrigin(decl) {
  return origins(decl)[0];
}

async function cmdList() {
  const ids = declared();
  if (!ids.length) {
    console.log('no workspace is declared under .ai/repo/workspaces/');
    return;
  }
  for (const each of ids) {
    const decl = declaration(each);
    const profile = profileState(decl);
    const store = storeDir(each);
    const synced = existsSync(store) ? `${readdirSync(resolve(store, 'observations'), { withFileTypes: true }).length} observation(s)` : 'nothing synced';
    console.log(`${each}  ${decl.vendor.name}  profile=${profile.state}  ${synced}`);
  }
}

async function cmdStatus() {
  const decl = requireId();
  const profile = profileState(decl);
  console.log(`workspace     ${decl.id} — ${decl.title}`);
  console.log(`vendor        ${decl.vendor.name} (${decl.vendor.id})`);
  console.log(`upstream      ${decl.upstream.kind} ${decl.upstream.id}`);
  console.log(`authorised    read${decl.authorisation.write ? ' and write' : ' only'}, asserted by ${decl.authorisation.asserted_by} on ${decl.authorisation.asserted_at}`);
  console.log(`origins       ${origins(decl).join(', ')}`);
  console.log(`profile key   ${profile.key}`);
  console.log(`profile map   ${profilesFile()}`);
  console.log(`profile       ${profile.state} — ${profile.detail}`);
  const store = storeDir(decl.id);
  console.log(`store         ${existsSync(store) ? store : `${store} (nothing synced)`}`);
  if (profile.state !== 'ready') {
    console.log('');
    console.log(`next          scripts/workspace login ${decl.id}`);
  }
}

/**
 * Open the workspace's own profile and let the operator sign in.
 *
 * This function types nothing and reads nothing. It creates the directory, opens the
 * vendor's front door in it, and waits for the person to close the window. Signing in is
 * theirs to do: a tool that handled a password or a second factor would be a tool that had
 * to hold one, and this repository has no place to hold one on purpose.
 */
async function cmdLogin() {
  const decl = requireId();
  const key = profileKey(decl);
  const dir = resolveProfileDir(key); // throws with instructions when the map is missing
  const first = !existsSync(dir);
  console.log(`opening ${signInOrigin(decl)} in the profile "${key}"`);
  console.log(`  ${dir}${first ? ' (creating)' : ''}`);
  console.log('');
  console.log('  Sign in yourself in the window that opens, then close it. Nothing here types');
  console.log('  anything, and no credential is read, copied or stored by this repository.');
  const context = await launchProfile({ profileKey: key, create: true, headless: false });
  const page = context.pages()[0] || (await context.newPage());
  await page.goto(signInOrigin(decl), { waitUntil: 'domcontentloaded' }).catch(() => {});
  await new Promise((done) => context.on('close', done));
  console.log('');
  console.log(`browser closed; run: scripts/workspace observe ${decl.id}`);
}

/** Is this page the vendor, or the vendor's front door asking who you are? */
function atSignIn(url) {
  return /accounts\.google\.com|auth\.openai\.com|\/auth\/|\/login|signin/i.test(url);
}

/**
 * Wait for a person to finish signing in, then carry on in the same session.
 *
 * Two commands were one too many. `login` ended when the window closed, which is a poor
 * signal: a window closes when a sign-in is finished, when it is abandoned, and when a
 * redirect chain lands somewhere unexpected — and only one of those means the session is
 * there. So this polls for the answer to the actual question, and the browser it hands to
 * the observation is the one that just proved itself signed in.
 */
async function waitForSignIn(context, page, decl, { timeoutMs = 10 * 60_000 } = {}) {
  const origin = signInOrigin(decl);
  const deadline = Date.now() + timeoutMs;
  let told = false;
  for (;;) {
    if (!atSignIn(page.url())) {
      const url = page.url();
      if (url.startsWith(origin)) return true;
    }
    if (!told) {
      console.log('');
      console.log('  Sign in yourself in the window that opened. Nothing here types anything.');
      console.log('  When you are through, leave the window open — this carries on by itself.');
      told = true;
    }
    if (Date.now() > deadline) return false;
    await page.waitForTimeout(3_000);
  }
}

/**
 * Read the workspace once, through the signed-in session, within the declared origins.
 *
 * Navigation only. Nothing is clicked, nothing is submitted, and no request is issued that
 * the page would not have issued on its own — `authorisation.write` is false and this is
 * what that means in code rather than in prose.
 */
async function cmdObserve() {
  const decl = requireId();
  if (decl.authorisation.write) {
    console.error(`workspace ${decl.id} declares authorisation.write: true, and this transport only reads; refusing rather than acting under an authorisation it does not use`);
    process.exit(2);
  }
  const key = profileKey(decl);
  const scope = origins(decl);
  const context = await launchProfile({ profileKey: key, headless: false });
  const seen = observe(context, scope);

  const url = decl.upstream.url;
  const page = context.pages()[0] || (await context.newPage());
  console.log(`observing ${url}`);
  console.log(`  scope: ${scope.join(', ')} — nothing outside these origins is read`);
  let signedIn = true;
  try {
    await page.goto(url, { waitUntil: 'networkidle', timeout: 60_000 });
  } catch (error) {
    console.error(`  the page did not settle: ${error.message}`);
  }
  // A vendor that redirects to its own login is not an error; it is the answer to "is this
  // profile still signed in", and it must be reported as that rather than as an empty sync.
  if (/\/auth|\/login|signin/i.test(page.url())) signedIn = false;

  await page.waitForTimeout(3_000); // late XHRs the load event does not wait for
  const store = storeDir(decl.id);
  mkdirSync(resolve(store, 'observations'), { recursive: true });
  const stamp = new Date().toISOString().replace(/[-:]/g, '').replace(/\.\d+Z$/, 'Z');

  if (!signedIn) {
    console.error(`  the profile "${key}" is not signed in: ${page.url()}`);
    console.error(`  run: scripts/workspace login ${decl.id}`);
    await context.close();
    process.exit(3);
  }

  const file = resolve(store, 'observations', `${stamp}.json`);
  writeFileSync(file, `${JSON.stringify({ workspace: decl.id, url, observed_at: new Date().toISOString(), origins: scope, responses: seen }, null, 2)}\n`);

  // One canonical record for the project itself. The conversations under it are mapped by
  // I1307, from this evidence — a mapper written before an observation exists is a guess.
  const record = makeRecord({
    workspace: decl.id,
    vendor: decl.vendor.id,
    type: 'project',
    upstreamId: decl.upstream.id,
    transport: 'browser',
    acquisition: 'browser_observed',
    support: 'browser_observed',
    title: decl.title,
    body: { url, observed_responses: seen.length },
  });
  mkdirSync(resolve(store, 'records'), { recursive: true });
  writeFileSync(resolve(store, 'records', `${record.type}-${record.upstream_id}.json`), `${JSON.stringify(record, null, 2)}\n`);

  console.log(`  ${seen.length} JSON response(s) recorded, headers redacted`);
  console.log(`  ${file}`);
  await context.close();
}

const verbs = { list: cmdList, status: cmdStatus, login: cmdLogin, observe: cmdObserve };
const run = verbs[verb];
if (!run) {
  console.error(`workspace: unknown verb ${verb ? `"${verb}"` : ''}; one of ${Object.keys(verbs).join(', ')}`);
  process.exit(2);
}
try {
  await run();
} catch (error) {
  console.error(`workspace ${verb}: ${error.message}`);
  process.exit(1);
}

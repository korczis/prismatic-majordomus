// browser.mjs: the one way a check in this repository gets a browser, and the one way it serves the built
// site to it. Rule: project.every-link-and-control-is-tested.
//
// launch(): the installed Chrome first (`channel: 'chrome'`, which is what CI's runners carry and what
// scripts/lib/cockpit-probe.mjs drives), then CHROME_BIN, then Playwright's own Chromium. It returns null
// when none starts, and the caller decides what that means: a local run says SKIP; a CI run (CI=true) must
// refuse, because a behaviour nobody ran is not a behaviour that holds.
//
// serveSite(): every request to the site's published origin is answered from site/public, so the pages are
// the build under test with their absolute URLs intact; a request to any other origin is refused and
// reported, because the site loads nothing from elsewhere.
import { existsSync, readFileSync, statSync } from 'node:fs';
import { extname, join } from 'node:path';

const TYPES = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript', '.mjs': 'text/javascript',
  '.json': 'application/json', '.svg': 'image/svg+xml', '.png': 'image/png', '.ico': 'image/x-icon',
  '.xml': 'application/xml', '.woff2': 'font/woff2', '.txt': 'text/plain', '.webmanifest': 'application/manifest+json' };

export async function launch() {
  let chromium;
  try { ({ chromium } = await import('playwright')); } catch { return null; }
  const attempts = [{ channel: 'chrome' }];
  if (process.env.CHROME_BIN) attempts.push({ executablePath: process.env.CHROME_BIN });
  attempts.push({});
  for (const opts of attempts) {
    try { return await chromium.launch(opts); } catch { /* the next one */ }
  }
  return null;
}

export function siteOrigin(root) {
  const cfg = readFileSync(join(root, 'site', 'config.toml'), 'utf8');
  const m = cfg.match(/^base_url\s*=\s*"([^"]+)"/m);
  if (!m) throw new Error('site/config.toml declares no base_url');
  return new URL(m[1]).origin;
}

export async function serveSite(context, pub, origin, onForeign) {
  await context.route('**/*', (route) => {
    const u = new URL(route.request().url());
    if (u.origin !== origin) {
      if (!/^(data|blob|about):/.test(u.protocol)) onForeign(u.href);
      return route.abort();
    }
    let f = join(pub, decodeURIComponent(u.pathname));
    if (existsSync(f) && statSync(f).isDirectory()) f = join(f, 'index.html');
    if (!existsSync(f)) return route.fulfill({ status: 404, contentType: 'text/plain', body: 'not in site/public' });
    return route.fulfill({ status: 200, contentType: TYPES[extname(f)] || 'application/octet-stream', body: readFileSync(f) });
  });
}

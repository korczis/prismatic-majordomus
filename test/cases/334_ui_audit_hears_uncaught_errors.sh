# majordomus-covers: none
# The UI audit reports an exception no script catches. A browser raises one as a page error,
# not as a console message, and scripts/lib/ui-audit.mjs listened to the console alone: Mermaid
# threw "Unsupported color format" on every diagram page of majordomus.dev while every sweep
# came back clean. Proved against a real browser over two fixture pages this case serves — one
# that throws, one that does not — so the finding is shown to name the throw and only the throw.
#
# Needs playwright with a Chromium it can launch; without one the case says so and skips,
# because an audit it cannot run proves nothing either way.
. "$ROOT/test/lib.sh"
command -v node >/dev/null 2>&1 || { echo "    skip: no node"; exit 0; }
[ -d "$ROOT/node_modules/playwright" ] && [ -d "$ROOT/node_modules/axe-core" ] || {
  echo "    skip: playwright and axe-core are not installed (npm ci)"; exit 0; }

mkdir -p www/throws www/quiet
page() { printf '<!doctype html><html lang="en"><head><title>%s</title></head><body><nav><a href="/">home</a></nav><main><h1>%s</h1></main>%s</body></html>' "$1" "$1" "$2"; }
page throws '<script>setTimeout(function () { throw new Error("fixture colour parser gave up"); }, 0);</script>' > www/throws/index.html
page quiet '<script>console.log("an ordinary log line is not an error");</script>' > www/quiet/index.html

cat > audit.mjs <<JS
import http from 'node:http';
import { readFileSync } from 'node:fs';
const { chromium } = await import('$ROOT/node_modules/playwright/index.mjs');
const { auditPage } = await import('$ROOT/scripts/lib/ui-audit.mjs');
const server = http.createServer((req, res) => {
  try { res.writeHead(200, { 'content-type': 'text/html' }); res.end(readFileSync('www' + req.url + 'index.html')); }
  catch { res.writeHead(404); res.end(); }
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const origin = 'http://127.0.0.1:' + server.address().port;
let browser;
try { browser = await chromium.launch(); } catch { console.log('NO-BROWSER'); server.close(); process.exit(0); }
const page = await browser.newPage();
for (const route of ['/throws/', '/quiet/']) {
  const visit = await auditPage(page, origin, route, 1024);
  // the same page object is reused, as the audit reuses it: listeners must not accumulate
  for (const f of visit.findings.filter((x) => x.rule.startsWith('runtime.'))) console.log(route + ' ' + f.rule + ' ' + f.detail);
  console.log(route + ' runtime findings ' + visit.findings.filter((x) => x.rule.startsWith('runtime.')).length);
}
await browser.close(); server.close();
JS
node audit.mjs > audit.out 2>&1 || { cat audit.out; echo "    the audit fixture did not run"; exit 1; }
if grep -q '^NO-BROWSER$' audit.out; then echo "    skip: playwright cannot launch Chromium here (npx playwright install chromium)"; exit 0; fi
expect_grep '^/throws/ runtime.console-error uncaught: fixture colour parser gave up$' audit.out
expect_grep '^/throws/ runtime findings 1$' audit.out
expect_grep '^/quiet/ runtime findings 0$' audit.out

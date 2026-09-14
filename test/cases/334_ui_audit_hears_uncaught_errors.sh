# majordomus-covers: none
# The UI audit reports an exception no script catches. A browser raises one as a page error,
# not as a console message, and scripts/lib/ui-audit.mjs listened to the console alone: Mermaid
# threw "Unsupported color format" on every diagram page of majordomus.dev while every sweep
# came back clean. Proved against a real browser over two fixture pages this case serves — one
# that throws, one that does not — so the finding is shown to name the throw and only the throw.
#
# Needs a browser scripts/lib/browser.mjs can launch (the installed Chrome, which CI's runners carry, or
# Playwright's Chromium). Without one it skips locally and fails under CI=true.
. "$ROOT/test/lib.sh"
command -v node >/dev/null 2>&1 || { echo "    skip: no node"; exit 0; }
[ -d "$ROOT/node_modules/playwright" ] && [ -d "$ROOT/node_modules/axe-core" ] || {
  echo "    skip: playwright and axe-core are not installed (npm ci)"; exit 0; }

mkdir -p www/throws www/quiet
# an empty icon, so the browser asks for no /favicon.ico: that request's 404 is a console error of its own, and
# whether it lands inside the audit's window depends on timing, not on the listener this case is about
page() { printf '<!doctype html><html lang="en"><head><title>%s</title><link rel="icon" href="data:,"></head><body><nav><a href="/">home</a></nav><main><h1>%s</h1></main>%s</body></html>' "$1" "$1" "$2"; }
# the throw happens while the page parses, before "load": a throw deferred with setTimeout can land after a fast
# audit has finished collecting, and then the case measures the runner's speed rather than the listener
page throws '<script>throw new Error("fixture colour parser gave up");</script>' > www/throws/index.html
page quiet '<script>console.log("an ordinary log line is not an error");</script>' > www/quiet/index.html

cat > audit.mjs <<JS
import http from 'node:http';
import { readFileSync } from 'node:fs';
const { launch } = await import('$ROOT/scripts/lib/browser.mjs');
const { auditPage } = await import('$ROOT/scripts/lib/ui-audit.mjs');
// the body is read before any header is written: a request for a file the fixture lacks (a browser asks for
// /favicon.ico) answers 404 once, instead of writing a second header after the 200
const server = http.createServer((req, res) => {
  let body;
  try { body = readFileSync('www' + req.url + 'index.html'); } catch { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { 'content-type': 'text/html' }); res.end(body);
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const origin = 'http://127.0.0.1:' + server.address().port;
// the installed Chrome first, as CI's runners carry it; Playwright's own Chromium after (scripts/lib/browser.mjs)
const browser = await launch();
if (!browser) { console.log('NO-BROWSER'); server.close(); process.exit(0); }
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
if grep -q '^NO-BROWSER$' audit.out; then
  # a behaviour nobody ran is not a behaviour that holds: under CI a missing browser is a failure
  [ "${CI:-}" = true ] && { echo "    no browser could be started under CI"; exit 1; }
  echo "    skip: no browser could be started (install Chrome, or: npx playwright install chromium)"; exit 0
fi
# on a failure the case shows what the audit saw, so a runner's answer is read rather than guessed at
grep -q '^/throws/ runtime findings 1$' audit.out && grep -q '^/quiet/ runtime findings 0$' audit.out || { echo "    the audit reported:"; sed 's/^/      /' audit.out; }
expect_grep '^/throws/ runtime.console-error uncaught: fixture colour parser gave up$' audit.out
expect_grep '^/throws/ runtime findings 1$' audit.out
expect_grep '^/quiet/ runtime findings 0$' audit.out

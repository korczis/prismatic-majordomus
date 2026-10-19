# majordomus-covers: none
# majordomus-skip-runs-in: ui
# The main navigation of a translated page is measured against the source language's bar on the
# same page and at the same width, in a real browser (I3600, scripts/lib/ui-navbar-locale.mjs).
# On 2026-10-19 the Czech bar of majordomus.dev took two rows at 1280px and 768px while every link
# and control in it still worked, so every check that presses and follows was green. This case
# holds the measurement that would have refused it, on a fixture served from memory:
#
#   wraps     a Czech bar whose labels are too long for the row it is laid out in is
#             navbar.wraps and navbar.locale-drift, and the findings land on the Czech visit
#   source    the English bar it is compared with has no finding: it is the reference
#   fixed     the same Czech bar with labels that fit is clean
#   clipped   an item that cuts its own text off is navbar.clipped
#   opened    at a width that shows the menu button, the opened Czech menu taller than the
#             opened English one is navbar.locale-drift for the opened state
#   restored  measuring leaves the page in the theme it found it in, with the menu closed
#
# Needs a browser scripts/lib/browser.mjs can launch. The suite runners have none, so the case
# defers to the ui job (majordomus-skip-runs-in: ui), which runs it with
# `--no-skips --runs-in ui`, where a skip is a failure.
. "$ROOT/test/lib.sh"
if [ -n "${CI:-}" ] && [ "${MAJORDOMUS_RUNS_IN:-}" != ui ]; then
  skip "the translated bar is measured in the ui job (--runs-in ui), not in this CI run"
fi
command -v node >/dev/null 2>&1 || skip "node is absent, so the translated bar was not measured"
[ -d "$ROOT/node_modules/playwright" ] \
  || skip "playwright is not installed (npm ci), so the translated bar was not measured"

cat > measure.mjs <<'EOF'
const [browserLib, localeLib] = process.argv.slice(2);
const { launch } = await import(browserLib);
const { applyLocaleDrift, measureNavbar } = await import(localeLib);

// one bar, a row of items in a box narrower than the viewport, and a menu button that shows
// below 700px and opens the list as a column
const page = (lang, labels, { clipped = false } = {}) => `<!doctype html><html lang="${lang}"><head>
<meta charset="utf-8"><title>${lang}</title><style>
  body { margin: 0; font: 16px/1.5 sans-serif; }
  nav { max-width: 480px; }
  nav ul { display: flex; flex-direction: row; flex-wrap: wrap; margin: 0; padding: 0; list-style: none; }
  nav li a { display: block; padding: 8px 12px; white-space: nowrap; }
  nav li a.clip { width: 40px; overflow: hidden; }
  nav button { display: none; }
  @media (max-width: 699px) {
    nav button { display: block; }
    nav ul { flex-direction: column; }
    nav ul.hidden { display: none; }
  }
</style></head><body>
<nav aria-label="Main">
  <button type="button" data-collapse-toggle="menu" aria-controls="menu" aria-expanded="false">Menu</button>
  <ul id="menu" class="hidden">${labels.map((l, i) => `<li><a href="#" class="${clipped && i === 0 ? 'clip' : ''}">${l}</a></li>`).join('')}</ul>
</nav>
<main><h1>${lang}</h1></main>
<script>
  const b = document.querySelector('[data-collapse-toggle]');
  b.addEventListener('click', () => {
    const open = document.getElementById('menu').classList.toggle('hidden') === false;
    b.setAttribute('aria-expanded', String(open));
  });
</script></body></html>`;

const EN = ['Docs', 'Guarantees', 'Cockpit', 'Releases'];
const CS_LONG = ['Dokumentace k produktu', 'Záruky a jejich důkazy', 'Řídicí panel', 'Vydání a změny'];
const CS_SHORT = ['Dokumenty', 'Záruky', 'Panel', 'Vydání'];
const CS_MANY = [...CS_SHORT, 'Další', 'Ještě', 'A další'];

const browser = await launch();
if (!browser) { console.log('NO-BROWSER'); process.exit(4); }
const run = async (pages, width) => {
  const context = await browser.newContext();
  const tab = await context.newPage();
  await tab.route('http://fixture.invalid/**', (route) => {
    const path = new URL(route.request().url()).pathname;
    const html = pages[path];
    return html ? route.fulfill({ status: 200, contentType: 'text/html', body: html }) : route.fulfill({ status: 404, body: '' });
  });
  await tab.setViewportSize({ width, height: 900 });
  const visits = [];
  for (const path of Object.keys(pages)) {
    await tab.goto(`http://fixture.invalid${path}`, { waitUntil: 'load' });
    const bar = await measureNavbar(tab, path);
    const state = await tab.evaluate(() => ({
      dark: document.documentElement.classList.contains('dark'),
      menuHidden: document.getElementById('menu').classList.contains('hidden'),
    }));
    visits.push({ route: path, width, findings: bar.findings, navbar: bar.navbar, state });
  }
  await context.close();
  applyLocaleDrift(visits);
  return visits;
};
const rules = (v) => v.findings.map((f) => f.rule).sort();

const out = {};
const wrap = await run({ '/': page('en', EN), '/cs/': page('cs', CS_LONG) }, 1280);
out.wraps = rules(wrap[1]);
out.wrapsDetail = wrap[1].findings.map((f) => f.detail);
out.source = rules(wrap[0]);
const fixed = await run({ '/': page('en', EN), '/cs/': page('cs', CS_SHORT) }, 1280);
out.fixed = [...rules(fixed[0]), ...rules(fixed[1])];
const clip = await run({ '/': page('en', EN), '/cs/': page('cs', CS_SHORT, { clipped: true }) }, 1280);
out.clipped = rules(clip[1]);
const opened = await run({ '/': page('en', EN), '/cs/': page('cs', CS_MANY) }, 390);
out.opened = opened[1].findings.map((f) => f.detail).filter((d) => d.includes('opened'));
out.restored = [...wrap, ...opened].every((v) => v.state.dark === false && v.state.menuHidden === true);
await browser.close();
console.log(JSON.stringify(out));
EOF

rc=0
node measure.mjs "$ROOT/scripts/lib/browser.mjs" "$ROOT/scripts/lib/ui-navbar-locale.mjs" > measure.out 2>&1 || rc=$?
if [ "$rc" -eq 4 ] && grep -q NO-BROWSER measure.out; then
  skip "no browser could be launched, so the translated bar was not measured"
fi
[ "$rc" -eq 0 ] || { echo "    the measurement failed (exit $rc):"; sed 's/^/    | /' measure.out; exit 1; }
R="$(tail -1 measure.out)"
echo "    $R"

expect_json() { # $1 jq filter that must be true, $2 what it proves
  printf '%s' "$R" | jq -e "$1" >/dev/null || { echo "    FAIL: $2"; echo "    $R"; exit 1; }
  echo "    ok: $2"
}
expect_json '.wraps | index("navbar.wraps") != null' 'a Czech bar too long for its row is navbar.wraps'
expect_json '.wraps | index("navbar.locale-drift") != null' 'and navbar.locale-drift against the English bar'
expect_json '[.wrapsDetail[] | select(test("tall in cs and [0-9]+px in en on / at 1280px"))] | length >= 1' 'the drift names both heights, the page and the width'
expect_json '.source == []' 'the English bar is the reference and carries no finding'
expect_json '.fixed == []' 'the same Czech bar with labels that fit is clean'
expect_json '.clipped | index("navbar.clipped") != null' 'an item that cuts its own text off is navbar.clipped'
expect_json '.opened | length >= 1' 'an opened Czech menu taller than the English one is drift for the opened state'
expect_json '.restored == true' 'measuring leaves the theme as it was and the menu closed'

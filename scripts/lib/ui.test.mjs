// The UI tooling's own tests: the discovery, the static scanner, the build's normalisations
// and the contrast arithmetic. A built site is a fixture of files and a theme is a fixture of
// CSS, so every claim here is made against one rather than against the repository's own.
// Run by `scripts/ui test`; the behaviour end to end is test/cases/85_ui_conformance.sh.
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import assert from 'node:assert/strict';

import { breakpointsFromCss, commonPrefix, criticalWidths, discoverPages, tierPages, viewports, REFLOW_FLOOR } from './ui-discover.mjs';
import { nameTaskCheckboxes, normalise, normaliseRendered, scan, scrollingTags, textOf } from './ui-static.mjs';
import { contrast, enforce, measure, raise, MINIMUM } from './ui-contrast.mjs';

const tests = [];
const test = (name, fn) => tests.push([name, fn]);

// ------------------------------------------------------------------ the static scanner
test('a scrolling box a keyboard cannot reach is a failure, and one it can is not', () => {
  assert.equal(scan('<div class="overflow-x-auto"><table></table></div>').length, 1);
  assert.equal(scan('<div class="overflow-x-auto" tabindex="0"></div>').length, 0);
  assert.equal(scan('<a class="overflow-x-auto">x</a>').length, 0, 'a link is already focusable');
});

test('which elements the theme makes scroll is read from the stylesheet, in either dialect', () => {
  assert.deepEqual(
    scrollingTags('.format :where(pre):not(:where([class~=not-format] *)) { overflow-x: auto }'),
    ['pre'],
  );
  assert.deepEqual(scrollingTags('.x pre, .y table { overflow-x: scroll }'), ['pre', 'table']);
  assert.deepEqual(scrollingTags('.card { overflow-y: auto }'), [], 'the other axis is not this');
});

test('a document carrying its own stylesheet is judged by that stylesheet', () => {
  const page = '<style>.scroll { overflow-x: auto }</style><div class="scroll" tabindex="0"></div><pre></pre>';
  assert.deepEqual(scan(page, { scrollingTagNames: ['pre'] }), [],
    'the theme of some other page does not apply here');
});

test('normalising is idempotent and leaves a declared tabindex alone', () => {
  const once = normalise('<pre>a</pre><table></table><pre tabindex="-1"></pre>', { scrollingTagNames: ['pre', 'table'] });
  assert.equal(once.changed, 2);
  assert.match(once.text, /<pre tabindex="0">a<\/pre>/);
  assert.equal(normalise(once.text, { scrollingTagNames: ['pre', 'table'] }).changed, 0);
});

test('a task checkbox is named from the text beside it, and its tag stays a tag', () => {
  const r = nameTaskCheckboxes('<li><input disabled="" type="checkbox" checked=""/>ship it');
  assert.match(r.text, /aria-label="ship it — done"/);
  assert.doesNotMatch(r.text, /checkbox"\/ /, 'the self-closing slash is not an attribute');
  assert.equal(nameTaskCheckboxes(r.text).changed, 0);
});

test('a checkbox inside a label is left alone, and an arrow function is not a tag boundary', () => {
  const label = '<label><input type="checkbox" checked x-on:change="off.filter(x => x !== 5)" class="h">'
    + '<span>met</span></label>';
  assert.equal(nameTaskCheckboxes(label).changed, 0, 'a labelled control already has a name');
  assert.equal(nameTaskCheckboxes(label).text, label);
});

test('a normalisation that changed what the page says is refused, not written', () => {
  const page = '<li><input type="checkbox">do the thing<pre>code</pre>';
  const r = normaliseRendered(page, { scrollingTagNames: ['pre'] });
  assert.equal(textOf(r.text), textOf(page), 'the reader sees the same words');
  assert.equal(r.scrolling, 1);
  assert.equal(r.named, 1);
});

// ------------------------------------------------------------------ contrast
test('the contrast ratio is the one WCAG defines', () => {
  assert.equal(Math.round(contrast('#000', '#fff')), 21);
  assert.equal(Math.round(contrast('#fff', '#fff')), 1);
});

test('a colour is darkened only as far as the threshold, and no further', () => {
  const raised = raise('#22A4E6', '#F8F9FA');
  assert.ok(contrast(raised, '#F8F9FA') >= MINIMUM, raised);
  assert.ok(contrast(raised, '#F8F9FA') < MINIMUM + 0.1, `${raised} overshot`);
  assert.equal(raise('#111111', '#F8F9FA'), '#111111', 'a colour that passes is untouched');
});

test('a palette is raised to the threshold, keeps its ground, and settles in one pass', () => {
  const css = '.z-l-code { color: #5C6166; background-color: #F8F9FA; }\n.z-l-1 { color: #22A4E6; }\n';
  const first = enforce(css);
  assert.equal(first.background, '#F8F9FA');
  assert.match(first.css, /background-color: #F8F9FA/, 'the reference is never moved');
  assert.equal(measure(first.css).colours.filter((c) => c.ratio < MINIMUM).length, 0);
  assert.equal(enforce(first.css).changed, 0);
});

function fixture() {
  const root = mkdtempSync(join(tmpdir(), 'mj-ui-'));
  const write = (rel, body) => {
    const path = join(root, rel);
    mkdirSync(join(path, '..'), { recursive: true });
    writeFileSync(path, body);
  };
  return { root, write };
}

test('a directory with an index and a stray html file are both routes', () => {
  const { root, write } = fixture();
  write('index.html', '<h1>home</h1>');
  write('docs/index.html', '<h1>docs</h1>');
  write('docs/cli/index.html', '<h1>cli</h1>');
  write('render-test.html', '<h1>x</h1>');
  const routes = discoverPages(root).map((p) => p.route);
  assert.deepEqual(routes, ['/', '/docs/', '/docs/cli/', '/render-test.html']);
  rmSync(root, { recursive: true, force: true });
});

test('a page the sitemap names but the build did not render is reported, not dropped', () => {
  const { root, write } = fixture();
  write('index.html', '<h1>home</h1>');
  write('sitemap.xml', '<urlset><url><loc>https://x.test/</loc></url><url><loc>https://x.test/gone/</loc></url></urlset>');
  const pages = discoverPages(root);
  const gone = pages.find((p) => p.route === '/gone/');
  assert.ok(gone, 'the sitemap route is in the target set');
  assert.equal(gone.rendered, false);
  assert.equal(gone.published, true);
  const home = pages.find((p) => p.route === '/');
  assert.equal(home.rendered && home.published, true);
  rmSync(root, { recursive: true, force: true });
});

test('an orphan the sitemap does not name is still audited', () => {
  const { root, write } = fixture();
  write('index.html', '<h1>home</h1>');
  write('orphan/index.html', '<h1>orphan</h1>');
  write('sitemap.xml', '<urlset><url><loc>https://x.test/</loc></url></urlset>');
  const orphan = discoverPages(root).find((p) => p.route === '/orphan/');
  assert.ok(orphan, 'the orphan is audited');
  assert.equal(orphan.published, false);
  rmSync(root, { recursive: true, force: true });
});

test('breakpoints come from the emitted media queries, in pixels, deduplicated', () => {
  const { root, write } = fixture();
  write('app.css', '@media (min-width:40rem){.a{}}@media (min-width:640px){.b{}}@media (min-width:64rem){.c{}}');
  // 40rem and 640px are the same width: one breakpoint, not two
  assert.deepEqual(breakpointsFromCss(join(root, 'app.css')), [640, 1024]);
  rmSync(root, { recursive: true, force: true });
});

test('a theme that adds a breakpoint adds a viewport, without this file changing', () => {
  const before = viewports([640, 1024]);
  const after = viewports([640, 900, 1024]);
  assert.ok(after.includes(900) && after.includes(899), 'the new boundary and the pixel below it');
  assert.ok(before.every((w) => after.includes(w)), 'and nothing already audited is lost');
});

test('every plan starts at the reflow floor and never below it', () => {
  const widths = viewports([320, 640]);
  assert.equal(widths[0], REFLOW_FLOOR);
  assert.ok(widths.every((w) => w >= REFLOW_FLOOR), 'a viewport under the floor is not audited');
});

test('the widths are sorted, unique, and hold each boundary with its neighbour below', () => {
  const widths = viewports([640, 768]);
  assert.deepEqual(widths, [...new Set(widths)].sort((a, b) => a - b));
  for (const b of [640, 768]) {
    assert.ok(widths.includes(b) && widths.includes(b - 1), `${b} and ${b - 1}`);
  }
});

test('a site published under a base path is compared at its own root', () => {
  const { root, write } = fixture();
  write('index.html', '<h1>home</h1>');
  write('docs/index.html', '<h1>docs</h1>');
  write('sitemap.xml',
    '<urlset><url><loc>https://x.test/project/</loc></url>' +
    '<url><loc>https://x.test/project/docs/</loc></url></urlset>');
  const pages = discoverPages(root);
  assert.deepEqual(pages.map((p) => p.route), ['/', '/docs/']);
  assert.ok(pages.every((p) => p.rendered && p.published), 'both mechanisms name the same routes');
  rmSync(root, { recursive: true, force: true });
});

test('the base path is the prefix every entry shares, and a site at the root has none', () => {
  assert.equal(commonPrefix(['/project/', '/project/docs/']), '/project/');
  assert.equal(commonPrefix(['/', '/docs/']), '');
  assert.equal(commonPrefix(['/a/x/', '/b/y/']), '');
});

test('every page is visited, and the sweep is spent on one page per section', () => {
  const widths = viewports([640, 1024]);
  const pages = tierPages(
    ['/', '/docs/', '/docs/cli/', '/docs/schemas/', '/skills/', '/skills/deploy/'].map((route) => ({ route })),
    widths,
  );
  assert.equal(pages.length, 6, 'no page is dropped');
  assert.ok(pages.every((p) => p.widths.length > 0), 'no page is visited at no width');
  const sweeps = pages.filter((p) => p.tier === 'sweep').map((p) => p.route);
  assert.deepEqual(sweeps, ['/', '/docs/', '/skills/'], 'the first page of each section');
  const later = pages.find((p) => p.route === '/docs/cli/');
  assert.deepEqual(later.widths, criticalWidths(widths));
  assert.ok(later.why.includes('docs'), later.why);
});

test('the critical widths always hold the reflow floor and the desktop end', () => {
  const widths = criticalWidths(viewports([640, 768, 1024]));
  assert.equal(widths[0], REFLOW_FLOOR);
  assert.equal(widths[widths.length - 1], 1440);
});

test('a new section brings its own sweep without a list changing', () => {
  const widths = viewports([640]);
  const before = tierPages([{ route: '/' }, { route: '/docs/' }], widths);
  const after = tierPages([{ route: '/' }, { route: '/docs/' }, { route: '/reports/' }], widths);
  assert.equal(before.filter((p) => p.tier === 'sweep').length, 2);
  assert.equal(after.filter((p) => p.tier === 'sweep').length, 3);
  assert.equal(after.find((p) => p.route === '/reports/').tier, 'sweep');
});

let failed = 0;
for (const [name, fn] of tests) {
  try {
    fn();
    console.log(`ok   ${name}`);
  } catch (error) {
    failed += 1;
    console.log(`FAIL ${name}\n     ${error.message}`);
  }
}
console.log(`ui: ${tests.length - failed} passed, ${failed} failed`);
process.exit(failed === 0 ? 0 : 1);

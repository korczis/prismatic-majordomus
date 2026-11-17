# majordomus-covers: none
# majordomus-negative: rules
# A value a build changes does not change the key of the sentence it sits in (I3758).
#
# A standalone number is already a <nK/> slot (I3095). This case holds the rest: a commit id, a
# version or a date that is the whole text of an inline element is a void <tN/> placeholder, so
# two builds of one page at different commits and releases cut the same segments with the same
# keys; the translated page puts the page's own element back with its link pointed at the
# language; a date or a version written in running prose stays words; and
# `site-translate rekey` moves an entry written before this rule to the key the build now
# computes, and leaves an entry whose translation lost the value as it was. Nothing in the
# repository is written.
. "$ROOT/test/lib.sh"
PASS="$ROOT/scripts/lib/site-i18n.mjs"
[ -f "$PASS" ] || { echo "    scripts/lib/site-i18n.mjs is missing"; exit 1; }

node --input-type=module - "$PASS" <<'JS' || exit 1
const { segments, localise, rekeyValues, keyOf } = await import(process.argv[2]);
let fail = 0;
const no = (msg) => { console.log(`    ${msg}`); fail = 1; };
const page = (commit, version, date) => `<!doctype html><html lang="en"><head><title>Home</title></head><body>
<p>Delivery the history proves, inferred at <a href="https://example.test/commit/${commit}">${commit.slice(0, 7)}</a></p>
<p>Changes the release record of <a href="https://example.test/changelog/v${version}/">v${version}</a> carries, <time datetime="${date}">${date}</time></p>
<p>Fixed on 2026-10-08, see <a href="https://example.test/plan/">the plan</a>.</p>
<p>Read the <a href="https://example.test/r/">v0.30.2 notes</a> first.</p>
</body></html>`;
const html = (p) => segments(p).filter((s) => s.kind === 'html');
const one = page('a8d7dec1234abcd', '1.0.0-alpha.5', '2026-10-22');
const two = page('76c5785fedcba98', '1.0.0-alpha.6', '2026-10-23');
const a = html(one);
const b = html(two);

// 1. two builds at different commits and releases cut the same keys
if (a.map((s) => s.key).join() !== b.map((s) => s.key).join()) no(`the keys moved with the values: ${a.map((s) => s.source).join(' | ')}`);
const want = ['Delivery the history proves, inferred at <t1/>', 'Changes the release record of <t1/> carries, <t2/>'];
for (const w of want) if (!a.some((s) => s.source === w)) no(`no segment reads "${w}": ${a.map((s) => s.source).join(' | ')}`);

// 1b. the footer: one entry serves every release once the version is its own element (33, 10-22:
//     its key changed with every release and 2,624 /cs/ pages showed it in English)
const foot = (v, c) => segments(`<html><body><footer><p>Generated from Prismatic Majordomus <span class="font-mono">v${v}</span> · commit <a href="https://example.test/commit/${c}" class="font-mono underline">${c.slice(0, 7)}</a> · inputs <span class="font-mono">${c.slice(0, 12)}</span>. Facts are generated.</p></footer></body></html>`).filter((s) => s.kind === 'html');
const f1 = foot('1.0.0-alpha.5', '76c5785914d58122aa2');
const f2 = foot('1.0.0-alpha.6', 'a01a428fedcba987654');
if (f1.map((s) => s.key).join() !== f2.map((s) => s.key).join()) no(`the footer's key moved with the release: ${f1.map((s) => s.source).join(' | ')}`);

// 2. a date and a version written in prose stay words, and are part of the key
if (!a.some((s) => s.source.includes('2026-10-08'))) no('a date in running prose became a placeholder');
if (!a.some((s) => s.source.includes('<t1>v0.30.2 notes</t1>'))) no('a version inside words became a placeholder');

// 3. the translated page puts the page's own element back, its link pointed at the language
const seg = a.find((s) => s.source === want[1]);
const memory = new Map([[seg.key, { k: seg.key, en: seg.source, cs: 'Změny, které nese záznam vydání <t1/>, <t2/>' }]]);
const out = localise(one, { lang: 'cs', memory, base: 'https://example.test', routes: new Set(['/changelog/v1.0.0-alpha.5/']), route: '/', englishRoute: '/' });
const text = typeof out === 'string' ? out : out.html;
if (!text.includes('<a href="https://example.test/cs/changelog/v1.0.0-alpha.5/">v1.0.0-alpha.5</a>, <time datetime="2026-10-22">2026-10-22</time>')) {
  no(`the value element did not come back with its counterpart link: ${(text.match(/Změny[^\n]*/) || [''])[0]}`);
}

// 4. rekey moves an old entry to the build's key, and refuses one whose translation lost the value
const old = { k: 'old', en: 'Changes the release record of <t1>v0.28.0</t1> carries, <t2>2026-09-30</t2>', cs: 'Změny, které nese záznam vydání <t1>v0.28.0</t1>, <t2>2026-09-30</t2>' };
const moved = rekeyValues(old, 'cs');
if (!moved || moved.refused || moved.entry.k !== keyOf(want[1]) || moved.entry.cs !== 'Změny, které nese záznam vydání <t1/>, <t2/>') no(`rekey did not move the entry to the build's key: ${JSON.stringify(moved)}`);
const lost = rekeyValues({ k: 'kept', en: 'inferred at <t1>a8d7dec</t1>', cs: 'odvozeno u <t1>jiného</t1>' }, 'cs');
if (!lost || lost.refused !== '<t1>a8d7dec</t1>' || lost.entry.k !== 'kept') no(`rekey moved an entry whose translation lost the value: ${JSON.stringify(lost)}`);
if (rekeyValues({ k: 'w', en: '<t1>Fixed</t1> words', cs: '<t1>Opraveno</t1> slova' }, 'cs') !== null) no('rekey touched an entry that holds no value');
process.exit(fail);
JS
echo "    a commit, version or date that is a whole element is a placeholder: two builds share their keys, and rekey moves the memory"

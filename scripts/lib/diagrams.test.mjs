// The diagram pass's own tests (scripts/lib/diagrams.mjs, ADR 0152): what a source declares,
// what a node resolves to, and what a built page becomes. Every claim is made against a
// fixture written here, never against this repository's own pages.
//
//   node --test scripts/lib/diagrams.test.mjs
//
// The pass end to end, over a built fixture site and through scripts/ci/link-check, is
// test/cases/3006_diagrams_are_interactive_and_linkable.sh.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { ACTIONS, annotate, buildIndex, nodes, plain, resolve, segments, unescapeHtml } from './diagrams.mjs';

const ids = (source) => nodes(source).map((n) => n.id);

test('a flowchart declares its nodes once each, in first-seen order, with their labels', () => {
  const found = nodes(`flowchart LR
    subgraph canonical
        A[share/skeleton]
        B[docs + README]
    end
    G[scripts/generate-site-data]
    D[(site/data/generated)]
    A --> G
    B --> G
    G --> D --> Z --> P`);
  assert.deepEqual(found.map((n) => n.id), ['A', 'B', 'G', 'D', 'Z', 'P']);
  assert.equal(found[0].label, 'share/skeleton');
  assert.equal(found[3].label, 'site/data/generated');
  assert.equal(found[4].label, 'Z', 'a node with no shape is labelled by its id');
});

test('the text of a link is not a node, in any of the three ways a link carries text', () => {
  assert.deepEqual(ids('flowchart TD\n  A -->|stamp| B'), ['A', 'B']);
  assert.deepEqual(ids('flowchart TD\n  A -- reads --> B'), ['A', 'B']);
  assert.deepEqual(ids('flowchart TD\n  L4 -. "exit 10 · the commit does not land" .-> L2'), ['L4', 'L2']);
  assert.deepEqual(ids('flowchart TD\n  A == writes ==> B'), ['A', 'B']);
});

test('links without spaces, open links and chained links split where Mermaid splits them', () => {
  assert.deepEqual(ids('graph TD\n  A-->B'), ['A', 'B']);
  assert.deepEqual(ids('graph TD\n  A --- B --> C'), ['A', 'B', 'C']);
  assert.deepEqual(ids('graph TD\n  A -.-> B ==> C'), ['A', 'B', 'C']);
  assert.deepEqual(ids('graph TD\n  A o--o B x--x C'), ['A', 'B', 'C']);
  assert.deepEqual(ids('graph TD\n  A & B --> C'), ['A', 'B', 'C']);
});

test('a quoted label, a class and a shape with two brackets are read whole', () => {
  const found = nodes(`flowchart LR
    I0001["I0001<br/>Audit what (actually) plans work"]:::done
    M(["Model<br/><small>chosen per run</small>"])
    I0001 --> M
    classDef done stroke-width:2px
    class I0001 done`);
  assert.deepEqual(found.map((n) => n.id), ['I0001', 'M']);
  assert.equal(plain(found[0].label), 'I0001 Audit what (actually) plans work');
  assert.deepEqual(segments(found[1].label), ['Model', 'chosen per run']);
});

test('a state diagram declares its states and never the start and end markers', () => {
  const found = nodes(`stateDiagram-v2
    [*] --> active: start
    active --> completed: finish --outcome completed
    completed --> [*]
    state "Handed over" as handed_over
    active --> handed_over`);
  assert.deepEqual(found.map((n) => n.id), ['active', 'completed', 'handed_over']);
  assert.equal(found[2].label, 'Handed over');
});

test('a diagram kind whose nodes carry no ids declares none, and a comment is not a statement', () => {
  assert.deepEqual(nodes('sequenceDiagram\n  Alice->>Bob: hello'), []);
  assert.deepEqual(ids('flowchart TD\n  %% X --> Y\n  A --> B %% C'), ['A', 'B']);
});

const index = new Map([
  ['i0001', { href: '/plan/i0001/', title: 'Audit', kind: 'issue' }],
  ['adr 0152', { href: '/adrs/adr-0152/', title: 'Diagrams', kind: 'adr' }],
  ['adr-0152', { href: '/adrs/adr-0152/', title: 'Diagrams', kind: 'adr' }],
  ['majordomus doctor', { href: '/commands/doctor/', title: 'doctor', kind: 'command' }],
  ['majordomus plan', { href: '/commands/plan/', title: 'plan', kind: 'command' }],
  ['docs/design.md', { href: '/docs/design/', title: 'design', kind: 'document' }],
  ['design.md', { href: '/docs/design/', title: 'design', kind: 'document', whole: true }],
  ['project.derived-once', { href: '/rules/project-derived-once-1/', title: 'derived once', kind: 'rule' }],
]);

test('a node resolves by its id, by its label, or by one line of its label', () => {
  assert.equal(resolve({ id: 'I0001', label: 'I0001<br/>Audit' }, index).href, '/plan/i0001/');
  assert.equal(resolve({ id: 'D', label: 'majordomus doctor' }, index).href, '/commands/doctor/');
  assert.equal(resolve({ id: 'X', label: 'The design<br/><small>docs/DESIGN.md</small>' }, index).href, '/docs/design/');
});

test('an identity inside a longer label resolves, and a subcommand resolves to its command', () => {
  assert.equal(resolve({ id: 'X', label: 'decided in ADR 0152' }, index).href, '/adrs/adr-0152/');
  assert.equal(resolve({ id: 'X', label: 'run majordomus plan next first' }, index).href, '/commands/plan/');
  assert.equal(resolve({ id: 'X', label: 'held by project.derived-once' }, index).href, '/rules/project-derived-once-1/');
});

test('a label that also names an object with no page, or a file by its bare name, is not linked', () => {
  assert.equal(resolve({ id: 'b', label: 'stage B: docs/SITE_CLAIMS.md, docs/DESIGN.md' }, index), null, 'one of two documents has no page');
  assert.equal(resolve({ id: 'x', label: 'docs/generated/design.json, design.yaml, design.md the inventory' }, index), null);
  assert.equal(resolve({ id: 'x', label: 'The design<br/>DESIGN.md' }, index).href, '/docs/design/', 'a whole line may be a bare file name');
  assert.equal(resolve({ id: 'x', label: 'decided in ADR 0152, see ADR 0999' }, index), null);
  assert.equal(resolve({ id: 'x', label: 'policy.yaml is read by project.derived-once' }, index).href, '/rules/project-derived-once-1/', 'a file name is not an identity');
});

test('a label written with Mermaid entities is read as the text it draws', () => {
  assert.equal(plain('majordomus start #quot;&lt;task&gt;#quot; --scope &lt;paths&gt;'), 'majordomus start "<task>" --scope <paths>');
  assert.equal(plain('"**bold** `code`<br/><small>small</small>"'), 'bold code small');
});

test('a label naming two objects links to neither, and one naming none links to nothing', () => {
  assert.equal(resolve({ id: 'X', label: 'ADR 0152 and I0001' }, index), null);
  assert.equal(resolve({ id: 'X', label: 'Zola' }, index), null);
  assert.equal(resolve({ id: 'plan', label: 'the plan' }, index), null, 'a bare word is not a command');
});

function fixture() {
  const root = mkdtempSync(join(tmpdir(), 'mj-diagrams-'));
  const data = join(root, 'data'); const pub = join(root, 'public');
  mkdirSync(data); mkdirSync(pub);
  const page = (route) => { mkdirSync(join(pub, route), { recursive: true }); writeFileSync(join(pub, route, 'index.html'), '<html></html>'); };
  writeFileSync(join(data, 'entities.json'), JSON.stringify({
    kinds: [{ kind: 'adr', route: '/adrs/' }, { kind: 'rule', route: '/rules/' }],
    entities: [
      { kind: 'adr', identity: 'adr-0152', slug: 'adr-0152', title: 'Diagrams' },
      { kind: 'adr', identity: 'adr-0999', slug: 'adr-0999', title: 'Unbuilt' },
      { kind: 'rule', identity: 'project.derived-once@1', slug: 'project-derived-once-1', title: 'Derived once' },
    ],
  }));
  writeFileSync(join(data, 'plan.json'), JSON.stringify({ issues: [{ id: 'I0001', title: 'Audit' }], milestones: [{ id: 'M000', title: 'Zero' }] }));
  writeFileSync(join(data, 'commands.json'), JSON.stringify({ commands: [{ name: 'doctor', summary: 'Check' }] }));
  writeFileSync(join(data, 'docs.json'), JSON.stringify({ docs: [{ path: 'DESIGN.md', route: '/docs/design/', summary: 'design' }] }));
  for (const r of ['adrs/adr-0152', 'rules/project-derived-once-1', 'plan/i0001', 'plan/m000', 'commands/doctor', 'docs/design']) page(r);
  return { root, data, pub };
}

test('the index offers an identity only when the build wrote its page, under the base it was built for', () => {
  const { root, data, pub } = fixture();
  try {
    const served = (route) => ['/adrs/adr-0152/', '/plan/i0001/', '/commands/doctor/', '/docs/design/', '/rules/project-derived-once-1/'].includes(route);
    const built = buildIndex(data, served, 'https://example.test/base/');
    assert.equal(built.get('adr 0152').href, 'https://example.test/base/adrs/adr-0152/');
    assert.equal(built.get('i0001').kind, 'issue');
    assert.equal(built.get('majordomus doctor').href, 'https://example.test/base/commands/doctor/');
    assert.equal(built.get('project.derived-once').href, built.get('project.derived-once@1').href);
    assert.equal(built.get('docs/design.md').href, built.get('design.md').href);
    assert.equal(built.has('adr-0999'), false, 'a decision whose page was not written is not offered');
    assert.equal(built.has('m000'), false, 'nor a milestone whose page was not written');
    assert.ok(pub);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('an identity two objects claim names neither', () => {
  const root = mkdtempSync(join(tmpdir(), 'mj-diagrams-'));
  try {
    writeFileSync(join(root, 'plan.json'), JSON.stringify({ issues: [{ id: 'X1', title: 'a' }], milestones: [{ id: 'x1', title: 'b' }] }));
    writeFileSync(join(root, 'docs.json'), JSON.stringify({ docs: [{ path: 'X1', route: '/docs/x1/', summary: 's' }] }));
    const built = buildIndex(root, () => true);
    assert.equal(built.has('x1'), false);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

const PAGE = `<html><body><h2 id="how-it-flows">How it flows</h2>
<pre class="mermaid">flowchart LR
    A[majordomus doctor] --&gt; I0001
    I0001 --&gt; Z[Zola]
</pre>
<pre class="mermaid m-0">flowchart TD
    Q --&gt; R
</pre>
<h2 id="states">States</h2>
<figure><pre class="mermaid" data-diagram="lifecycle" tabindex="0">stateDiagram-v2
    [*] --&gt; active
</pre></figure></body></html>`;

test('a page gets each diagram named, its nodes listed, and its inferred links written beside it', () => {
  const { text, diagrams, links } = annotate(PAGE, index);
  assert.equal(diagrams, 3);
  assert.equal(links, 2);
  assert.match(text, /<pre class="mermaid" tabindex="0" id="diagram-how-it-flows" data-diagram="how-it-flows" data-nodes="A I0001 Z">/);
  assert.match(text, /<pre class="mermaid m-0" tabindex="0" id="diagram-how-it-flows-2" data-diagram="how-it-flows-2" data-nodes="Q R">/);
  assert.match(text, /<pre class="mermaid" tabindex="0" id="diagram-lifecycle" data-diagram="lifecycle" data-nodes="active">/);
  assert.match(text, /<details class="mj-diagram-links" data-diagram-links="diagram-how-it-flows"><summary>Linked from this diagram \(2\)<\/summary>/);
  assert.match(text, /<a href="\/commands\/doctor\/" data-node="A" data-kind="command">majordomus doctor<\/a>/);
  assert.match(text, /<a href="\/plan\/i0001\/" data-node="I0001" data-kind="issue">I0001<\/a>/);
  assert.equal((text.match(/mj-diagram-links/g) || []).length, 1, 'a diagram with no link has no list');
});

test('every diagram gets the same toolbar, hidden, naming the diagram it controls', () => {
  const { text } = annotate(PAGE, index);
  const bars = [...text.matchAll(/<pre\b[^>]*\sid="([^"]+)"[^>]*>[^<]*<\/pre><div class="mj-diagram-bar" role="toolbar" aria-label="Diagram ([^"]+)" aria-controls="(diagram-[^"]+)" data-diagram-bar="\3" hidden>([\s\S]*?)<\/div>/g)];
  assert.deepEqual(bars.map((b) => b[3]), ['diagram-how-it-flows', 'diagram-how-it-flows-2', 'diagram-lifecycle']);
  for (const bar of bars) {
    assert.equal(bar[1], bar[3], 'a toolbar stands directly after the diagram it controls');
    const actions = [...bar[4].matchAll(/data-diagram-action="([^"]+)" aria-label="([^"]+)" title="\2">([^<]+)</g)];
    assert.deepEqual(actions.map((a) => a[1]), ACTIONS.map((a) => a[0]));
    for (const [, , label, shown] of actions) {
      assert.ok(shown.length === 1 || label.includes(shown), `the name "${label}" contains what the button shows, "${shown}"`);
    }
  }
});

test('the source of a diagram is not changed by a byte', () => {
  const { text } = annotate(PAGE, index);
  const body = (html) => [...html.matchAll(/<pre\b[^>]*>([\s\S]*?)<\/pre>/g)].map((m) => m[1]);
  assert.deepEqual(body(text), body(PAGE));
  assert.equal(unescapeHtml(body(PAGE)[0]).includes('A[majordomus doctor] --> I0001'), true);
});

test('a second pass over its own output changes nothing', () => {
  const once = annotate(PAGE, index).text;
  assert.equal(annotate(once, index).text, once);
});

test('a page without a diagram is returned as it was', () => {
  const html = '<html><body><pre class="console">$ ls</pre></body></html>';
  assert.equal(annotate(html, index).text, html);
  assert.equal(annotate(html, index).diagrams, 0);
});

test('a name that is already an id on the page is not given a second time', () => {
  const html = '<h2 id="x">X</h2><div id="diagram-x"></div><pre class="mermaid">graph TD\n A--&gt;B</pre>';
  assert.match(annotate(html).text, /id="diagram-x-2"/);
});

test('two diagrams declared under one name, and a diagram with a foreign id, are refused', () => {
  const twice = '<pre class="mermaid" data-diagram="plan">graph TD\n A</pre><pre class="mermaid" data-diagram="plan">graph TD\n B</pre>';
  assert.throws(() => annotate(twice), /two diagrams on one page are named "plan"/);
  assert.throws(() => annotate('<pre class="mermaid" id="mine">graph TD\n A</pre>'), /a diagram's id is diagram-<name> and is derived/);
});

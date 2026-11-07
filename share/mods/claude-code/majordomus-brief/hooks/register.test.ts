// Run with `claude plugin test share/mods/claude-code/majordomus-brief` (ADR 0136).
import { test, expect, mock } from 'claude-code/testing';
import type { On, PromptSubmitInput } from 'claude-code';
import {
  SELF_REPORT,
  changedSections,
  fingerprint,
  normalise,
  reminder,
  selfReport,
  whatChanged,
  type Brief,
} from './register.ts';

const MINUTE = 60_000;

// A brief as `majordomus brief --format json` answers it, with every form that moves with the
// clock: the `at`, an observation's age and instant, a merge's age, the release's wait, the
// derive lock's hold and why the leaseholder did not answer.
type Facts = {
  at?: string;
  head?: string;
  attached?: number;
  open?: number;
  observed?: string;
  waited?: string;
  held?: string;
  answeredBy?: string;
  interval?: number;
  line?: string;
};
const make = (f: Facts = {}): Brief => {
  const head = f.head ?? '1a2b3c4d5e';
  const attached = f.attached ?? 2;
  const peers = Array.from(
    { length: attached },
    (_, i) => `             p${i + 1} claude-code @main: announced nothing`,
  );
  return {
    at: f.at ?? '2026-10-26T15:33:26Z',
    line: f.line ?? `t-1 @main | peers ${attached}/3b | mesh 0/0 | prs ${f.open ?? 14} open`,
    lines: [
      `session      episode s-1 on main at ${head}; task t-1: cover the brief`,
      `peers        ${attached} attached on 3 board(s)`,
      ...peers,
      'mesh         0 of 0 link(s) connected; 2 claim(s) held, 2 of them this runtime\'s',
      `integration  main at ${head} observed ${f.observed ?? '4 d ago (2026-10-21T16:55:57Z)'}: ` +
        `${f.open ?? 14} open — held 3, waiting 11; nothing ready; 2 diagnostic(s): ` +
        'majordomus prs status; lease free; last merge #2366 4 d ago',
      'release      27 landed commit(s) in 2 landing(s) wait past v1.0.0 on origin/main, the ' +
        `oldest for ${f.waited ?? '21 h 44 min'} since this clone saw it land`,
      `derive       lock held by derive:main (pid 4242, running, ${f.held ?? '12 min'})`,
      `answered by  ${f.answeredBy ?? 'this_process (no server answers for this checkout)'}`,
    ],
    interval_minutes: f.interval ?? 5,
    session: { branch: 'main', head },
    release: { stance: 'owed' },
  };
};
const json = (f: Facts = {}) => JSON.stringify(make(f));

// The same facts n minutes later: every form that moves with the clock, moved.
const two = (n: number) => String(n % 60).padStart(2, '0');
const later = (n: number, f: Facts = {}): Facts => ({
  at: `2026-10-26T${String(15 + Math.floor(n / 60)).padStart(2, '0')}:${two(n)}:26Z`,
  observed: `${n} min ago (2026-10-27T10:${two(n)}:51Z)`,
  waited: `${n} h ${n % 60} min`,
  held: `${n} min`,
  answeredBy: `this_process (the leaseholder did not answer (os error ${n}))`,
  ...f,
});

// The test's hooks stand for the engine beneath the plugin.
const engine = (on: On, opts: { supervised: boolean; answer?: () => string; exit?: number }) => {
  const submitted: PromptSubmitInput[] = [];
  const status: (string | undefined)[] = [];
  const argvs: (readonly string[])[] = [];
  on('session.start', (_$, e) => ({ cwd: e.cwd }));
  on('session.root', () => ({ value: '/repo' }));
  on('session.id', () => ({ value: 'sess-1' }));
  on('fs.exists', (_$, e) => ({
    value:
      (opts.supervised && e.path === '/repo/.ai/manifest.yaml') || e.path === '/repo/bin/majordomus',
  }));
  on('process.run', (_$, e) => {
    argvs.push(e.argv);
    return {
      value: {
        exitCode: opts.exit ?? 0,
        stdout: opts.answer ? opts.answer() : json({ line: 'I1 @main | peers 2/3b' }),
        stderr: '',
        isStdoutTruncated: false,
        isStderrTruncated: false,
      },
    };
  });
  on('ui.status', (_$, e) => {
    status.push(e.text);
    return { value: undefined };
  });
  on('prompt.submit', (_$, e) => {
    submitted.push(e);
    return { text: e.text };
  });
  return { submitted, status, argvs };
};

const start = { cwd: '/repo', surface: 'terminal' as const, isInteractive: true };

// ---------------------------------------------------------------- the fingerprint

test('every relative age and instant the brief emits is the clock, not a fact', () => {
  const forms: [string, string][] = [
    // `prs brief`'s ago(): s, min, h and d
    ['observed 95 s ago (2026-10-21T16:55:57Z)', 'observed <age> ago (<at>)'],
    ['observed 4 min ago', 'observed <age> ago'],
    ['observed 38 h ago', 'observed <age> ago'],
    ['last merge #2366 5 d ago', 'last merge #2366 <age> ago'],
    ['lease stale (renewed 95 s ago; the next', 'lease stale (renewed <age> ago; the next'],
    // `release.currency`'s age_words(): min, h min, d h
    ['the oldest for 7 min since', 'the oldest for <age> since'],
    ['the oldest for 21 h 44 min since', 'the oldest for <age> since'],
    ['the oldest for at most 3 d 2 h: this clone', 'the oldest for at most <age>: this clone'],
    ['longer than the 6 h 0 min a release may wait', 'longer than the <age> a release may wait'],
    // the derive lock's hold, in the lines and in the one line
    ['(pid 4242, running, 12 min)', '(pid 4242, running, <age>)'],
    ['derive derive:main 12m', 'derive derive:main <age>'],
    // the compact forms other surfaces print
    ['p1 here (16s)', 'p1 here (<age>)'],
    ['checkpoint 36m ago', 'checkpoint <age> ago'],
    ['at 2026-10-27T10:21:10.123+02:00', 'at <at>'],
    ['at 2026-10-27 10:21', 'at <at>'],
  ];
  for (const [raw, said] of forms) expect(normalise(raw)).toBe(said);
});

test('what is a fact survives normalisation', () => {
  const facts = [
    'main at 1a2b3c4d5e observed',
    'main at 123456789d observed',
    '14 open — held 3, waiting 11; 2 diagnostic(s)',
    '3 attached on 15 board(s)',
    'p1 claude-code @main [claude-code pid 4242 on host.local, /tmp/peer/4242.sock]',
    'next merge #2565; 27 landed commit(s) in 2 landing(s) past v1.0.0-alpha.8',
  ];
  for (const f of facts) expect(normalise(f)).toBe(f);
  expect(normalise('a   b')).toBe('a b');
});

test('two briefs that differ only by the clock have one fingerprint', () => {
  expect(fingerprint(make(later(7)))).toBe(fingerprint(make()));
  expect(fingerprint(make(later(59)))).toBe(fingerprint(make(later(3))));
  expect(changedSections(make(), make(later(42)))).toEqual([]);
});

test('every fact moves the fingerprint, and the change is named', () => {
  const base = make();
  const cases: [Facts, string[], string][] = [
    [{ head: '9f8e7d6c5b' }, ['session', 'integration'], 'main moved 1a2b3c4 → 9f8e7d6'],
    [{ attached: 3 }, ['peers'], 'a peer attached'],
    [{ attached: 0 }, ['peers'], '2 peers left'],
    [{ open: 23 }, ['integration'], 'open pull requests went from 14 to 23'],
    [{ answeredBy: 'superseded (x)' }, ['answered by'], 'another process answers the brief'],
  ];
  for (const [f, sections, words] of cases) {
    const after = make(f);
    expect(fingerprint(after)).not.toBe(fingerprint(base));
    expect(changedSections(base, after)).toEqual(sections);
    expect(whatChanged(base, after)).toBe(words);
  }
  // one move is said once, though two sections carry it
  expect(whatChanged(base, make({ head: '9f8e7d6c5b', attached: 3 }))).toBe(
    'main moved 1a2b3c4 → 9f8e7d6 and a peer attached',
  );
  // the mesh as a runtime with links says it (2026-10-27, two reads a minute apart)
  const mesh = (claims: number, overlaps: number): Brief => ({
    ...base,
    lines: base.lines.map(l =>
      l.startsWith('mesh ')
        ? `mesh         0 of 0 link(s) connected; ${claims} claim(s) held, ${claims} of them ` +
          `this runtime's; for this runtime: 0 review(s), 0 handover(s) waiting, ${overlaps} overlap(s)`
        : l,
    ),
  });
  expect(changedSections(mesh(3, 2), mesh(2, 1))).toEqual(['mesh']);
  expect(whatChanged(mesh(3, 2), mesh(2, 1))).toBe(
    'mesh claims went from 3 to 2 and the mesh now has 0 review(s), 0 handover(s) waiting, ' +
      '1 overlap(s) for this session',
  );
  // which process composed the brief is a fact; why another did not answer is not
  expect(fingerprint(make({ answeredBy: 'this_process (os error 35)' }))).toBe(
    fingerprint(make({ answeredBy: 'this_process (os error 61)' })),
  );
});

// ---------------------------------------------------------------- the submitted text

test('the self-report says what changed, then the brief, then one line to the agent', () => {
  const text = selfReport(make(), make({ head: '9f8e7d6c5b' }));
  expect(text.startsWith(SELF_REPORT)).toBe(true);
  const [opening, ...rest] = text.split('\n');
  expect(opening).toBe('Majordomus brief: since the last report, main moved 1a2b3c4 → 9f8e7d6.');
  const after = rest.slice(rest.lastIndexOf('```') + 1).filter(l => l !== '');
  expect(after).toHaveLength(1);
  expect(after[0]).toMatch(/^To the agent: /);
  expect(text).not.toMatch(/two to four lines:|if there is none, say so and stop/);
  expect(reminder(make(), make({ attached: 3 }))).toMatch(
    /^Majordomus brief at 2026-10-26T15:33:26Z, changed since this session was last told: a peer attached \(ADR 0136\)\./,
  );
});

// ---------------------------------------------------------------- the session

test('the status line is the brief, refreshed on the clock, read without building', async ($, on) => {
  const clock = mock.clock(on);
  let n = 0;
  const w = engine(on, { supervised: true, answer: () => json({ line: `line ${++n}` }) });
  await $.session.start(start);
  expect(w.status.at(-1)).toBe('line 1');
  expect(w.argvs[0]).toEqual(['/repo/bin/majordomus', 'brief', '--format', 'json', '--session', 'sess-1']);
  await clock.advance(MINUTE);
  expect(w.status.at(-1)).toBe('line 2');
  expect(w.submitted).toHaveLength(0);
});

test('an idle session whose surroundings only aged is never woken', async ($, on) => {
  // 2026-10-26/27: about 40 self-reports overnight that differed by `at` and "N d ago" alone
  const clock = mock.clock(on);
  let n = 0;
  const w = engine(on, { supervised: true, answer: () => json(later(++n)) });
  await $.session.start(start);
  await clock.advance(12 * 60 * MINUTE);
  expect(w.status.length).toBeGreaterThan(700);
  expect(w.submitted).toHaveLength(0);
});

test('a changed fact wakes an idle session once, at most once per interval', async ($, on) => {
  const clock = mock.clock(on);
  let facts: Facts = {};
  let n = 0;
  const w = engine(on, { supervised: true, answer: () => json(later(++n, facts)) });
  await $.session.start(start);
  // the change lands a minute in, before the interval (5) since the start has passed
  await clock.advance(MINUTE);
  facts = { head: '9f8e7d6c5b' };
  await clock.advance(3 * MINUTE);
  expect(w.submitted).toHaveLength(0);
  await clock.advance(MINUTE);
  expect(w.submitted).toHaveLength(1);
  expect(w.submitted[0]?.text.startsWith(SELF_REPORT)).toBe(true);
  expect(w.submitted[0]?.text).toContain('since the last report, main moved 1a2b3c4 → 9f8e7d6.');
  expect(w.submitted[0]?.text).toMatch(/peers {8}2 attached on 3 board\(s\)/);
  // nothing new for an hour: nothing more
  await clock.advance(60 * MINUTE);
  expect(w.submitted).toHaveLength(1);
  // a second change is told as itself, not with the first again
  facts = { head: '9f8e7d6c5b', attached: 3 };
  await clock.advance(MINUTE);
  expect(w.submitted).toHaveLength(2);
  expect(w.submitted[1]?.text).toContain('since the last report, a peer attached.');
  // a third, a minute after the second, waits out the interval
  facts = { head: '9f8e7d6c5b', attached: 3, open: 20 };
  await clock.advance(MINUTE);
  expect(w.submitted).toHaveLength(2);
  await clock.advance(4 * MINUTE);
  expect(w.submitted).toHaveLength(3);
  expect(w.submitted[2]?.text).toContain('open pull requests went from 14 to 20');
});

test('a change that reverts before the interval passes is not news', async ($, on) => {
  const clock = mock.clock(on);
  let facts: Facts = {};
  let n = 0;
  const w = engine(on, { supervised: true, answer: () => json(later(++n, facts)) });
  await $.session.start(start);
  facts = { attached: 3 };
  await clock.advance(2 * MINUTE);
  facts = {};
  await clock.advance(30 * MINUTE);
  expect(w.submitted).toHaveLength(0);
});

test('a prompt after the interval carries the brief only when a fact changed', async ($, on) => {
  // the refreshes fail after the second, so no self-report is queued and the prompt is the
  // only way the brief reaches the model
  const clock = mock.clock(on);
  const session = mock.session(on);
  let n = 0;
  const w = engine(on, {
    supervised: true,
    answer: () => {
      n += 1;
      if (n === 1) return json({ interval: 30 });
      if (n === 2) return json(later(2, { interval: 30, attached: 3 }));
      return 'not json';
    },
  });
  await $.session.start(start);
  await clock.advance(10 * MINUTE);
  await $.prompt.submit({ text: 'early' });
  expect(session.appended()).toHaveLength(0);
  await clock.advance(21 * MINUTE);
  await $.prompt.submit({ text: 'late' });
  const rows = session.appended();
  expect(rows).toHaveLength(1);
  const said = JSON.stringify(rows[0]?.message.content);
  expect(said).toMatch(/changed since this session was last told: a peer attached/);
  expect(said).toMatch(/2026-10-26T15:02:26Z/);
  // told now: the same brief is not appended again, however long the session runs
  await clock.advance(60 * MINUTE);
  await $.prompt.submit({ text: 'again' });
  expect(session.appended()).toHaveLength(1);
  expect(w.submitted.filter(p => p.text.startsWith(SELF_REPORT))).toHaveLength(0);
});

test('a prompt after the interval adds nothing when only the clock moved', async ($, on) => {
  const clock = mock.clock(on);
  const session = mock.session(on);
  let n = 0;
  engine(on, { supervised: true, answer: () => json(later(++n, { interval: 30 })) });
  await $.session.start(start);
  await clock.advance(31 * MINUTE);
  await $.prompt.submit({ text: 'late' });
  await clock.advance(31 * MINUTE);
  await $.prompt.submit({ text: 'later' });
  expect(session.appended()).toHaveLength(0);
});

test('the self-report is not briefed again on its way through the prompt hook', async ($, on) => {
  const clock = mock.clock(on);
  const session = mock.session(on);
  let facts: Facts = {};
  let n = 0;
  const w = engine(on, { supervised: true, answer: () => json(later(++n, facts)) });
  await $.session.start(start);
  facts = { attached: 3 };
  await clock.advance(5 * MINUTE);
  expect(w.submitted).toHaveLength(1);
  expect(session.appended()).toHaveLength(0);
});

test('a brief that cannot be read says so on the status line and asks nothing', async ($, on) => {
  const clock = mock.clock(on);
  const w = engine(on, { supervised: true, exit: 12 });
  await $.session.start(start);
  expect(w.status.at(-1)).toBe('majordomus brief: not answered');
  await clock.advance(60 * MINUTE);
  expect(w.submitted).toHaveLength(0);
});

test('stays silent outside a supervised repository', async ($, on) => {
  const clock = mock.clock(on);
  const w = engine(on, { supervised: false });
  await $.session.start(start);
  await clock.advance(60 * MINUTE);
  expect(w.argvs).toHaveLength(0);
  expect(w.status).toHaveLength(0);
  expect(w.submitted).toHaveLength(0);
});

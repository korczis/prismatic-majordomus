// Run with `claude plugin test share/mods/claude-code/majordomus-brief` (ADR 0136).
import { test, expect, mock } from 'claude-code/testing';
import type { On, PromptSubmitInput } from 'claude-code';
import {
  LEGACY_MARKER,
  changedSections,
  fingerprint,
  isOwn,
  reminder,
  selfReport,
  type Brief,
} from './register.ts';

const MINUTE = 60_000;
const MARKER = 'Majordomus report owed';

// A brief as `majordomus brief --format json` answers it: per-section fingerprints of its
// clock-free facts, and the report composed from the contract, which this mod relays.
type Facts = {
  at?: string;
  integration?: string;
  peers?: string;
  interval?: number;
  line?: string;
};
const make = (f: Facts = {}): Brief => {
  const at = f.at ?? '2026-10-29T09:40:12Z';
  const lines = [
    'attention    [stale] integration: queue observed 17 h ago at 81ef931a3e; origin/master is',
    '             5370ee63d1 (+3) — majordomus prs refresh (session)',
    `session      [current] p2 = 07a5ffde (provider session); episode s-1 on main at 1a2b3c4d5e`,
  ];
  return {
    at,
    line: f.line ?? 'p2 I4334 @main | integration stale queue observed 17 h ago | prs stale 17h',
    lines,
    interval_minutes: f.interval ?? 5,
    attention: [
      {
        id: 'integration.stale',
        section: 'integration',
        word: 'stale',
        fact: 'queue observed 17 h ago at 81ef931a3e; origin/master is 5370ee63d1 (+3)',
        remedy: 'majordomus prs refresh',
        actor: 'session',
      },
    ],
    report: {
      contract: 'brief-report/v1',
      fingerprint: '3f9a1c07e2b4',
      marker: MARKER,
      prompt: `${MARKER}: the brief composed at ${at} under brief-report/v1 #3f9a1c07e2b4.\n\n(the rest, as brief.session composed it)`,
      reminder: `Majordomus brief ${at} (brief-report/v1 #3f9a1c07e2b4)\n\n(the lines)`,
    },
    fingerprints: {
      session: 'aaaaaaaaaaaa',
      work: 'bbbbbbbbbbbb',
      peers: f.peers ?? 'cccccccccccc',
      mesh: 'dddddddddddd',
      integration: f.integration ?? 'eeeeeeeeeeee',
      ci: 'ffffffffffff',
      machine: '111111111111',
      release: '222222222222',
      derive: '333333333333',
    },
  };
};
const json = (f: Facts = {}) => JSON.stringify(make(f));

// An executable older than the report contract: no report, no fingerprints.
const legacy = (head = '1a2b3c4d5e', at = '2026-10-26T15:33:26Z', age = '4 min'): Brief => ({
  at,
  line: 'I1 @main',
  lines: [`session      episode s-1 on main at ${head}; observed ${age} ago (${at})`],
  interval_minutes: 5,
});

// The same facts n minutes later: only the clock moved.
const two = (n: number) => String(n % 60).padStart(2, '0');
const later = (n: number, f: Facts = {}): Facts => ({
  at: `2026-10-29T${String(9 + Math.floor((40 + n) / 60)).padStart(2, '0')}:${two(40 + n)}:12Z`,
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

// ---------------------------------------------------------------- what is relayed

test('the submitted prompt is report.prompt, byte for byte', () => {
  const b = make();
  expect(selfReport(b)).toBe(b.report?.prompt);
  expect(reminder(b)).toBe(b.report?.reminder);
});

test('an executable older than the contract is said to predate it, and names no category', () => {
  const text = selfReport(legacy());
  expect(text.startsWith(LEGACY_MARKER)).toBe(true);
  expect(text).toContain('predates brief-report/v1');
  expect(text).toContain('session      episode s-1 on main');
  expect(text).not.toMatch(/two to four lines|a board that could not be read|superseded server/);
  expect(reminder(legacy())).toContain('predates brief-report/v1');
});

test('the guard knows the report marker and the legacy marker', () => {
  const b = make();
  expect(isOwn(`${MARKER}: the brief composed at x`, b)).toBe(true);
  expect(isOwn(`${LEGACY_MARKER} something`, b)).toBe(true);
  expect(isOwn(`${LEGACY_MARKER} something`, undefined)).toBe(true);
  expect(isOwn('Please fix the build', b)).toBe(false);
});

// ---------------------------------------------------------------- what counts as a change

test('a change in a section fingerprint, and only that, is a change', () => {
  expect(changedSections(make(), make(later(42)))).toEqual([]);
  expect(fingerprint(make(later(7)))).toBe(fingerprint(make()));
  expect(changedSections(make(), make({ integration: '999999999999' }))).toEqual(['integration']);
  expect(changedSections(make(), make({ peers: '999999999999', integration: '8' }))).toEqual([
    'peers',
    'integration',
  ]);
  // lines and the line that differ while every fingerprint holds are not a change
  const reworded = { ...make(), lines: ['something else entirely'], line: 'another line' };
  expect(changedSections(make(), reworded)).toEqual([]);
  // a section read past its budget is `late`: not read, so not changed, either way round
  expect(changedSections(make(), make({ peers: 'late' }))).toEqual([]);
  expect(changedSections(make({ peers: 'late' }), make())).toEqual([]);
});

test('an older executable is compared by its lines with the clock taken out', () => {
  expect(changedSections(legacy(), legacy('1a2b3c4d5e', '2026-10-26T16:01:02Z', '31 min'))).toEqual([]);
  expect(changedSections(legacy(), legacy('9f8e7d6c5b'))).toEqual(['brief']);
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
  // the second read says since when (the start event's briefing, the first report) and
  // which fingerprints the session was last told, so the report names what changed
  expect(w.argvs[1]).toEqual([
    '/repo/bin/majordomus', 'brief', '--format', 'json', '--session', 'sess-1',
    '--since', '2026-10-29T09:40:12Z',
    '--previous-json', JSON.stringify(make().fingerprints),
  ]);
  expect(w.submitted).toHaveLength(0);
});

// Twelve hours on the mock clock are 720 one-minute ticks, each a brief read and a status
// line. At a load average of 90 on 2026-10-30 they took more than the kit's default 5 s, so
// the test failed on the machine's load and not on the mod: with 30 s it passed in 11.3 s.
const IDLE_DAY = { timeoutMs: 30_000 };
test('an idle session whose surroundings only aged is never woken', IDLE_DAY, async ($, on) => {
  const clock = mock.clock(on);
  let n = 0;
  const w = engine(on, { supervised: true, answer: () => json(later(++n)) });
  await $.session.start(start);
  await clock.advance(12 * 60 * MINUTE);
  expect(w.status.length).toBeGreaterThan(700);
  expect(w.submitted).toHaveLength(0);
});

test('a changed fingerprint wakes an idle session with report.prompt, at most once per interval', async ($, on) => {
  const clock = mock.clock(on);
  let facts: Facts = {};
  let n = 0;
  let answered: Brief = make();
  const w = engine(on, {
    supervised: true,
    answer: () => {
      answered = make(later(++n, facts));
      return JSON.stringify(answered);
    },
  });
  await $.session.start(start);
  await clock.advance(MINUTE);
  facts = { integration: '999999999999' };
  await clock.advance(3 * MINUTE);
  expect(w.submitted).toHaveLength(0);
  await clock.advance(MINUTE);
  expect(w.submitted).toHaveLength(1);
  const sent = w.submitted[0]?.text ?? '';
  expect(sent).toBe(answered.report?.prompt ?? 'no report');
  expect(sent.startsWith(MARKER)).toBe(true);
  // nothing new for an hour: nothing more
  await clock.advance(60 * MINUTE);
  expect(w.submitted).toHaveLength(1);
  // a second change, and a third a minute later that waits out the interval
  facts = { integration: '999999999999', peers: '777777777777' };
  await clock.advance(MINUTE);
  expect(w.submitted).toHaveLength(2);
  facts = { integration: '1', peers: '777777777777' };
  await clock.advance(MINUTE);
  expect(w.submitted).toHaveLength(2);
  await clock.advance(4 * MINUTE);
  expect(w.submitted).toHaveLength(3);
});

test('a change that reverts before the interval passes is not news', async ($, on) => {
  const clock = mock.clock(on);
  let facts: Facts = {};
  let n = 0;
  const w = engine(on, { supervised: true, answer: () => json(later(++n, facts)) });
  await $.session.start(start);
  facts = { peers: '777777777777' };
  await clock.advance(2 * MINUTE);
  facts = {};
  await clock.advance(30 * MINUTE);
  expect(w.submitted).toHaveLength(0);
});

test('a prompt after the interval carries report.reminder only when a fact changed', async ($, on) => {
  const clock = mock.clock(on);
  const session = mock.session(on);
  let n = 0;
  // the start event's brief, then a changed one every minute up to the 30th, whose tick fails
  // (so the interval's own report is not submitted first), then nothing readable
  const changed = later(2, { interval: 30, peers: '777777777777' });
  engine(on, {
    supervised: true,
    answer: () => {
      n += 1;
      if (n === 1) return json({ interval: 30 });
      if (n <= 30) return json(changed);
      return 'not json';
    },
  });
  await $.session.start(start);
  await clock.advance(10 * MINUTE);
  await $.prompt.submit({ text: 'early' });
  expect(session.appended()).toHaveLength(0);
  await clock.advance(20 * MINUTE + 30_000);
  await $.prompt.submit({ text: 'late' });
  const rows = session.appended();
  expect(rows).toHaveLength(1);
  const said = JSON.stringify(rows[0]?.message.content);
  expect(said).toContain(JSON.stringify(make(changed).report?.reminder).slice(1, -1));
  await clock.advance(60 * MINUTE);
  await $.prompt.submit({ text: 'again' });
  expect(session.appended()).toHaveLength(1);
});

test('a reminder read from a brief older than two refreshes is not sent', async ($, on) => {
  const clock = mock.clock(on);
  const session = mock.session(on);
  let n = 0;
  const w = engine(on, {
    supervised: true,
    answer: () => {
      n += 1;
      if (n === 1) return json({ interval: 30 });
      if (n === 2) return json(later(2, { interval: 30, peers: '777777777777' }));
      return 'not json';
    },
  });
  await $.session.start(start);
  await clock.advance(31 * MINUTE);
  await $.prompt.submit({ text: 'late' });
  expect(session.appended()).toHaveLength(0);
  // and the status line says why it is not read, and how old the last answer is
  expect(w.status.at(-1)).toBe('majordomus brief [unread]: its answer is not JSON; last answer 30 min ago');
});

test('a reminder never moves the instant Delivered is counted from', async ($, on) => {
  const clock = mock.clock(on);
  const session = mock.session(on);
  let n = 0;
  let facts: Facts = {};
  const ats: string[] = [];
  const w = engine(on, {
    supervised: true,
    answer: () => {
      n += 1;
      // the 11th read, at 10:00, fails: the person's prompt at 10:30 then gets the reminder
      if (n === 11) return 'not json';
      const b = make(later(n, facts));
      ats.push(b.at);
      return JSON.stringify(b);
    },
  });
  await $.session.start(start);
  facts = { integration: '999999999999' };
  await clock.advance(5 * MINUTE);
  // the report at 5:00 (read 6)
  expect(w.submitted).toHaveLength(1);
  const reportedAt = ats[5];
  facts = { integration: '999999999999', peers: '777777777777' };
  await clock.advance(5 * MINUTE + 30_000);
  await $.prompt.submit({ text: 'a person asks' });
  expect(session.appended()).toHaveLength(1);
  await clock.advance(MINUTE);
  // the read after the reminder still counts Delivered from the report, not the reminder
  const after = w.argvs.at(-1) ?? [];
  expect(after[after.indexOf('--since') + 1]).toBe(reportedAt);
});

test('the self-report is not briefed again on its way through the prompt hook', async ($, on) => {
  const clock = mock.clock(on);
  const session = mock.session(on);
  let facts: Facts = {};
  let n = 0;
  const w = engine(on, { supervised: true, answer: () => json(later(++n, facts)) });
  await $.session.start(start);
  facts = { peers: '777777777777' };
  await clock.advance(5 * MINUTE);
  expect(w.submitted).toHaveLength(1);
  expect(session.appended()).toHaveLength(0);
});

test('a brief that cannot be read says so on the status line and asks nothing', async ($, on) => {
  const clock = mock.clock(on);
  const w = engine(on, { supervised: true, exit: 12 });
  await $.session.start(start);
  expect(w.status.at(-1)).toBe('majordomus brief [unread]: exit 12');
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

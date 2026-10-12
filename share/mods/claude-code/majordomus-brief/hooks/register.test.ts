// Run with `claude plugin test share/mods/claude-code/majordomus-brief` (ADR 0136).
import { test, expect, mock } from 'claude-code/testing';
import type { On, PromptSubmitInput } from 'claude-code';

const MINUTE = 60_000;

const brief = (line: string, interval = 20) =>
  JSON.stringify({
    at: '2026-10-11T00:00:00Z',
    line,
    lines: ['session      episode s-1', 'peers        2 attached on 3 board(s)'],
    interval_minutes: interval,
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
        stdout: opts.answer ? opts.answer() : brief('I1 @main | peers 2/3b | mesh off | prs ?'),
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

test('the status line is the brief, refreshed on the clock, read without building', async ($, on) => {
  const clock = mock.clock(on);
  let n = 0;
  const w = engine(on, { supervised: true, answer: () => brief(`line ${++n}`) });
  await $.session.start(start);
  expect(w.status.at(-1)).toBe('line 1');
  expect(w.argvs[0]).toEqual(['/repo/bin/majordomus', 'brief', '--format', 'json', '--session', 'sess-1']);
  await clock.advance(MINUTE);
  expect(w.status.at(-1)).toBe('line 2');
  expect(w.submitted).toHaveLength(0);
});

test('an idle session is briefed once per interval, as a prompt of its own', async ($, on) => {
  const clock = mock.clock(on);
  const w = engine(on, { supervised: true, answer: () => brief('l', 5) });
  await $.session.start(start);
  await clock.advance(4 * MINUTE);
  expect(w.submitted).toHaveLength(0);
  await clock.advance(MINUTE);
  expect(w.submitted).toHaveLength(1);
  expect(w.submitted[0]?.text).toMatch(/^Periodic self-report \(majordomus-brief, ADR 0136\)\./);
  expect(w.submitted[0]?.text).toMatch(/peers {8}2 attached on 3 board\(s\)/);
  expect(w.submitted[0]?.text).toMatch(/two to four lines/);
  await clock.advance(4 * MINUTE);
  expect(w.submitted).toHaveLength(1);
  await clock.advance(MINUTE);
  expect(w.submitted).toHaveLength(2);
});

test('a prompt sent after the interval carries the last brief read, dated', async ($, on) => {
  // the refreshes after the first fail, so no self-report is queued and the prompt is the
  // only way the brief reaches the model
  const clock = mock.clock(on);
  const session = mock.session(on);
  let n = 0;
  const w = engine(on, {
    supervised: true,
    answer: () => (++n === 1 ? brief('l', 30) : 'not json'),
  });
  await $.session.start(start);
  await clock.advance(10 * MINUTE);
  await $.prompt.submit({ text: 'early' });
  expect(session.appended()).toHaveLength(0);
  await clock.advance(21 * MINUTE);
  await $.prompt.submit({ text: 'late' });
  const rows = session.appended();
  expect(rows).toHaveLength(1);
  expect(JSON.stringify(rows[0]?.message.content)).toMatch(/What surrounds this session now/);
  expect(JSON.stringify(rows[0]?.message.content)).toMatch(/2026-10-11T00:00:00Z/);
  await $.prompt.submit({ text: 'again' });
  expect(session.appended()).toHaveLength(1);
  expect(w.submitted.filter(p => p.text.startsWith('Periodic'))).toHaveLength(0);
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

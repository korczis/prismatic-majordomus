// Run with `claude plugin test share/mods/claude-code/majordomus-handover` (ADR 0123).
import { test, expect } from 'claude-code/testing';
import type { On, PromptSubmitInput } from 'claude-code';

const WINDOW = 200_000;
const measure = (tokens: number) => ({
  context: { window: WINDOW, tokens, percent: Math.round((tokens / WINDOW) * 100) },
  rateLimits: [],
  changed: ['context' as const],
});

// The test's hooks stand for the engine beneath the plugin.
const engine = (on: On, isSupervised: boolean): PromptSubmitInput[] => {
  const submitted: PromptSubmitInput[] = [];
  on('session.root', () => ({ value: '/repo' }));
  on('fs.exists', (_$, e) => ({
    value: isSupervised && e.path === '/repo/.ai/manifest.yaml',
  }));
  on('session.measure', (_$, e) => ({ changed: e.changed }));
  on('prompt.submit', (_$, e) => {
    submitted.push(e);
    return { text: e.text };
  });
  return submitted;
};

test('asks once at the line (140k of 200k), and again after the fill falls', async ($, on) => {
  const submitted = engine(on, true);

  await $.session.measure(measure(100_000));
  expect(submitted).toHaveLength(0);

  await $.session.measure(measure(141_000));
  expect(submitted).toHaveLength(1);
  expect(submitted[0]?.text).toMatch(/`majordomus handover`/);
  expect(submitted[0]?.text).toMatch(/# Next Action/);

  await $.session.measure(measure(170_000));
  expect(submitted).toHaveLength(1);

  await $.session.measure(measure(30_000));
  await $.session.measure(measure(150_000));
  expect(submitted).toHaveLength(2);
});

test('the line keeps a fixed headroom on a large window', async ($, on) => {
  const submitted = engine(on, true);
  const large = (tokens: number) => ({ ...measure(tokens), context: { window: 1_000_000, tokens } });
  await $.session.measure(large(790_000));
  expect(submitted).toHaveLength(0);
  await $.session.measure(large(800_000));
  expect(submitted).toHaveLength(1);
});

test('stays silent outside a supervised repository', async ($, on) => {
  const submitted = engine(on, false);
  await $.session.measure(measure(190_000));
  expect(submitted).toHaveLength(0);
});

test('does not ask over a handover the worker already wrote', async ($, on) => {
  const submitted = engine(on, true);
  on('tool.call', { tool: 'Bash' }, () => ({
    result: { stdout: '.ai/local/state/handovers/x.md', stderr: '', interrupted: false },
  }));
  await $.tool.call({ tool: 'Bash', command: 'majordomus handover < body.md' });
  await $.session.measure(measure(150_000));
  expect(submitted).toHaveLength(0);
});

test('a resolve is a read, and does not count as a handover written', async ($, on) => {
  const submitted = engine(on, true);
  on('tool.call', { tool: 'Bash' }, () => ({
    result: { stdout: '', stderr: '', interrupted: false },
  }));
  await $.tool.call({ tool: 'Bash', command: 'majordomus handover --resolve' });
  await $.session.measure(measure(150_000));
  expect(submitted).toHaveLength(1);
});

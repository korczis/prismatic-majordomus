// Written by `majordomus capture install` from share/mods/claude-code/majordomus-brief;
// edit it there, not here (ADR 0136).
//
// A session that is not told what surrounds it goes quiet, and a quiet session cannot be
// told from a dead one. This keeps `majordomus brief` in front of the session without
// anyone asking: its one line as the status line, refreshed on a clock, and its full form
// put before the model once per interval, which `session.brief_interval_minutes` declares
// and the brief itself carries. When the session is idle the brief arrives as a prompt of
// its own, so the session wakes, reports in a few lines and goes on.
import type { EngineInterface, Register } from 'claude-code';

// How often the status line is refreshed. A brief costs one process and, through the
// server holding the lease, about a second.
const REFRESH_MS = 60_000;
// The interval when the brief cannot be read, in minutes; the brief's own wins.
const DEFAULT_INTERVAL_MINUTES = 20;
// How long one `majordomus brief` may take before it is given up on.
const CALL_TIMEOUT_MS = 30_000;

// The first words of the prompt this mod submits, so that its own prompt is never briefed.
export const SELF_REPORT = 'Periodic self-report (majordomus-brief, ADR 0136).';

type Brief = { at: string; line: string; lines: string[]; interval_minutes: number };

// The session's state. A reload starts it over, as the new process measures anew.
let last: Brief | undefined;
let shownAt = 0;
let queued = false;
const supervised = new Map<string, boolean>();

// A repository is supervised iff `.ai/manifest.yaml` exists at its root.
const isSupervised = async ($: EngineInterface): Promise<boolean> => {
  const root = await $.session.root();
  const known = supervised.get(root);
  if (known !== undefined) return known;
  const found = await $.fs.exists(`${root}/.ai/manifest.yaml`);
  supervised.set(root, found);
  return found;
};

// The repository's own launcher first, then an installed tree's, then the PATH.
const executable = async ($: EngineInterface): Promise<string> => {
  const root = await $.session.root();
  for (const candidate of [`${root}/bin/majordomus`, `${root}/.majordomus/bin/majordomus`]) {
    if (await $.fs.exists(candidate)) return candidate;
  }
  return 'majordomus';
};

// The brief as JSON, or nothing. Never builds: a status line is not where a compiler starts.
const read = async ($: EngineInterface): Promise<Brief | undefined> => {
  try {
    const argv = [await executable($), 'brief', '--format', 'json', '--session', await $.session.id()];
    const ran = await $.process.run(argv, {
      env: { MAJORDOMUS_NO_BUILD: '1' },
      timeoutMs: CALL_TIMEOUT_MS,
    });
    if (ran.exitCode !== 0) return undefined;
    const brief = JSON.parse(ran.stdout) as Brief;
    return typeof brief.line === 'string' && Array.isArray(brief.lines) ? brief : undefined;
  } catch {
    return undefined;
  }
};

const intervalMs = (b: Brief | undefined): number =>
  Math.max(1, b?.interval_minutes ?? DEFAULT_INTERVAL_MINUTES) * 60_000;

const body = (b: Brief): string => '```\n' + b.lines.join('\n') + '\n```';

export const selfReport = (b: Brief): string =>
  `${SELF_REPORT} The brief as \`majordomus brief\` composed it at ${b.at}:\n\n${body(b)}\n\n` +
  'Report to the person in two to four lines: what this session is doing and where it ' +
  'stands, then whatever above needs attention (a board that could not be read, a ' +
  'superseded server, work the mesh addresses to you, the integration queue, the derive ' +
  'lock). Read live state before acting on any of it. Then go on with the work in hand; ' +
  'if there is none, say so and stop.';

export const reminder = (b: Brief): string =>
  `What surrounds this session now (majordomus brief, ${b.at}; ADR 0136):\n\n${body(b)}`;

// One refresh: the status line, and the self-report when the interval has passed.
const tick = async ($: EngineInterface): Promise<void> => {
  const b = await read($);
  if (b === undefined) {
    $.ui.status('majordomus brief: not answered');
    return;
  }
  last = b;
  $.ui.status(b.line);
  const now = await $.clock.now();
  if (!queued && now - shownAt >= intervalMs(b)) {
    queued = true;
    shownAt = now;
    // Queued: it starts a turn of its own once the session is idle, never inside a running one.
    void $.prompt
      .submit({ text: selfReport(b) })
      .catch(() => undefined)
      .finally(() => {
        queued = false;
      });
  }
};

export const register: Register = on => {
  on('session.start', async ($, e, next) => {
    const started = await next(e);
    if (await isSupervised($)) {
      // The start event's hook has just briefed the session and told it how to keep awake;
      // the first self-report is owed one interval from now.
      shownAt = await $.clock.now();
      await tick($);
      $.clock.every(REFRESH_MS, () => void tick($));
    }
    return started;
  });

  // A prompt the person sends after the interval has passed carries the brief with it, read
  // from the last refresh so that the prompt never waits for a process.
  on('prompt.submit', async ($, e, next) => {
    const now = await $.clock.now();
    if (
      last !== undefined &&
      !e.text.startsWith(SELF_REPORT) &&
      now - shownAt >= intervalMs(last) &&
      (await isSupervised($))
    ) {
      shownAt = now;
      await $.session.append({ message: { type: 'user', content: [{ type: 'text', text: reminder(last) }] } });
    }
    return next(e);
  }).catch(($, e, next) => next(e));
};

// Written by `majordomus capture install` from share/mods/claude-code/majordomus-brief;
// edit it there, not here (ADR 0136).
//
// A session that is not told what surrounds it goes quiet, and a quiet session cannot be
// told from a dead one. This keeps `majordomus brief` in front of the session without
// anyone asking: its one line as the status line, refreshed on a clock, and the report the
// session owes put before the model at most once per interval, and only when a fact in the
// brief changed since the session was last told.
//
// The mod is a relay. What the report holds is declared once, in share/brief/report.yaml;
// `brief.session` composes from it the prompt (`report.prompt`) and the reminder
// (`report.reminder`), and this module submits and appends them verbatim. What counts as a
// change is the brief's own per-section fingerprints, of its facts with every instant and
// age taken out, so nothing here reads the brief's prose.
import type { EngineInterface, Register } from 'claude-code';

// How often the status line is refreshed. A brief costs one process and, through the
// server holding the lease, a few seconds at most: every section is bounded.
const REFRESH_MS = 60_000;
// The interval when the brief cannot be read, in minutes; the brief's own wins. A projection
// of DEFAULT_INTERVAL_MINUTES in apps/majordomus-cli/src/capability/builtin/brief.rs.
const DEFAULT_INTERVAL_MINUTES = 20;
// How long one `majordomus brief` may take before it is given up on.
const CALL_TIMEOUT_MS = 30_000;

// The first words of the self-report an earlier release of this mod submitted, and of the
// one it submits for an executable older than the report contract: never briefed again.
export const LEGACY_MARKER = 'Majordomus brief: since the last report,';

export type Brief = {
  at: string;
  line: string;
  lines: string[];
  interval_minutes: number;
  attention?: { id: string; section: string; word: string; fact: string; remedy: string; actor: string }[];
  report?: {
    contract: string;
    fingerprint: string;
    marker: string;
    prompt: string;
    reminder: string;
    since?: string | null;
  };
  fingerprints?: Record<string, string>;
};

// ---------------------------------------------------------------- what counts as a change

// An executable older than the per-section fingerprints is compared by its lines with the
// clock taken out: every instant and every span of time.
const INSTANT = /\b\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:?\d{2})?/g;
const UNIT = '(?:s|secs?|seconds?|m|mins?|minutes?|h|hrs?|hours?|d|days?|w|wks?|weeks?)';
const SPAN = `\\d{1,5}\\s?${UNIT}`;
const DURATION = new RegExp(`\\b${SPAN}(?:,?\\s${SPAN})*\\b`, 'g');
const legacyText = (b: Brief): string =>
  b.lines
    .filter(l => !l.startsWith('answered by'))
    .join('\n')
    .replace(INSTANT, '<at>')
    .replace(DURATION, '<age>')
    .replace(/[ \t]+/g, ' ');

/** One string per brief, which differs exactly when a fact in the brief does. */
export const fingerprint = (b: Brief): string =>
  b.fingerprints
    ? Object.keys(b.fingerprints)
        .sort()
        .map(k => `${k}=${b.fingerprints?.[k]}`)
        .join('\n')
    : legacyText(b);

/** The sections whose fingerprint differs between two briefs. */
export const changedSections = (before: Brief, after: Brief): string[] => {
  if (!before.fingerprints || !after.fingerprints) {
    return fingerprint(before) === fingerprint(after) ? [] : ['brief'];
  }
  const names = [...new Set([...Object.keys(after.fingerprints), ...Object.keys(before.fingerprints)])];
  return names.filter(k => before.fingerprints?.[k] !== after.fingerprints?.[k]);
};

// ---------------------------------------------------------------- what is submitted

const body = (b: Brief): string => '```\n' + b.lines.join('\n') + '\n```';

/**
 * The prompt an idle session is woken with: the report the brief owes, verbatim. An
 * executable older than the report contract carries none, and is said to.
 */
export const selfReport = (b: Brief): string =>
  b.report?.prompt ||
  `${LEGACY_MARKER} this executable predates brief-report/v1, so its brief carries no ` +
    `report; its lines follow.\n\n${body(b)}`;

/** The row a prompt the person sends carries when the brief changed: verbatim. */
export const reminder = (b: Brief): string =>
  b.report?.reminder || `Majordomus brief ${b.at} (this executable predates brief-report/v1)\n\n${body(b)}`;

/** Whether a prompt is one this mod submitted, whichever executable composed it. */
export const isOwn = (text: string, b: Brief | undefined): boolean =>
  text.startsWith(LEGACY_MARKER) || (!!b?.report?.marker && text.startsWith(b.report.marker));

// ---------------------------------------------------------------- the session

// The session's state. A reload starts it over, as the new process measures anew.
let last: Brief | undefined;
// The brief the session was last told, by the start event or by this mod, and when.
let told: Brief | undefined;
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
// The instant the session was last told goes with it, so that the report says what was
// delivered since. The provider's background tasks are not passed: this engine does not
// expose them, and the brief says so rather than this module guessing.
const read = async ($: EngineInterface): Promise<Brief | undefined> => {
  try {
    const argv = [
      await executable($),
      'brief',
      '--format',
      'json',
      '--session',
      await $.session.id(),
    ];
    if (told !== undefined) argv.push('--since', told.at);
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

// Whether the session is owed the report: once the interval has passed since it was last
// told, and only when a section's fingerprint changed. With nothing told yet, the brief read
// now is what the start event's briefing said, and becomes what was told.
const owed = (b: Brief, now: number): boolean => {
  if (told === undefined) {
    told = b;
    return false;
  }
  return now - shownAt >= intervalMs(b) && changedSections(told, b).length > 0;
};

// One refresh: the status line, and the report when it is owed.
const tick = async ($: EngineInterface): Promise<void> => {
  const b = await read($);
  if (b === undefined) {
    $.ui.status('majordomus brief: not answered');
    return;
  }
  last = b;
  $.ui.status(b.line);
  const now = await $.clock.now();
  if (!queued && owed(b, now)) {
    const before = told as Brief;
    queued = true;
    told = b;
    shownAt = now;
    // Queued: it starts a turn of its own once the session is idle, never inside a running one.
    void $.prompt
      .submit({ text: selfReport(b) })
      .catch(() => {
        // not delivered, so not told: the change is owed again at the next interval
        if (told === b) told = before;
      })
      .finally(() => {
        queued = false;
      });
  }
};

export const register: Register = on => {
  on('session.start', async ($, e, next) => {
    const started = await next(e);
    if (await isSupervised($)) {
      // The start event's hook has just briefed the session, so what it said is what was
      // told; the first report is owed no sooner than one interval from now.
      told = undefined;
      last = undefined;
      shownAt = await $.clock.now();
      await tick($);
      $.clock.every(REFRESH_MS, () => void tick($));
    }
    return started;
  });

  // A prompt the person sends once the interval has passed carries the reminder when a fact
  // in the brief changed, read from the last refresh so that the prompt never waits for a
  // process.
  on('prompt.submit', async ($, e, next) => {
    const now = await $.clock.now();
    if (last !== undefined && !isOwn(e.text, last) && (await isSupervised($)) && owed(last, now)) {
      told = last;
      shownAt = now;
      await $.session.append({
        message: { type: 'user', content: [{ type: 'text', text: reminder(last) }] },
      });
    }
    return next(e);
  }).catch(($, e, next) => next(e));
};

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
  at_local?: string;
  line: string;
  lines: string[];
  interval_minutes: number;
  attention?: {
    id: string;
    section: string;
    report_section?: string;
    word: string;
    fact: string;
    remedy: string;
    actor: string;
  }[];
  report?: {
    contract: string;
    fingerprint: string;
    marker: string;
    prompt: string;
    reminder: string;
    since?: string | null;
    changed?: string[] | null;
  };
  fingerprints?: Record<string, string>;
};

// What a section read past its budget carries for a fingerprint: its facts were not read,
// so it is never said to have changed (LATE in brief.rs).
const LATE = 'late';

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
  return names.filter(k => {
    const [b, a] = [before.fingerprints?.[k], after.fingerprints?.[k]];
    return b !== LATE && a !== LATE && b !== a;
  });
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
// When `last` was read, on the engine's clock.
let lastAt = 0;
// Why the latest read failed, when it did; cleared by the next read that answers.
let failure: string | undefined;
// The brief the session was last told, by the start event, a report or a reminder, and when:
// what the change test compares against.
let told: Brief | undefined;
let shownAt = 0;
// The instant of the last report this mod submitted: what `--since` names, so that a
// reminder (which asks for no report) never moves the start of what "Delivered" covers.
let reportedAt: string | undefined;
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

// The brief as JSON, or why it was not read. Never builds: a status line is not where a
// compiler starts. The instant of the last report goes with it, so that the report says
// what was delivered since, and the fingerprints of the brief the session was last told, so
// that the report says which sections changed and which are unchanged. The provider's
// background tasks are not passed: this engine does not expose them, and the brief says so
// rather than this module guessing.
type Read = { brief: Brief } | { why: string };
const read = async ($: EngineInterface): Promise<Read> => {
  try {
    const argv = [
      await executable($),
      'brief',
      '--format',
      'json',
      '--session',
      await $.session.id(),
    ];
    if (reportedAt !== undefined) argv.push('--since', reportedAt);
    if (told?.fingerprints !== undefined) argv.push('--previous-json', JSON.stringify(told.fingerprints));
    const ran = await $.process.run(argv, {
      env: { MAJORDOMUS_NO_BUILD: '1' },
      timeoutMs: CALL_TIMEOUT_MS,
    });
    if (ran.exitCode !== 0) {
      const first = (ran.stderr ?? '').split('\n').find(l => l.trim() !== '')?.trim();
      return { why: `exit ${ran.exitCode}${first ? ` — ${first}` : ''}` };
    }
    let brief: Brief;
    try {
      brief = JSON.parse(ran.stdout) as Brief;
    } catch {
      return { why: 'its answer is not JSON' };
    }
    return typeof brief.line === 'string' && Array.isArray(brief.lines)
      ? { brief }
      : { why: 'its answer is not a brief' };
  } catch (e) {
    const text = e instanceof Error ? e.message : String(e);
    return {
      why: /time/i.test(text) ? `timed out after ${CALL_TIMEOUT_MS / 1000} s` : text || 'it did not run',
    };
  }
};

// `45 s`, `17 min`, `3 h`: how long ago, as the brief says it.
const ago = (ms: number): string => {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 120) return `${s} s`;
  if (s < 7200) return `${Math.floor(s / 60)} min`;
  return `${Math.floor(s / 3600)} h`;
};

const intervalMs = (b: Brief | undefined): number =>
  Math.max(1, b?.interval_minutes ?? DEFAULT_INTERVAL_MINUTES) * 60_000;

// Whether the session is owed the report: once the interval has passed since it was last
// told, and only when a section's fingerprint changed. With nothing told yet, the brief read
// now is what the start event's briefing said, and becomes what was told.
const owed = (b: Brief, now: number): boolean => {
  if (told === undefined) {
    // what the start event said is the first report: what is delivered counts from it
    told = b;
    reportedAt ??= b.at;
    return false;
  }
  return now - shownAt >= intervalMs(b) && changedSections(told, b).length > 0;
};

// One refresh: the status line, and the report when it is owed. A read that fails says why
// and how old the last answer is, never only "not answered".
const tick = async ($: EngineInterface): Promise<void> => {
  const r = await read($);
  const now = await $.clock.now();
  if ('why' in r) {
    failure = r.why;
    $.ui.status(
      last === undefined
        ? `majordomus brief [unread]: ${r.why}`
        : `majordomus brief [unread]: ${r.why}; last answer ${ago(now - lastAt)} ago`,
    );
    return;
  }
  const b = r.brief;
  last = b;
  lastAt = now;
  failure = undefined;
  $.ui.status(b.line);
  if (!queued && owed(b, now)) {
    const [before, beforeAt] = [told as Brief, shownAt];
    queued = true;
    told = b;
    shownAt = now;
    // Queued: it starts a turn of its own once the session is idle, never inside a running one.
    void $.prompt
      .submit({ text: selfReport(b) })
      .then(() => {
        // a report was made: what it delivered is counted from here on
        reportedAt = b.at;
      })
      .catch(() => {
        // not delivered, so not told: the change is owed again at the next interval
        if (told === b) [told, shownAt] = [before, beforeAt];
      })
      .finally(() => {
        queued = false;
      });
  }
};

// A reminder read from a refresh older than two refreshes is not today's: it is not sent.
const fresh = (now: number): boolean => lastAt > 0 && now - lastAt <= 2 * REFRESH_MS;

export const register: Register = on => {
  on('session.start', async ($, e, next) => {
    const started = await next(e);
    if (await isSupervised($)) {
      // The start event's hook has just briefed the session, so what it said is what was
      // told; the first report is owed no sooner than one interval from now.
      told = undefined;
      last = undefined;
      lastAt = 0;
      failure = undefined;
      reportedAt = undefined;
      shownAt = await $.clock.now();
      await tick($);
      $.clock.every(REFRESH_MS, () => void tick($));
    }
    return started;
  });

  // A prompt the person sends once the interval has passed carries the reminder when a fact
  // in the brief changed, read from the last refresh so that the prompt never waits for a
  // process, and only while that refresh is fresh: a reminder from a brief many minutes old
  // would be presented as now. A reminder asks for no report, so `reportedAt` stays.
  on('prompt.submit', async ($, e, next) => {
    const now = await $.clock.now();
    if (
      last !== undefined &&
      !isOwn(e.text, last) &&
      fresh(now) &&
      (await isSupervised($)) &&
      owed(last, now)
    ) {
      told = last;
      shownAt = now;
      await $.session.append({
        message: { type: 'user', content: [{ type: 'text', text: reminder(last) }] },
      });
    }
    return next(e);
  }).catch(($, e, next) => next(e));
};

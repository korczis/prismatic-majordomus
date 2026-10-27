// Written by `majordomus capture install` from share/mods/claude-code/majordomus-brief;
// edit it there, not here (ADR 0136).
//
// A session that is not told what surrounds it goes quiet, and a quiet session cannot be
// told from a dead one. This keeps `majordomus brief` in front of the session without
// anyone asking: its one line as the status line, refreshed on a clock, and its full form
// put before the model at most once per interval (`session.brief_interval_minutes`, carried
// by the brief), and only when a fact in it changed since the session was last told. When
// the session is idle that brief arrives as a prompt of its own, so the session wakes, says
// what the change means for its work and goes on. A brief in which only the clock moved
// (its `at`, "observed 4 min ago", "for 2 h 5 min") is not news and wakes nobody.
import type { EngineInterface, Register } from 'claude-code';

// How often the status line is refreshed. A brief costs one process and, through the
// server holding the lease, about a second.
const REFRESH_MS = 60_000;
// The interval when the brief cannot be read, in minutes; the brief's own wins.
const DEFAULT_INTERVAL_MINUTES = 20;
// How long one `majordomus brief` may take before it is given up on.
const CALL_TIMEOUT_MS = 30_000;

// The first words of the prompt this mod submits, so that its own prompt is never briefed.
export const SELF_REPORT = 'Majordomus brief: since the last report,';

export type Brief = {
  at: string;
  line: string;
  lines: string[];
  interval_minutes: number;
  session?: { branch?: string; head?: string };
  release?: { stance?: string | null };
};

// ---------------------------------------------------------------- what counts as a change

// An instant: the brief's `at`, and the `(2026-10-27T10:10:51Z)` an observation is dated by.
const INSTANT = /\b\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:?\d{2})?/g;
// A span of time, alone or chained: "38 h", "21 h 44 min", "3 d 2 h", "30 s", "(16s)", "36m".
// At most five digits, so that a short commit id that happens to end in `d` is never one.
const UNIT = '(?:s|secs?|seconds?|m|mins?|minutes?|h|hrs?|hours?|d|days?|w|wks?|weeks?)';
const SPAN = `\\d{1,5}\\s?${UNIT}`;
const DURATION = new RegExp(`\\b${SPAN}(?:,?\\s${SPAN})*\\b`, 'g');

// A brief's section label, as `compose` writes it: a word padded to the thirteenth column.
const LABELLED = /^(answered by|[a-z]+)\s{2,}(.*)$/;

/**
 * What a line of the brief says once the clock is taken out of it: every instant and every
 * span of time becomes a placeholder, and whitespace is folded.
 */
export const normalise = (text: string): string =>
  text.replace(INSTANT, '<at>').replace(DURATION, '<age>').replace(/\s+/g, ' ').trim();

/**
 * The brief's sections, each as the facts it states: the label of a line, else the section
 * the line continues (the peers it names). `answered by` keeps only which process composed
 * the brief; why another did not answer (an errno, a timeout) is not a fact about the
 * session's surroundings.
 */
export const sections = (b: Brief): Map<string, string> => {
  const out = new Map<string, string>();
  let current = 'brief';
  for (const raw of b.lines) {
    const m = /^\S/.test(raw) ? LABELLED.exec(raw) : null;
    let text = raw;
    if (m) {
      current = m[1] ?? current;
      text = m[2] ?? '';
      if (current === 'answered by') text = text.split(/\s/)[0] ?? '';
    }
    const before = out.get(current);
    out.set(current, (before === undefined ? '' : `${before}\n`) + normalise(text));
  }
  return out;
};

/** One string per brief, which differs exactly when a fact in the brief does. */
export const fingerprint = (b: Brief): string =>
  [...sections(b)].map(([k, v]) => `${k}=${v}`).join('\n');

/** The sections whose facts differ between two briefs, in the order the newer one lists them. */
export const changedSections = (before: Brief, after: Brief): string[] => {
  const a = sections(before);
  const b = sections(after);
  const names = [...b.keys(), ...[...a.keys()].filter(k => !b.has(k))];
  return names.filter(k => a.get(k) !== b.get(k));
};

const short = (sha: string): string => sha.slice(0, 7);
const first = (text: string | undefined, re: RegExp): string | undefined =>
  text === undefined ? undefined : re.exec(text)?.[1];
const count = (n: number, one: string, many: string): string =>
  n === 1 ? one : `${n} ${many}`;

// A section's change in the words a person reads it in; the section's name when no closer
// reading applies.
const describe = (name: string, before: Brief, after: Brief): string[] => {
  const was = sections(before).get(name);
  const now = sections(after).get(name);
  switch (name) {
    case 'session': {
      const said: string[] = [];
      const head = (b: Brief, s?: string) =>
        b.session?.head ?? first(s, / at ([0-9a-f]{7,40})\b/);
      const branch = (b: Brief, s?: string) => b.session?.branch ?? first(s, / on (\S+) at /);
      const [b0, b1] = [branch(before, was), branch(after, now)];
      const [h0, h1] = [head(before, was), head(after, now)];
      if (b0 !== b1 && b1 !== undefined) said.push(`the session moved to ${b1}`);
      else if (h0 !== h1 && h0 !== undefined && h1 !== undefined)
        said.push(`${b1 ?? 'the branch'} moved ${short(h0)} → ${short(h1)}`);
      const task = (s?: string) => first(s, /; task (\S+?):/) ?? 'none';
      if (task(was) !== task(now))
        said.push(task(now) === 'none' ? 'no task is active now' : `the task is now ${task(now)}`);
      const episode = (s?: string) => first(s, /^episode (\S+)/);
      if (said.length === 0 && episode(was) !== episode(now)) said.push('a new episode began');
      return said.length > 0 ? said : ['the session changed'];
    }
    case 'peers': {
      const attached = (s?: string) => Number(first(s, /^(\d+) attached/) ?? NaN);
      const d = attached(now) - attached(was);
      if (d > 0) return [count(d, 'a peer attached', 'peers attached')];
      if (d < 0) return [count(-d, 'a peer left', 'peers left')];
      return ['what the peers announced changed'];
    }
    case 'integration': {
      const open = (s?: string) => first(s, /: (\d+) open\b/);
      const [o0, o1] = [open(was), open(now)];
      if (o0 !== undefined && o1 !== undefined && o0 !== o1)
        return [`open pull requests went from ${o0} to ${o1}`];
      const lanes = (s?: string) => first(s, / open — ([^;]*)/);
      if (lanes(was) !== lanes(now)) return ['integration lanes changed'];
      const next = (s?: string) => first(s, /(next merge #\d+|nothing ready)/);
      const n1 = next(now);
      if (next(was) !== n1 && n1 !== undefined) {
        // "next merge #2565" names the pull request that merges next
        return [n1 === 'nothing ready' ? 'nothing is ready to merge now' : `${n1.slice(11)} is next`];
      }
      // the trunk the queue was observed at; the same words as the session's own move, which
      // `whatChanged` then says once
      const trunk = / ?(\S+) at ([0-9a-f]{7,40}) observed/;
      const [t0, t1] = [trunk.exec(was ?? ''), trunk.exec(now ?? '')];
      const rest = (s?: string) => (s ?? '').replace(trunk, ' observed');
      if (t0 && t1 && t0[2] !== t1[2] && rest(was) === rest(now))
        return [`${t1[1]} moved ${short(t0[2] ?? '')} → ${short(t1[2] ?? '')}`];
      return ['the integration queue changed'];
    }
    case 'release': {
      const [s0, s1] = [before.release?.stance, after.release?.stance];
      if (s0 && s1 && s0 !== s1) return [`the release went from ${s0} to ${s1}`];
      return ['the release changed'];
    }
    case 'mesh': {
      const said: string[] = [];
      const links = (s?: string) => first(s, /^(\d+ of \d+) link\(s\) connected/);
      if (links(was) !== links(now) && links(now) !== undefined)
        said.push(`${links(now)} mesh links are connected`);
      const claims = (s?: string) => first(s, /(\d+) claim\(s\) held/);
      if (claims(was) !== claims(now) && claims(was) !== undefined && claims(now) !== undefined)
        said.push(`mesh claims went from ${claims(was)} to ${claims(now)}`);
      const addressed = (s?: string) => first(s, /(for this runtime: .*)$/);
      if (addressed(was) !== addressed(now) && addressed(now) !== undefined)
        said.push(`the mesh now has ${addressed(now)?.slice(18)} for this session`);
      return said.length > 0 ? said : ['the mesh changed'];
    }
    case 'derive':
      return [now?.startsWith('lock free') ? 'the derive lock was freed' : 'the derive lock changed'];
    case 'answered by':
      return ['another process answers the brief'];
    default:
      return [`${name} changed`];
  }
};

/** What changed between two briefs, as one plain clause: "main moved b1be080 → d62c13a". */
export const whatChanged = (before: Brief, after: Brief): string => {
  const said = [
    ...new Set(changedSections(before, after).flatMap(name => describe(name, before, after))),
  ];
  if (said.length <= 1) return said[0] ?? 'nothing changed';
  return `${said.slice(0, -1).join(', ')} and ${said.at(-1)}`;
};

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

/**
 * The prompt an idle session is woken with. The person reads it in their transcript, so it
 * opens with what changed, in one sentence, then the brief; the one line after it is the
 * agent's, and says so.
 */
export const selfReport = (before: Brief, after: Brief): string =>
  `${SELF_REPORT} ${whatChanged(before, after)}.\n\n${body(after)}\n\n` +
  'To the agent: tell the person in two to four lines what this changes for the work in ' +
  'hand, reading live state before acting on it, then carry on or stop.';

/** The row a prompt the person sends carries when the brief changed since it was last told. */
export const reminder = (before: Brief, after: Brief): string =>
  `Majordomus brief at ${after.at}, changed since this session was last told: ` +
  `${whatChanged(before, after)} (ADR 0136).\n\n${body(after)}`;

// Whether the session is owed the brief: once the interval has passed since it was last
// told, and only when a fact in it changed. With nothing told yet, the brief read now is
// what the start event's briefing said, and becomes what was told.
const owed = (b: Brief, now: number): boolean => {
  if (told === undefined) {
    told = b;
    return false;
  }
  return now - shownAt >= intervalMs(b) && fingerprint(b) !== fingerprint(told);
};

// One refresh: the status line, and the self-report when it is owed.
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
      .submit({ text: selfReport(before, b) })
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
      // told; the first self-report is owed no sooner than one interval from now.
      told = undefined;
      last = undefined;
      shownAt = await $.clock.now();
      await tick($);
      $.clock.every(REFRESH_MS, () => void tick($));
    }
    return started;
  });

  // A prompt the person sends once the interval has passed carries the brief when a fact in it
  // changed, read from the last refresh so that the prompt never waits for a process.
  on('prompt.submit', async ($, e, next) => {
    const now = await $.clock.now();
    if (
      last !== undefined &&
      !e.text.startsWith(SELF_REPORT) &&
      (await isSupervised($)) &&
      owed(last, now)
    ) {
      const before = told as Brief;
      told = last;
      shownAt = now;
      await $.session.append({
        message: { type: 'user', content: [{ type: 'text', text: reminder(before, last) }] },
      });
    }
    return next(e);
  }).catch(($, e, next) => next(e));
};

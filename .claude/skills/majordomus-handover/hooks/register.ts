// Written by `majordomus capture install` from share/mods/claude-code/majordomus-handover;
// edit it there, not here (ADR 0123).
//
// The provider's PreCompact event fires when the conversation is already about to be
// folded: a checkpoint is derived there because the worker is not asked anything at that
// moment and could not answer if it were. This asks earlier, while the window still has
// room for the writing turn, so the record the next worker resumes from is the worker's
// own account of the live state rather than a derivation from the ledger alone.
import type { EngineInterface, Register } from 'claude-code';

// The request is made when the window has this much room left, or at this fill, whichever
// comes first: the writing turn costs a fixed number of tokens (git state, `majordomus
// check`, the body), not a share of the window.
const HEADROOM_TOKENS = 60_000;
const MAX_FILL = 0.8;

// It re-arms once the fill falls below this share of the line again: after a compaction
// or a /clear the window is new, and so is the reason to ask.
const REARM_SHARE = 0.5;

const WRITES_HANDOVER = /\bmajordomus\s+handover\b/;
const READS_HANDOVER = /--(resolve|list|help)\b/;

// The window's state. A reload starts it over, as the new process measures anew.
let asked = false;
let written = false;
const supervised = new Map<string, boolean>();

const lineFor = (window: number): number =>
  Math.min(Math.round(window * MAX_FILL), window - HEADROOM_TOKENS);

// A repository is supervised iff `.ai/manifest.yaml` exists at its root.
const isSupervised = async ($: EngineInterface): Promise<boolean> => {
  const root = await $.session.root();
  const known = supervised.get(root);
  if (known !== undefined) return known;
  const found = await $.fs.exists(`${root}/.ai/manifest.yaml`);
  supervised.set(root, found);
  return found;
};

const request = (percent: number, tokens: number, window: number): string =>
  `The context window is ${percent}% full (${tokens} of ${window} tokens) and will be ` +
  'compacted. Write a Majordomus handover now, before compaction discards what this ' +
  'session knows. Read the live state first, never from memory: `git status -sb`, ' +
  '`git log --oneline -8`, `majordomus check`. Then pipe a body with the non-empty ' +
  'level-one headings `# Objective`, `# Current State` and `# Next Action` to ' +
  '`majordomus handover` on stdin (add `--no-task` only if it refuses for want of an ' +
  'active task). Current State names the branch, the head commit, what is uncommitted ' +
  'or unpushed and what is still running; Next Action is the one step to take first. ' +
  'Record a durable decision with `majordomus decision`, not in the handover. Report ' +
  'the path it printed and stop: start nothing new.';

export const register: Register = on => {
  on('session.measure', async ($, e, next) => {
    const { tokens, percent, window } = e.context;
    if (tokens === undefined) return next(e);

    const line = lineFor(window);
    if (tokens < line * REARM_SHARE) {
      asked = false;
      written = false;
    }

    if (!asked && !written && tokens >= line && (await isSupervised($))) {
      asked = true;
      const shown = percent ?? Math.round((tokens / window) * 100);
      $.ui.toast(`context at ${shown}%: asking for a majordomus handover`);
      // Queued: it starts a turn of its own once the session is idle, never inside the
      // running one. A turn that runs into compaction first is covered by PreCompact.
      void $.prompt.submit({ text: request(shown, tokens, window) });
    }

    return next(e);
  });

  // A handover the worker wrote on its own in this window answers the question already.
  // It only observes: a failure here passes the call on, and never refuses it.
  on('tool.call', { tool: 'Bash' }, async ($, e, next) => {
    const ran = await next(e);
    const isWrite = WRITES_HANDOVER.test(e.command) && !READS_HANDOVER.test(e.command);
    if (isWrite && ran.deny === undefined && ran.isError !== true) written = true;
    return ran;
  }).catch(($, e, next) => next(e));
};

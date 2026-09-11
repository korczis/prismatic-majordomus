# A captured prompt names the episode it belongs to or records that none could be resolved; the archive is private, bounded and free of credential material

## What it means

Every record under `.ai/local/prompts/` says which execution episode it belongs to, and says how it knows. Where no episode can be established, it says that instead — it is never attached to whichever episode happened to be nearest in time. The archive is readable by its owner alone, directory included; it is bounded by a declared retention policy that takes prompt bodies and never records; and the credential shapes the writer knows are replaced before anything reaches the disk.

## The defect this closes

A record carried the provider's own session identifier — a UUID that means nothing outside that provider — and nothing else that could place it. Turning it into the canonical episode id needed `.ai/local/state/sessions-open/<provider session>.yaml`, and closing the episode deletes that file.

The consequence is not a gap that widens slowly. It is total and immediate: the mapping exists while the episode is open and is gone the instant it closes, so every prompt of that episode becomes unattributable at once. Measured in this repository on 2026-09-11, before the change: 979 records, 80 distinct provider sessions, 20 surviving session contexts, and 713 prompts with no path to an episode. Alongside it, the archive was 20 MB with no bound of any kind, every file was mode 0644 while the session state beside it had always been 0600, and a prompt containing a credential was persisted verbatim and kept.

## How it works

**Identity, written while it is still true.** `mj_capture_write` resolves the episode through `mj_open_session_id` with `MJ_SESSION_KEY` set from the payload's own provider session. That is the strict form of the resolver the rest of the tool uses: that episode or none. The resolver's other two steps fall back to the environment and then to this checkout's pointer, and either would hand a prompt the episode of whichever window opened one last. `repository_id` and `worktree_id` are `mj_repository_id` and `mj_worktree_id` — the identity the rest of the tool already computes, not a second notion of it.

**Provenance is a field, not a convention.** `episode_link` is one of five words. `open` means the episode was open under that provider session when the prompt arrived: an observation. `session-context` and `ledger` mean the link was made afterwards from evidence that survived the close: an inference, and the reader is told which. `orphan` means the capture looked and found no episode; `unlinked-legacy` means the record predates the field. The last two carry no episode. A reader that cannot tell an observation from an inference cannot tell what the archive proves.

**`capture reconcile` links only on evidence.** Three tiers, in order: the open-episode store; the frozen working context under `.ai/local/session-contexts/`, which names the provider session that opened an episode and is not deleted at the close; and the episode windows, where exactly one unclaimed closed episode of this worktree contains the whole span of that provider session's prompts. There is no fourth tier and in particular no "nearest episode in time". A prompt attributed to the wrong episode is worse than one attributed to none: the second is visibly missing, the first is quietly false. What the third tier refuses is what makes it usable — it declines the moment two episodes could both be the answer.

**Retention takes the body and keeps the record.** The old rule was that nothing may ever be removed, on the argument that a prompt is derived from nothing and no other file can reconstruct it. That argument is about the record — that it happened, when, in which episode, under which head — and it was being applied to the bytes of the text, which is the part that carries the credentials and whose risk does not decay with its value. `prompts.retention_max_days` and `prompts.retention_max_bytes` are declared in the policy; `majordomus capture prune` applies them, age first and then size, oldest body first. A pruned record keeps every field including the provenance of its episode link, and gains a tombstone: when the body went, how long it was, and its digest. A record is still never deleted, and nothing prunes as a side effect of the hook that was supposed to be keeping them.

**Redaction happens before persistence.** A declared table of credential shapes — provider API keys, GitHub tokens and PATs, AWS access key ids, Google and Slack and Stripe tokens, PEM private-key headers, bearer tokens, and an assignment of a long opaque value to something called a key, a token or a password — is substituted into the raw JSON span on its way to the record. There is therefore no moment at which the original bytes were on disk: not in the record, not in either rendering, not in the file name, because the slug is taken from the redacted text, and not in the diagnostic log, which prints a payload's key names and never a value. What was replaced is recorded in `redacted`, so that "nothing was found" and "the secret is gone" are statements a reader can tell apart.

**0600, directory included.** The file names are the openings of the prompts, so a world-readable directory discloses what was asked even while it is empty. `init` creates the archive 0700, `update` converges an existing one, every write sets the file's mode, and `capture render` — the archive's repair command, and the one the finding names — sets both over the whole directory on every run.

## How to see it

```bash
majordomus capture reconcile --dry-run   # what evidence survives, and for how many records
majordomus capture reconcile             # links what it can; marks the rest, guesses at none
majordomus capture prune --dry-run       # which bodies the two bounds would take
majordomus doctor                        # OK prompts — n record(s) each name their episode
                                         #    or say why they cannot: x linked, y unlinked
```

## What it does not cover

It does not recover a link that no evidence supports. In this repository 713 of 979 records stay `unlinked-legacy`, and the validator reports that count rather than rounding it to zero: a check that claimed a fully linked archive would be asserting exactly what the migration deliberately refused to invent.

Redaction covers shapes that are credentials by construction — a token whose own prefix says what it is — plus one assignment form. It is not a general secret scanner and does not try to be: a heuristic that redacted anything secret-looking would eat the prompts people most need to read back, and an archive that mangles ordinary text is one nobody trusts. A credential in a shape the table does not know is still persisted, and the table is data, so adding one is a line.

Pruning is a command a person runs, like `history --rotate` before it. `doctor` reports an archive over either bound and names the repair; nothing shrinks the archive behind anyone's back.

## Why it exists

An archive of a thousand prompts that cannot say which episode any of them belonged to is a heap of text with timestamps. The continuity subsystem's whole claim is that a worker's sitting is a record with a beginning, an end and the work between them, and prompts are the one part of that record the model cannot write for itself. Making them joinable forever is what makes the rest of the subsystem true; making them private and bounded is what makes keeping them defensible.

# The session subsystem can be watched, not only used: every open episode, what cannot close itself, and which commit the process is answering about

## What it means

Reading the lifecycle back is two different questions with two different answers, and until this existed only one of them had a surface. A worker asks *what am I resuming from?* and `continuity.state` answers it: one episode — the one `.ai/local/state/session-current.yaml` resolves to — one handover, one checkpoint, the blockers. An operator asks *is this subsystem working?* and that question is structurally unanswerable from the first, because the pointer is a symlink that the most recent start event re-aims. `lifecycle.episodes`, `lifecycle.recovery`, `lifecycle.runtime`, `lifecycle.providers` and `lifecycle.closed` read the store rather than the pointer and answer the second question, over MCP, over `GET /api/v1/lifecycle/<name>`, and on the Cockpit's Continuity page, which holds no session model of its own and is a projection of exactly these five and `continuity.state`.

## How it works

`lifecycle.episodes` lists `.ai/local/state/sessions-open/*.yaml` — one file per provider session — and gives each a standing decided from what the repository can observe: `current` when the pointer resolves to it, `open` when it names this worktree and the pointer names another episode, `foreign` when it names another worktree, `stranded` when the worktree it opened in is gone from disk and nothing can compose its record any more. Every episode carries a note saying which and why, and the ledger is joined to it by the episode id each line is stamped with — never by a time range, because one ledger holds several workers and no timestamp separates them. The same join gives the task relation, and it runs from the episode outwards: an episode names the tasks its lines touched, and no task decides whether the episode's own records are written.

`lifecycle.recovery` reports what nobody can fix by running the tool normally: an open record whose worktree has vanished or which the ledger already recorded as closed, with the command that clears it; the `.tmp.` files a close killed between writing and renaming leaves in the tracked sessions section, where they are untracked, invisible to every reader of the section and offered to the next `git add .`; whether `session-current.yaml` is a symlink into the per-provider store or the regular file of the layout that kept one episode per checkout; and the arithmetic ADR 0041 asks for — `session.started` against `session.closed` against the open records, where a positive difference is episodes whose end event arrived and produced nothing.

`lifecycle.runtime` compares the commit the served index was built at with the commit `git` reports on the call. Both are read; neither is assumed. A server that built its index once and held it answers every question about a commit from whenever it started, identically from the HTTP API, from MCP and from the Cockpit, with nothing in any answer saying the picture is old, and there is no way to notice that from inside one of those answers.

`lifecycle.providers` reports, per provider, the lifecycle events its adapter declares in the provider's own vocabulary, whether it can archive prompts, and which of this repository's `enforcement` entries name its hook in `wired_by`. The declaration is `share/providers.yaml`; `test/cases/170` refuses it drifting from the adapter table the shell actually drives. The two obvious alternatives are both wrong: reading the provider's hook directory reports an *installation* as a capability, and a list written into a page is a second source of truth that goes stale the day an adapter changes.

`lifecycle.closed` reads the tracked records through the object index — kind `session` — and reports how many exist, how many closed on this branch, and the newest twenty with a link to each object page. It is the only half of the subsystem that survives a clone.

None of the five declares a command line. They read `.ai/local/`, so they are served to the worker in front of the checkout and never shipped: a command line is how a value reaches a script, a log and eventually a commit.

## How to see it

```bash
majordomus serve                              # then open /cockpit/continuity
curl -s localhost:$PORT/api/v1/lifecycle/episodes  | jq '[.episodes[] | {session_id, standing, provider, last_event}]'
curl -s localhost:$PORT/api/v1/lifecycle/recovery  | jq '{balance, stranded: [.stranded[].session_id], orphans: [.orphans[].path]}'
curl -s localhost:$PORT/api/v1/lifecycle/runtime   | jq '{served_head, repository_head, agree}'
curl -s localhost:$PORT/api/v1/lifecycle/providers | jq '[.providers[] | {id, lifecycle, prompt_capture, wired}]'
curl -s localhost:$PORT/api/v1/lifecycle/closed    | jq '{total, on_this_branch}'
```

## What it does not cover

**It never writes.** The lifecycle has one writer, the shell tool, and a second account of events the ledger already holds is what that design refuses. Nothing here closes a stranded episode or deletes an orphan; each finding carries the command a person runs.

**It decides no thresholds.** Nothing in `lifecycle.*` judges a record to be old. Age is `session.freshness` in the policy and `continuity.state`'s to report; a second engine for it here would be exactly the duplicated numbers that make two surfaces disagree.

**It does not know whether a provider is attached.** Whether the client that opened an episode is still in its conversation is not a fact of this repository: the episode file records no process, the peer board records no episode id, and a client that exits without firing its end event leaves a file identical to one a live worker is using. `stranded` is reserved for the one case the repository can establish — the worktree is gone — and everything else is reported as where the file is and when the ledger last saw it.

**It shows no prompt.** The prompt archive is the one part of the local half that is a conversation, and no capability here and no card on the Continuity page reads it.

**The page does not follow a live channel.** The executions pages follow a WebSocket because an execution emits events; the session store does not, and a browser asking every few seconds whether an episode is still open would spend the day being told yes. A reload is the refresh.

## Why it exists

On 2026-09-11 this repository held five open episodes in one checkout and every surface the tool has could name exactly one of them — `continuity.state` follows the pointer, and the pointer is aimed at whichever episode opened most recently. The four others were invisible, and an episode nobody can see is an episode nobody closes: the ledger's own arithmetic showed twenty-one starts against fifteen closes at the same moment. The same week, a forensic audit found the subsystem had written no checkpoint and no handover for six days while `doctor` reported health throughout — ADR 0041, "The session lifecycle is the episode's, not the task's" — and a long-lived server was found serving a six-hour-old HEAD from three surfaces with nothing saying so. Three failures, one shape: the subsystem had no reading of itself, so each of them was discovered by a person noticing something else.

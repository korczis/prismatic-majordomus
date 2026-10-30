<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: share/brief/report.yaml, the one declaration of the self-report; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.9 -->
# The self-report contract

What a session reports about itself on the interval, composed by brief.session from this declaration and the attention entries it measured, and relayed verbatim by every carrier. Declared in `share/brief/report.yaml` (`brief-report/v1`, schema `majordomus/brief-report/v1`, fingerprint `3398aa56f184`). `majordomus brief --report` prints the report owed now. This page is rendered from the declaration; edit the declaration, never this page.

The sections below are reported in this order, and none is ever omitted.

## Sections, in order

| # | Key | Heading | Holds | Evidence | Cap |
|---|---|---|---|---|---|
| 1 | `header` | Majordomus report | the report's clock time in the person's zone and in UTC; the session id and its peer id; when the brief was composed, by which process, and in how long | brief.at, answered_by.process, answered_by.elapsed_ms | 1 line |
| 2 | `blocked` | Blocked on you | each decision, approval, credential, scope or physical act only the person can give (the attention entries whose actor is person), and what waits on it | the waiting object (PR number, run id, issue id, seat) and since when | 5 lines |
| 3 | `delivered` | Delivered since {since} | own pull requests merged (with the merge sha), own branch heads pushed (branch, sha of 10, push time) and releases tagged since the previous report | per report-verification-state: VERIFIED with the command that showed it, or PARTIAL or BLOCKED with what is missing | 8 lines |
| 4 | `in_flight` | In flight | each own item of work, one per issue or pull request: issue, branch, worktree, PR number, head, merge-state word; the train carrying it; its latest run with id, jobs done/total and every failed job by name; local background processes with pid and age | a status word per item; run ids; the source and age of any fact not read live | 8 items, at most 2 lines each |
| 5 | `integration` | Integration | the master sha and its required run's word and id; the queue observation's word, age and remedy; the trains carrying own pull requests or master's fix; the last merge on master | sha of 10, run id, the observation's age | 3 lines |
| 6 | `mesh` | Mesh | peers and unread boards; overlaps with own claims and with whom; reviews and handovers for this runtime; failing links | peer ids, claim scopes, link counts | 3 lines |
| 7 | `machine_ci` | Machine and CI | free disk against the builder floor; swap; builders against their limit; the derive lock; CI hosted running and queued; self-hosted runners online and busy | GiB figures, runner names, the age of the forge reading | 3 lines |
| 8 | `next` | Next | the one next action of this session with its trigger (now, or an event); then each planned action, labelled planned, with its owner and trigger; with no work in hand, say so and stop | the object each action acts on | 3 lines |

## Rules

| Id | Rule |
|---|---|
| `status-words` | Every state is a status word of share/design/tokens.yaml status.states, written in brackets ([failing], [stale], [queued], [merged]). No other verdict adjective (green, red, done, fine, OK, looks good). |
| `evidence` | Every line carries the identifier that proves it (PR number, run id, sha of 10, issue id, path). A fact not read in this turn also carries its source and age. |
| `stale-is-not-current` | A fact the brief marks stale, partial or unread is reported with that word, its age and its remedy, or is first read live with a command under live reads. It is never presented as current. |
| `planned` | Nothing future is written as state ("X will be removed", "is being fixed"). A future action is labelled planned and names its owner (this session, a peer id, a seat) and its trigger (an event or a clock time). |
| `empty` | An empty section is its heading and one word (none; in Czech, nic). |
| `no-restatement` | Do not restate this contract, the brief's lines, or a list of categories. Report only what was measured or read. |
| `language` | Write in the language of the person's last message. Identifiers, commands, branch names, section keys and status words stay verbatim. |
| `length` | At most a section's cap. Past it, write "+N more — <command that lists them>". Never pad to reach a cap, and never drop an attention entry. |
| `then` | After the report, carry on with the work in hand, without asking. |

## Attention rows

Each row is filled by one condition of `brief.session`; the two sets are held equal by a unit test.

| Id | Section | Word | Actor | Fact | Remedy |
|---|---|---|---|---|---|
| `integration.stale` | integration | `stale` | session | queue observed {age} ago at {observed}; origin/master is {now} ({moved}) | `majordomus prs refresh` |
| `integration.unfetched` | integration | `stale` | session | origin/master last fetched {age} ago | `git fetch origin master` |
| `master.failing` | integration | `failing` | seat:landing | master {sha}: {workflow} run {run} {conclusion} | `gh run view {run} --json jobs` |
| `run.failing` | work | `failing` | session | {branch}: run {run} {done}/{total}, failed {jobs} | `gh run view {run} --log-failed` |
| `pr.dirty` | work | `dirty` | session | #{pr} conflicts with master | `git -C {worktree} merge origin/master` |
| `pr.blocked` | work | `blocked` | session | #{pr} mergeStateStatus BLOCKED | `gh pr checks {pr}` |
| `branch.no_pr` | work | `unmerged` | session | {branch} pushed at {head}, no PR | `gh pr create --head {branch}` |
| `ci.queued_idle` | ci | `queued` | session | {queued} queued while self-hosted {online} online, {busy} busy | `gh api repos/{repo}/actions/runners` |
| `ci.no_runner` | ci | `blocked` | person | {queued} queued, 0 runners online | `gh api repos/{repo}/actions/runners` |
| `ci.unread_scope` | ci | `unread` | person | runners not readable ({reason}) | `gh auth refresh -h github.com -s admin:org` |
| `machine.below_floor` | machine | `blocked` | session | {free} GiB free < floor {floor} GiB | `scripts/reap-orphans` |
| `machine.swap` | machine | `degraded` | session | swap {used}/{total} GiB ({pct} %) | `top -o mem` |
| `machine.builders_full` | machine | `blocked` | session | builders {n}/{max} | `ps -Ao pid,etime,comm \| grep -E 'cargo\|rustc'` |
| `peers.unread` | peers | `unread` | session | boards not read: {boards} | `majordomus peers` |
| `peers.superseded` | peers | `superseded` | session | this server lost the lease to {leaseholder} | `majordomus server` |
| `mesh.review` | mesh | `pending` | session | {n} review(s) for this runtime | `majordomus mesh state` |
| `mesh.handover` | mesh | `pending` | session | {n} handover(s) waiting | `majordomus mesh handover consume` |
| `mesh.overlap` | mesh | `warn` | session | {n} overlap(s) between a claim of this runtime and another's | `majordomus peers` |
| `mesh.failing` | mesh | `failing` | session | {n} link(s) failing | `majordomus mesh doctor` |
| `release.owed` | release | `owed` | seat:release | {landed} landed commit(s) past the last release | `majordomus release next` |
| `release.overdue` | release | `overdue` | seat:release | {landed} landed commit(s) past the last release, past its bound | `majordomus release next` |
| `derive.dead_holder` | derive | `blocked` | session | derive lock held by dead pid {pid} for {held} | `majordomus doctor` |
| `session.no_task` | session | `inactive` | session | no active task (last {task} [{outcome}]) | `majordomus start "<task>" --scope <paths>` |
| `section.over_budget` | {section} | `unread` | session | {section} took > {budget} ms | `majordomus brief --format json` |

## Live reads

| Fact | Command |
|---|---|
| a run's jobs now | `gh run view <id> --json jobs` |
| the integration queue | `majordomus prs refresh` |
| origin/master | `git fetch origin master` |
| one pull request | `gh pr view <n> --json mergeStateStatus,statusCheckRollup` |
| every worker | `majordomus peers` |
| the mesh | `majordomus mesh state` |
| the CI pool | `gh api repos/<owner>/<repo>/actions/runners` |
| the machine | `df -g / ; sysctl vm.swapusage` |

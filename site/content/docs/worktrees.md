+++
title = "Worktrees"
description = "the branch-to-worktree topology: `<repo>-wt/<branch>` derived from git identity and never registered, the standings and diagnostic codes, the commands, the lifecycle, the layered enforcement, the fingerprint-verified migration, failure modes and recovery"
weight = 60
[extra]
source = "docs/WORKTREES.md"
+++

{% raw %}

Where every linked git worktree of this repository belongs, how that is decided, and what
the tool does about one that is somewhere else. Behaviour as implemented and tested; where
this document and the executable disagree, the document is wrong and changes in the same
commit. The decision is [ADR 21](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/adrs/0021-the-branch-to-worktree-topology-is-derived-from-git-identity-and-enfor.md);
the rule is `project.worktree-topology`.

```text
~/dev/prismatic-majordomus                    the primary checkout: hosts the trunk, never moves
~/dev/prismatic-majordomus-wt/                the container: the checkout's sibling, name + "-wt"
~/dev/prismatic-majordomus-wt/feature/x       the one worktree of branch feature/x
~/dev/prismatic-majordomus-wt/fix/a/b         the one worktree of branch fix/a/b — hierarchy kept
```

The branch name *is* the relative path. Nothing is flattened (`feature-x`), hashed
(`feature-x-a93f`) or numbered (`worktree-1`), and nothing is registered anywhere: given the
repository and a branch, there is exactly one path, and it is derived the same way by every
surface.

## Why

Several sessions — people and agents — work on one repository at once and each needs its
own checkout. Git's linked worktree is the mechanism, and git takes the destination as an
argument, so every caller decides where one goes. Fifty-three of them had decided
differently here: flat siblings named after nothing, session scratch directories, one
inside the primary checkout, one occupying the container's own path. The cost was not
untidiness. It was that "where is branch X" and "where does a new one go" had no answer
that did not involve looking, guessing or inventing, and a session that has to invent a
path leaves its work where the next session does not look.

The topology gives the answer once: feature isolation with no decision, parallel agents
that cannot collide on a path, a predictable session → branch → worktree mapping that a
handover can name and a resumed session can navigate back to, cleanup that is a derived
list rather than an archaeology, and a topology that every surface can show because every
surface computes it the same way.

## How the path is derived

```text
git rev-parse --git-common-dir             which repository this is, from any directory of any worktree
git worktree list --porcelain              its worktrees; the first record is the primary checkout
parent(primary) / name(primary) + "-wt"    the container
container / branch name                    the worktree, one directory per component
```

The second line is the one that is easy to get wrong. `git rev-parse --show-toplevel`
answers *the current worktree*, so deriving a container from it inside a linked worktree
would produce `…-wt/feature/x-wt`, a container per worktree, nesting for ever. The primary
checkout is read from git's shared metadata instead, so the answer is identical from the
primary checkout, from a subdirectory of it, from a linked worktree, and from four
directories deep inside one.

A branch name is filesystem-derived input and is treated as such: the name is validated by
git's own reference rules (no `..`, no component beginning with `.`, none ending in
`.lock`, no control characters, none of `~ ^ : ? * [ \`, no `@{`, no leading `-`), every
component of a valid name is an ordinary directory name, and the derived path is proved on
every call to be strictly below the container. Unicode components are ordinary directory
names. Two names that differ only by case are reported (`worktree.case_collision`), because
one directory holds both on a case-insensitive filesystem.

The trunk is discovered, never hardcoded: the remote's HEAD (`refs/remotes/origin/HEAD`),
then `init.defaultBranch` when that branch exists, then whichever of `main` and `master`
exists alone, then the primary checkout's own branch. Which one decided is reported.

`-wt` is written once, as `CONTAINER_SUFFIX` in `apps/majordomus-cli/src/worktree/path.rs`.
There is no configuration: a container that could be configured is a container two
checkouts of one repository could disagree about.

## The architecture

<pre class="mermaid">
flowchart TD
  git["git&lt;br&gt;common dir · worktree list · for-each-ref · status"]
  identity["RepositoryIdentity&lt;br&gt;primary, current, container, trunk —&lt;br&gt;from anywhere inside"]
  path["path::expected_path()&lt;br&gt;container / branch, proved to stay&lt;br&gt;inside the container"]
  service["WorktreeService&lt;br&gt;topology · status · guard · inspect&lt;br&gt;create · migrate · repair · remove"]
  cli["CLI"]
  registry["capability registry"]
  hook["git hook"]
  mcp["MCP tools + majordomus://worktrees&lt;br&gt;/api/v1/worktrees* · OpenAPI · Swagger UI"]
  cockpit["Cockpit /cockpit/worktrees · docs/generated"]
  git --&gt; identity
  identity --&gt; path
  path --&gt; service
  service --&gt; cli
  service --&gt; registry
  service --&gt; hook
  registry --- mcp
  registry --- cockpit
</pre>


Business logic exists once, in `apps/majordomus-cli/src/worktree/`. Every surface is an
adapter that renders or projects what the service answered; none derives a path, decides a
standing or judges whether a move is safe. The five read-only questions are capabilities
of the registry (`worktree.topology`, `worktree.status`, `worktree.inspect`,
`worktree.migration_plan`, `worktree.uncommitted`), which is what puts them on MCP, HTTP, OpenAPI, the Swagger UI,
the Cockpit and the generated reference without any of those carrying a route or a schema
of their own. Creating, migrating, repairing and removing are command-line operations of
the same service. A capability that writes the repository must declare the effect
`repository_mutation`, be a command and be reached by `POST` — a unit test pins the set of
such capabilities, so adding one is a visible decision — and moving worktrees on disk is not
something the shared MCP server takes on; the Cockpit names the exact command for each.

## Standings and diagnostics

Every registered worktree has one standing:

<div class="overflow-x-auto" tabindex="0">

| standing | meaning |
|---|---|
| `primary` | the main worktree; exempt from the path rule, held to the trunk rule |
| `canonical` | a linked worktree at exactly its branch's path |
| `misplaced` | a linked worktree somewhere else; migration brings it home |
| `detached` | no branch, so no canonical path; never moved |
| `ephemeral` | a session's scratch checkout, under a declared scratch root — the temporary directory, `<primary>/.claude/worktrees/` for Claude Code, bb's thread directory under its data directory — as `share/providers.yaml` declares them; reported, refused for commits, moved only on request |
| `missing` | a registration whose directory is gone; `repair` drops it |

</div>


and every condition has a stable code, the same on every surface, each with a remedy:

<div class="overflow-x-auto" tabindex="0">

| code | severity | what it means |
|---|---|---|
| `worktree.path_mismatch` | error | a linked worktree is not at its branch's canonical path |
| `worktree.container_occupied` | error | a worktree occupies the container path itself |
| `worktree.nested` | warning | it sits inside the primary checkout or another worktree |
| `worktree.destination_conflict` | error | the canonical path is occupied by something else — a registered worktree of another branch, a foreign checkout, a directory, a file, a symbolic link |
| `worktree.missing` / `worktree.stale_registration` | warning | the directory is gone |
| `worktree.branch_already_checked_out` | error | the branch is checked out somewhere other than its canonical path |
| `worktree.detached` | info | no branch |
| `worktree.ephemeral` | warning | a session's scratch checkout holding a branch |
| `worktree.primary_on_non_trunk` | error | the primary checkout holds a branch that is not the trunk |
| `worktree.trunk_in_linked_worktree` | warning | the trunk is checked out in a linked worktree |
| `worktree.path_escape` / `worktree.invalid_branch_name` | error | the name cannot derive a path |
| `worktree.locked` | warning | git will not move it until it is unlocked |
| `worktree.migration_verification_failed` | error | the fingerprint after a move differs from the one before |
| `worktree.trunk_unknown` | warning | nothing said which branch the trunk is |
| `worktree.case_collision` | warning | two branch names derive one directory on a case-insensitive filesystem |
| `worktree.cross_device` | info | a move was made by copy across filesystems |

</div>


The topology is *valid* when no error-level diagnostic stands. Detached and ephemeral
worktrees do not make it invalid; the guard still refuses a commit from an ephemeral one.

Which directories make a checkout ephemeral is data, not a list in the executable:
`share/providers.yaml` declares the tool's own scratch roots (the temporary directory) and
each provider's — `<primary>/.claude/worktrees` for Claude Code, and for bb, an
orchestrator that keeps a worktree per thread, `${BB_DATA_DIR:-~/.bb}/plugins/environment-git-worktree/host-data/worktrees`.
A root is expanded against the primary checkout and the environment; a root the primary
checkout itself lives under is skipped, so a test fixture in the temporary directory has
ordinary siblings. The diagnostic names the provider whose root it matched. Somebody
else's checkout is never migrated unasked and never cleaned up by the tool: the remedy the
guard offers is to continue in the canonical worktree (ADR 0024).

## Commands

```bash
majordomus worktree                            # where am I, and is that where I belong (exit 10 if not)
majordomus worktree create feature/improve-cli # start: the path is derived, never given
cd "$(majordomus worktree path feature/improve-cli)"
majordomus worktree list                       # every worktree with its standing
majordomus worktree topology --format json     # the whole document the API and the Cockpit render
majordomus worktree doctor                     # every diagnostic with its code and remedy
majordomus worktree migrate --plan             # what would move; changes nothing
majordomus worktree migrate                    # move, verify, report
majordomus worktree cleanup                    # what is merged and clean; deletes nothing
majordomus worktree uncommitted                # uncommitted work in linked worktrees, and what holds it
```

<!-- majordomus:unrun it removes worktrees, which a documentation check must never do to the checkout it reads; case 611 runs it in a disposable repository and holds every refusal -->
```bash
majordomus worktree cleanup --remove           # removes those worktrees, refusing what it cannot prove
```

<div class="overflow-x-auto" tabindex="0">

| command | what it does | exit 10 when |
|---|---|---|
| `worktree` / `worktree status` | this worktree: branch, standing, canonical path, uncommitted work, upstream, issue | this worktree is out of place |
| `worktree list` | every worktree, one line each | the topology has an error |
| `worktree topology` | repository, container, trunk, worktrees, branches without a worktree, diagnostics, tallies | the topology has an error |
| `worktree root` | the container, one path | |
| `worktree path <branch>` | the canonical path of a branch, one path; the branch need not exist | the name is invalid |
| `worktree inspect <branch>` | canonical path, whether the branch exists, what occupies the path, what stands in the way | |
| `worktree create <branch> [--base REF] [--issue ID]` | the canonical worktree; a branch the trunk's remote has is taken up and tracks it, a new one starts at the remote's trunk fetched now, and the line names the exact commit | occupied, checked out elsewhere, invalid |
| `worktree ensure <branch>` | the same, answering an existing canonical worktree instead of refusing | checked out elsewhere |
| `worktree migrate [--plan] [--only B]… [--allow-copy] [--include-ephemeral]` | bring misplaced worktrees home, fingerprint-verified | a step was blocked or failed |
| `worktree validate` / `worktree doctor` | the errors, or every diagnostic | an error stands |
| `worktree guard [--quiet]` | may a commit proceed from here | no |
| `worktree repair [--dry-run]` | drop stale registrations, repair git's links; deletes no directory | |
| `worktree remove <branch\|path> [--force]` | remove one linked worktree; never the primary, never a branch, never dirty work unforced | refused |
| `worktree cleanup` | branches merged into the trunk, or into the remote-tracking branch the trunk follows, whose worktree is clean or absent, with the commands that would remove them. A primary checkout nobody has pulled does not hide what has already landed. | |
| `worktree cleanup --remove` | removes those worktrees, each re-measured first; refuses a branch ahead of its remote, a worktree something is working in, uncommitted work, or a reading it could not take | |
| `worktree uncommitted [--older-than D]` | every linked worktree holding uncommitted work: paths, age of the oldest change, what holds it, `abandoned` or not, and the command that would put it on a remote ref; writes nothing | |
| `worktree trunk` | how far this checkout and the local trunk are from the remote trunk, as of the last fetch, judged against `checkout.trunk_distance`, with the commands that bring it current; fetches nothing | the checkout is stale |
| `worktree branches [--without-worktree]` | every local branch, one per line | |

</div>


`wt` is an alias for `worktree`. `--format json` is available everywhere and is the same
typed answer the human form renders. A **selector** for `remove` is exact — a branch name or
a path — and nothing is matched by prefix or similarity. There is no `worktree cd`: a child
process cannot change its parent shell's directory, so `path` prints one and the shell does
the rest.

`worktree create --issue I0042` names the branch `feature/I0042-<slug>` from the issue's
own record, and the topology reads the issue back from any branch that carries an issue id
as a path component (`feature/I0042-live-page` → `I0042`). Nothing is inferred from
similarity: `feature/I00420-x` names no issue.

Shell completion of the branch arguments reads the live set rather than a list:

```bash
# zsh
_mj_wt_branches() { compadd -- ${(f)"$(majordomus worktree branches 2>/dev/null)"} }
compdef '_arguments "1:sub:(status list topology root path inspect create ensure migrate validate doctor guard repair remove cleanup uncommitted trunk branches)" "2:branch:_mj_wt_branches"' majordomus-worktree
```

## Lifecycle

<pre class="mermaid">
flowchart TD
  issue["issue / task&lt;br&gt;majordomus plan next"]
  branch["branch&lt;br&gt;feature/&amp;lt;ID&amp;gt;-&amp;lt;slug&amp;gt;, or any name git accepts"]
  worktree["canonical worktree&lt;br&gt;majordomus worktree create &amp;lt;branch&amp;gt;&lt;br&gt;→ &amp;lt;repo&amp;gt;-wt/&amp;lt;branch&amp;gt;"]
  session["session / agent&lt;br&gt;majordomus context, start, checkpoint, handover&lt;br&gt;— in that worktree"]
  commit["commit / PR&lt;br&gt;the pre-commit hook asks the guard;&lt;br&gt;push and open the PR from there"]
  cleanup["cleanup&lt;br&gt;majordomus worktree cleanup → worktree remove,&lt;br&gt;git branch -d, by a person"]
  issue --&gt; branch
  branch --&gt; worktree
  worktree --&gt; session
  session --&gt; commit
  commit --&gt; cleanup
</pre>


A handover records the branch and the worktree; a session resumed elsewhere derives the
worktree from the branch rather than trusting the recorded path, because the path is
ephemeral and the branch is not.

### Where a new branch starts

The primary checkout hosts the trunk, and nothing advances it: on 2026-10-16 its local
`master` was 779 commits behind `origin/master`. A branch cut from it carried none of that
work, the report said `(new, from master)` as if that were current, and every worker reset
the new worktree to `origin/master` by hand. A branch that existed only on the remote was
worse: it was started again from that stale `master` rather than taken up.

So `create` and `ensure`, for a branch that does not exist here, ask the trunk's remote —
the remote whose HEAD names the trunk, else the one `branch.<trunk>.remote` names — with two
quiet, bounded fetches (`--no-tags`, no prompt, killed after 60 seconds) of the trunk and of
the branch into their remote-tracking refs. The local trunk is never moved.

<div class="overflow-x-auto" tabindex="0">

| the remote | the branch starts | the report |
|---|---|---|
| has the branch | where the remote's branch stands, tracking it | `branch  fix/x (from origin/fix/x at <sha>, tracking it)` |
| answers, has no such branch | at the remote's trunk as fetched now, tracking nothing until its first push | `branch  feature/x (new, from origin/master at <sha>)` |
| cannot be read | at the local trunk, or at the remote's branch as last fetched when one is known | the same line from `master`, and a `warning` line naming the remote, git's reason, how many commits the local trunk is behind the remote's trunk as last fetched, and the reset to run once it answers |
| there is none | at the local trunk; nothing is fetched and nothing is said | `branch  feature/x (new, from master at <sha>)` |

</div>


`--base REF` is taken as given and never fetched. The JSON carries the decision as `start`
(`from`: `remote_trunk`, `remote_branch`, `local_trunk`, `given`, `head`, with the remote,
the reason and the distance) and the commit read back from the new worktree as
`start_commit`. This is the one place a worktree command reaches the network; every other
read and decision works offline.

### The `.envrc` of a new path

direnv approves an `.envrc` by path and content, so the repository's own `.envrc` at a
path that did not exist a moment ago is a file direnv has never seen, and the first `cd`
into a fresh worktree was answered with `direnv: error .envrc is blocked`. Measured on
2026-09-09: 36 of the 37 worktrees that carried an `.envrc` were blocked, and the one
approved was the primary checkout. `create`, `ensure` and every moved step of `migrate`
therefore carry the primary checkout's approval to the new path — `direnv allow` on the
worktree's `.envrc`, at the moment the path exists — and report what happened on an
`envrc` line and in the `envrc` field of the JSON.

Carried, not granted: the worktree's `.envrc` must be byte-for-byte the primary
checkout's, and the primary checkout's must itself be approved, or nothing is approved and
the report says which (`differs`, `not_approved_in_primary`). A branch with an `.envrc` of
its own is a file the person has not read, and approving it for them is the one thing
`direnv allow` exists to prevent. Without direnv on the PATH the outcome is
`direnv_absent` and nothing is blocked, because nothing would load the file.

The primary checkout's machine-local `.envrc.local` is shared in the same moment. The
repository's `.envrc` sources `.envrc.local` from the directory it is entered from, and
the adapter rule keeps that file free of the program a lookup of the primary checkout
would need. So a new worktree gets a symlink to the primary checkout's `.envrc.local`, and
the person's own exports, including keychain-backed secrets such as `OPENAI_API_KEY`, load
there too, from one file. The link is made only where git ignores the name
(`not_ignored` otherwise), and a worktree with an `.envrc.local` of its own keeps it
(`own_kept`).

### The hooks of its own tree

A repository that tracks its hooks (`.githooks/`, named by `core.hooksPath`) means each
commit to be judged by the hooks of the tree it is made in. `core.hooksPath` lives in the
configuration every worktree shares, so once it holds an absolute path —
`<primary>/.githooks`, as it did here — every linked worktree runs the primary checkout's
hooks. On 2026-10-14 the primary checkout was three days behind, and the `doctor` its hooks
ran refused every commit of current master in every worktree (`FAIL wiring
ssh-permissions-on-commit … not invoked by <primary>/.githooks/pre-commit`); I2718.

Each linked worktree therefore has a hooks path of its own. `create`, `ensure`, every moved
step of `migrate` and `repair` read the repository's policy from the primary checkout's
effective `core.hooksPath`, find the directory it names inside the worktree, and, when git
would run another checkout's, bind the worktree to its own with git's per-worktree
configuration:

```sh
git config core.repositoryformatversion 1          # only when it was 0
git config extensions.worktreeConfig true          # only when it was off
git -C <worktree> config --worktree core.hooksPath <worktree>/.githooks
```

The value is absolute, so a moved worktree is bound again at its new path. The report says
what happened on a `hooks` line and in the `hooks` field of the JSON (`set`, `own`,
`not_configured`, `shared`, `untracked`, `failed`). Nothing is invented: a repository with no
`core.hooksPath` keeps git's own hooks directory, a hooks directory outside the primary
checkout is shared on purpose and left alone, and a tree that does not carry the directory
keeps the primary's rather than none. A relative `.githooks` is already resolved by git
against each worktree's own top level, and is reported as `own`. The primary checkout is
never changed.

`doctor`, run in a linked worktree that git sends to another checkout's hooks, fails
`wiring hooks-path` with the exact command that fixes it, and judges the wiring of the
worktree's own directory — what it will run once fixed — rather than the stale one.

The last step is the one with no mechanism behind it. Creating a worktree is one command;
removing one is a decision nobody is prompted to make, and `cleanup` deliberately deletes
nothing — it names what is merged and clean and leaves the act to a person, because a tool
that removed a worktree on the strength of a merged branch would eventually remove work
somebody had not finished. The cost is real and was paid here on 2026-09-08, eighteen hours
after the topology landed: fifteen worktrees between them held 104 GB of Rust `target/`
directories, nine of those with branches already merged into the trunk, and the volume
reached 124 MiB free. Nothing in the topology reclaims anything and no budget is declared
for the container, so the disk is what says stop — and it says it in whatever vocabulary
the next subsystem to fail happens to use. Three said their content was wrong. See ADR 0021
for why the mechanism is absent rather than missing.

If you hold a worktree whose branch has landed, `worktree cleanup` tells you; `git worktree
remove <path>` and `git branch -d <branch>` are the two commands, and the `target/`
directory inside it is usually most of what it costs.

## Enforcement

Layered, and honest about what each layer can do:

<div class="overflow-x-auto" tabindex="0">

| layer | what it does |
|---|---|
| the doctrine | ADR 21 and `project.worktree-topology` say what the rule is |
| the agent bootstraps | `AGENTS.md`, `CLAUDE.md` and the other provider files, generated from the templates, tell every worker to establish the topology before implementing and to correct a mismatch with the tool |
| the workflows | `.ai/repo/workflows/task-lifecycle.md` starts with `worktree status` |
| the command line | `worktree create` is the way a branch's worktree comes into being; a path is never an argument |
| the git hook | `.githooks/pre-commit` asks `majordomus worktree guard`; a feature branch is committed only from its canonical worktree, the primary checkout only on the trunk |
| the wiring check | the policy's `enforcement` list declares the guard, so `majordomus doctor` proves the hook asks it |
| the repository entry | `.envrc` renders the repository environment on entry through `bin/majordomus-env`, which never builds ([`ENVIRONMENT.md`](@/docs/environment.md)); the worktree standing in that banner is a target |
| the surfaces | the topology is on the command line, MCP, HTTP, the Swagger UI and the Cockpit, so a wrong one is visible everywhere |
| the tests | the crate's suite proves the derivation, the safety and the migration against real git; the shell case proves the wiring |
| the CI gate | `scripts/ci/worktree-check` holds the constant, the hook, the policy, the documents and the case together |

</div>


A git hook is bypassed with `--no-verify`, and a CI runner cannot see a contributor's local
filesystem; the gate therefore measures the rule's machinery, not this machine's
directories. Within Majordomus-controlled workflows a wrong worktree is refused; outside
them it is visible on every surface.

## Migration

`worktree migrate --plan` lists every misplaced worktree with a branch: where it is, where
it belongs, how it would move, the uncommitted work that moves with it, and what blocks it.
The container's occupant, if any, is first — every other destination is inside the
container, and while a worktree *is* the container those destinations would be created
inside that worktree. Exceptions — detached worktrees, stale registrations, ephemeral
scratch checkouts, the primary checkout off the trunk — are listed with what a person does
about each.

`worktree migrate` recomputes the plan under the repository lock and, for each movable step:

1. takes a fingerprint of the worktree: branch, HEAD, the index (every tracked path with
   mode, blob and stage), the staged diff, the unstaged diff, every untracked file with its
   size and content hash, every ignored entry by path and size;
2. asks git to move it — `git worktree move`, one rename, dirty state included; the
   container's occupant goes out to a staging path and then in; a move across filesystems
   is refused unless `--allow-copy`, and then made by copy, `git worktree repair`,
   verification against a manifest of every entry of the tree, and only then removal of
   the original;
3. takes the fingerprint again at the destination, verifies that git registers it there,
   and reports the step as `moved` only when the two fingerprints are equal — otherwise
   `failed`, with what differs, and nothing further is touched;
4. names relative symbolic links that no longer resolve from the new path
   (`node_modules -> ../sibling/node_modules`), which the move did not change and does not
   rewrite.

Nothing is reset, stashed, cleaned, checked out or deleted. A locked worktree is blocked
until it is unlocked. A destination that exists — a registered worktree of another branch,
a foreign git checkout, a directory empty or not, a file, a link — is a conflict named by
kind and never overwritten. If the command was run from a worktree that moved, the report
says where to `cd`.

## Failure modes and recovery

<div class="overflow-x-auto" tabindex="0">

| situation | what you see | what to do |
|---|---|---|
| a dirty worktree in the wrong place | `worktree.path_mismatch`, the dirty counts on the plan step | `majordomus worktree migrate`; the work moves with it |
| the canonical path is taken | `worktree.destination_conflict` naming what is there | move it aside by hand; nothing overwrites it |
| a detached worktree | `worktree.detached`, standing `detached` | `git switch -c <branch>` there, then migrate; or leave it |
| a missing directory | `worktree.missing` / `worktree.stale_registration` | `majordomus worktree repair` |
| a locked worktree | `worktree.locked`, the step blocked | `git worktree unlock <path>`, then migrate |
| the wrong branch in a worktree | `worktree.branch_already_checked_out` on create | migrate the worktree that holds it, or work there |
| `direnv: error .envrc is blocked` in a worktree | `envrc   differs …` or `envrc   the primary checkout's is not approved …` on the create or migrate report | read the file, then `direnv allow` there; approve the primary checkout's first when that is what the report names. A worktree older than the report: `majordomus worktree ensure <branch>` carries the approval now |
| a commit in a worktree judged by stale hooks | `FAIL wiring hooks-path … runs <primary>/.githooks, not its own` from `doctor` | the command doctor prints, or `majordomus worktree repair`, which binds every linked worktree to its own `.githooks` |
| the primary checkout on a feature branch | `worktree.primary_on_non_trunk`; the guard refuses | when clean, `git switch <trunk>`; then `worktree create <branch>` |
| a new worktree started behind the remote | `warning could not fetch <trunk> from <remote> (…): started from the local <trunk>, N commits behind …` on the create report | once the remote answers, the `git fetch` and `git reset --hard` the warning prints, in the new worktree, before its first commit |
| a scratch checkout of a session on a branch | `worktree.ephemeral`; the guard refuses | `git switch --detach` there and continue in the canonical worktree, or `migrate --include-ephemeral --only <branch>` |
| a task in a fresh worktree off the mesh | `WARN mesh … not claimed on it` from `start`, then `FAIL mesh … never claimed on it` from the pre-push `finish --check` | `start` ensures the worktree's own server before it claims, so this appears only when it could not (`MAJORDOMUS_RUNTIME=off`, a stale executable, a server that did not converge): `majordomus serve ensure`, then `majordomus check`, which claims the running task's scope. No partial finish and second start (`docs/MESH.md`) |
| a move that crossed devices | `worktree.cross_device` on the step, or a refusal | `migrate --allow-copy` |
| a fingerprint mismatch | `worktree.migration_verification_failed`, the differences listed | inspect the worktree where git registers it before touching anything; a rename cannot lose content, so something wrote to it during the move |
| a built executable that was there a minute ago | `MAJORDOMUS_BIN is not an executable`, or a test failing on a branch that was green | an environment event, not a defect in the branch under test: another worker reclaimed the build directory. Rebuild in **your own** worktree and re-measure. `project.reclaim-only-what-you-own` says whose it was |
| a borrowed executable from another worktree | artifacts reported stale on a tree that is current; a derivation that removes thousands of lines | establish it matches before using it — `majordomus generate --check` must say `in sync` — and read `git diff --stat -- docs/generated site/data` before committing. `project.a-tool-that-cannot-match-the-tree-refuses` |

</div>


## Performance

`worktree status` and `worktree guard` read the registrations, the branches and one `git
status` of the current worktree: a handful of subprocesses, no index build, no network, no
scan. `topology` and `list` add one `git status` per existing worktree and nothing else.
The migration hashes the content of every modified and untracked file of the worktrees it
moves, which is the one place that cost is right. Nothing is cached across calls: the
topology changes outside the process.

## Where things are

<div class="overflow-x-auto" tabindex="0">

| | |
|---|---|
| the constant and the derivation | `apps/majordomus-cli/src/worktree/path.rs` |
| identity and trunk | `apps/majordomus-cli/src/worktree/identity.rs` |
| the typed topology | `apps/majordomus-cli/src/worktree/model.rs` |
| the service | `apps/majordomus-cli/src/worktree/service.rs` |
| the migration and fingerprints | `apps/majordomus-cli/src/worktree/migrate.rs`, `fingerprint.rs` |
| the capabilities | `apps/majordomus-cli/src/capability/builtin/worktree.rs` |
| the command line | `apps/majordomus-cli/src/commands/worktree.rs`, declared in `src/cli.rs` |
| the Cockpit page | `/cockpit/worktrees`, `apps/majordomus-cli/src/cockpit/pages.rs` |
| the launcher the hook uses | `bin/majordomus-cli`; `.envrc` goes through `bin/majordomus-env`, the launcher that never builds, and both find the executable through `lib/rust_bin.sh` |
| the hook | `.githooks/pre-commit`; the entry in `.ai/repo/policy.yaml` `enforcement` |
| the gate | `scripts/ci/worktree-check`, gate `worktree-topology` in `.ai/repo/ci/gates.yaml` |
| the tests | `apps/majordomus-cli/tests/worktree.rs`, `test/cases/96_worktree_topology.sh` |
| the decision and the rule | ADR 21, `.ai/repo/rules/project/worktree-topology.v1.md` |

</div>


## Reclaiming

`majordomus worktree cleanup` lists what is spare: every branch merged into the trunk whose
worktree is clean or absent. `--remove` removes those worktrees, and nothing else — branches
are never deleted, because the disk is what runs out and a branch is the only durable name a
piece of work has.

The listing and the removal are two moments, and everything that makes a worktree safe to
remove can change between them, so the listing only nominates: `--remove` reads the branch's
standing against its remote, whether anything is running inside and whether the tree is clean
again for each candidate immediately before it goes, and names what it refuses:

<div class="overflow-x-auto" tabindex="0">

| refusal | why it is not covered by "merged and clean" |
|---|---|
| ahead of its remote, no upstream, or upstream gone | the branch's merged history is on the remote; a commit on top of it is on one disk, and being merged says nothing about that |
| a process has its working directory inside | mtime lies: two worktrees swept on 2026-09-15 looked untouched since the 12th and had live processes in them |
| uncommitted work | read again at the moment of removal, not taken from the listing |
| `lsof` is missing, or git could not compare with the upstream | not knowing is not the same as nothing being there |

</div>


`test/cases/611_the_reclaim_refuses_what_it_cannot_prove.sh` plants a fixture with its own
remote: a spare worktree, which goes, and a branch ahead of its remote, one never pushed, one
whose remote branch was deleted, one with a process inside and one dirty before the listing,
which stay. It includes the combination the refusals exist for: a branch merged into the
trunk *and* carrying a commit its remote has never seen. What changes *after* the listing — a
commit, a deleted remote branch, a file written — is planted by the unit tests of the reclaim
in `apps/majordomus-cli/src/commands/worktree.rs`. A missing `lsof` is not planted anywhere.

## A checkout and its distance from the trunk

On 2026-10-16 the primary checkout was 779 commits behind `origin/master`, with uncommitted
changes nobody could attribute. Its shared server answered every session's plan questions
from that tree, and its local `master` was the base new branches were made from. Every
reading said the server was ready; none said the tree it served was not the trunk.
`project.a-checkout-serves-the-current-trunk` makes that distance a reading.

`worktree::trunk::measure` counts, for the checkout's `HEAD` and for the local trunk branch,
the commits of the remote-tracking trunk each lacks (`behind`) and the commits each has that
the trunk lacks (`ahead`). The remote trunk is the trunk's upstream, else the remote whose
`HEAD` names the trunk, else the first remote with a branch of that name. Nothing is fetched:
the answer holds as of the last fetch, and `fetched_at` (the time of `FETCH_HEAD`) says when
that was. While a merge is in progress its incoming commits count as present, so the merge
that brings a branch current is never judged by the tree it is replacing.

<div class="overflow-x-auto" tabindex="0">

| standing | when | doctor |
|---|---|---|
| `current` | behind by less than `warn_behind`, or not behind | `OK clone trunk` |
| `lagging` | at or past `checkout.trunk_distance.warn_behind` | `WARN`, with the remedy |
| `stale` | at or past `checkout.trunk_distance.fail_behind` | `FAIL`, so the commit hook refuses |
| `unjudged` | behind, and the policy declares no threshold | `INFO` |
| `unmeasured` | no trunk, or no remote-tracking branch carries it | `INFO` |

</div>


The thresholds are the policy's (`.ai/repo/policy.yaml`, `checkout.trunk_distance`), and
nothing reads a default when they are absent. The standing of the checkout is what its
server serves; the standing of the local trunk (`local_trunk_standing`) is what a branch
made from it starts from. In a linked worktree the local trunk is another checkout's to
move, so doctor reports it as `WARN clone local-trunk` and never fails that worktree's
commits for it.

The same reading appears wherever a tree is served:

- `majordomus worktree trunk` (`worktree.trunk`, `majordomus_worktree_trunk`,
  `GET /api/v1/worktrees/trunk`) answers it alone, and exits 10 when the checkout is stale;
- `majordomus worktree status` prints a `currency` line and returns `trunk_distance`;
- `server.status` (`majordomus serve status`, `majordomus_server`, `GET /api/v1/server`)
  carries it as `trunk` for the checkout the server serves;
- the MCP `initialize` instructions add one sentence — "This server serves a tree N
  commit(s) behind origin/master" — when the distance is lagging, stale or unjudged;
- `majordomus doctor` renders it under `clone`.

The remedy each reading names brings the checkout current: a fast-forward of the trunk
checkout, a merge of the remote trunk into a feature branch (a pushed branch is never
rewritten), and a fast-forward of the local trunk where it is checked out. Uncommitted work
is rescued first — `git stash create`, `git update-ref refs/rescue/<checkout>-<timestamp>`
with that commit, the untracked files copied beside it. A typed `majordomus worktree update`
that performs exactly that, and refuses while a process stands in the checkout, is the next
slice of the rule. `test/cases/3176_a_checkout_serves_the_current_trunk.sh` holds the
distances, the standings, doctor's verdicts and the merge-in-progress case against a fixture
remote.

## Uncommitted work nobody holds

A branch whose commits reach no remote is named by `doctor` (`clone unpushed`). Work that
reached no commit at all was not: on 2026-10-07 the supersession-trust half of the
integration safety fixes sat as 45 uncommitted files in the worktree of a session that had
ended, and on 2026-10-08 the completion doctrine sat as 21 staged files in another. Both
were found because somebody happened to read those directories, and pushed as
`rescue/...` branches; the same evening the data volume had 125 MiB free (I1995).

`majordomus worktree uncommitted` (`worktree.uncommitted`, `majordomus_worktree_uncommitted`,
`GET /api/v1/worktrees/uncommitted`) lists every linked worktree whose index or working tree
holds changes — staged, unstaged, untracked and conflicted paths counted together — with:

<div class="overflow-x-auto" tabindex="0">

| field | what it is |
|---|---|
| `paths`, `work` | how many paths, and the counts `worktree status` gives |
| `oldest_change`, `age` | the earliest modification time among the changed paths; for a path that no longer exists (a deletion) the index file's time stands in |
| `task` | the active task recorded there: `.ai/local/state/current.yaml` with outcome `active`, the record `start` writes and `finish` closes |
| `episodes` | the open episodes there (`.ai/local/state/sessions-open/`), each with its last sign of life and whether it is stranded |
| `process_inside` | a process has its working directory inside, by the `lsof` reading `cleanup --remove` refuses on |
| `standing` | `held` when the task, an episode that is not stranded or a process holds it; otherwise `abandoned` when the oldest change is older than the threshold (or its age cannot be read), else `recent` |
| `remedy` | the command that would commit the work there and push it to `rescue/<branch>-<YYYYMMDD>`; suggested, never run |

</div>


The threshold is the policy's `session.stranded_after`, the silence after which an episode's
client is taken to be gone. It answers both questions asked here — does an open episode
still hold the work, and has work that nothing holds waited long enough to be abandoned — so
there is no second number to drift from it; `--older-than` names another for one reading.
A policy that declares none judges nothing abandoned and says so.

`majordomus doctor` renders the reading: one `WARN clone <path>` per abandoned worktree with
its count and its age, and the rescue command as the remedy; held and recent work is
counted in one `OK`. The reading takes one `git status` per linked worktree, about 15 s over
three hundred of them, so `.githooks/pre-commit` runs doctor as
`MAJORDOMUS_DOCTOR_CONTEXT=commit bin/majordomus doctor`: in that context, the only one doctor
accepts, the reading is left out and an `INFO clone uncommitted` line says so and names
`majordomus worktree uncommitted`. Neither the reading nor doctor writes anything: every `git status`
runs under `GIT_OPTIONAL_LOCKS=0`, so no index is refreshed, and another worktree's
`.ai/local/` is read, never written. `test/cases/3025_abandoned_work_in_a_worktree_is_named.sh`
plants an abandoned worktree, a held one and a recent one, and holds `git status` of every
worktree byte-identical across the reading, the MCP tool and doctor.

## A build behind another worktree's lock

The worktrees share one build directory (ADR 0139), and cargo locks a build directory for
as long as a build uses it, so two builds at once are one build and one wait. Every build
this repository's tooling starts goes through `mj_cargo_bounded` (`lib/rust_bin.sh`):

<div class="overflow-x-auto" tabindex="0">

| what happens | what a worker sees |
|---|---|
| nobody else is building | the build runs in the shared directory, as before |
| another worktree holds the lock | after `MAJORDOMUS_BUILD_LOCK_WAIT` seconds (default 60) one line says how long it waited and which cargo processes exist, and the build runs again in this tree's own directory under `~/.cargo/build-private/` |
| the lock came free before the bound | nothing: the build went on in the shared directory |
| the build is already in its own directory and is still blocked | exit 75 with the reason, instead of a wait |
| the disk floor refuses a private build | exit 12 with the floor's own message |

</div>


A private directory compiles every dependency once, so its first build is the slow one.
`MAJORDOMUS_BUILD_LOCK_WAIT=0` never waits; an exported `CARGO_BUILD_BUILD_DIR` is taken as
the tree's own directory and is never replaced. The rule is
`project.a-build-never-waits-in-silence-on-anothers-lock`; `scripts/liveness-check` refuses
a tracked script that calls cargo any other way, and
`test/cases/3002_a_build_never_waits_in_silence.sh` proves each row above.

## Idle incremental sessions in the shared build directory

rustc keeps a crate's incremental state as session directories under
`<build-dir>/<profile>/incremental/<crate>-<hash>/`, and only a later compile of the same crate
removes a finished one. A crate that stops being compiled leaves its last session behind, and in
the build directory every worktree shares that means every worktree's leftovers. On 2026-10-16
the shared directory held 79 GB of them in 454 sessions, five of them live, with 6.9 GiB of the
disk left free.

So before a build starts, `mj_cargo_bounded` and the disk floor's check (`lib/rust_bin.sh`)
remove the idle sessions of the build directory that build uses:

<div class="overflow-x-auto" tabindex="0">

| what | what happens |
|---|---|
| a finished session (`s-*`, not `-working`) unwritten for over `MAJORDOMUS_INCREMENTAL_IDLE_MIN` minutes (default 30) | removed, after its age and size are printed; then one line says how many were removed, what du counted in them, and the volume's free space before and after |
| a `-working` session, a younger one, the lock files beside them | never touched |
| another build directory, a target directory, a profile directory that is a symlink out of the build directory | never touched |
| cargo does not say where the build directory is, it carries no mark of cargo's, or a listing cannot be read whole | nothing is removed, and one line says why |
| free space is still under the floor after that | the build refuses, and the refusal names the five largest build directories with their sizes |

</div>


The build directory is the one `cargo metadata` reports, asked from inside the crate so that
the pinned toolchain answers. `MAJORDOMUS_INCREMENTAL_IDLE_MIN=0` removes nothing.
`test/cases/3080_idle_incremental_sessions_are_pruned.sh` proves each row against a fixture
build directory.

## Stale crate artifacts, and no sweep under a running build

Every build of `majordomus-cli` writes its library, its executable, every test and bench binary
and, on macOS, one object file per codegen unit into `<build-dir>/<profile>/deps`, named
`<target>-<hash>`. The hash is the unit's metadata, so a generation that stops being built — a
dependency bumped, a profile setting changed, a test renamed — stays there for good, and in the
shared build directory that is every worktree's leftovers. On 2026-10-16 the shared `deps` held
204 GB; the next morning the volume reached zero and every session on the machine stopped.

So the same sweep that removes idle incremental sessions (`mj_rust_prune_build_dir` in
`lib/rust_bin.sh`, asked by `mj_cargo_bounded` and the disk floor's check before every build)
also removes this crate's stale generations, in every profile of the build directory — debug,
release, each target triple's and a coverage run's `llvm-cov-target/debug`:

<div class="overflow-x-auto" tabindex="0">

| what | what happens |
|---|---|
| a generation of this crate's own units (library, executables, tests, benches) no file of which was written for over `MAJORDOMUS_DEPS_IDLE_MIN` minutes (default 120), none of them open | removed whole; one line says how many generations and files and what du counted, the twelve largest by file count are listed, and one line gives the volume's free space before and after |
| a generation with one recent file, or one file some process holds open (lsof) | never touched |
| another crate's artifact, a name with no unit hash, a lock, a directory of cargo's own | never touched |
| a profile a build holds cargo's lock on | not swept, and one line says so |
| a profile some process names in its command line — a test binary, a doctest's rustdoc, a compiler | not swept, and one line names the process |
| no `flock(1)` or no `lsof` | nothing is removed, and one line says why |

</div>


The lock is the one cargo itself holds for the whole of a build,
`<profile>/.cargo-build-lock` (flock(2); `.cargo-lock` in a target directory of an older cargo),
taken without waiting. It is not enough on its own, and the second condition exists because of
what it misses: `cargo test` releases the lock when compiling ends, before the binaries and
doctests it built run, and a doctest reads the crate's rlib from `deps`. On 2026-10-16 rlibs
vanished under a running build while a sweep judged by age alone. Each condition is asked again
the moment before anything is removed. A generation removed too early costs one compile: cargo
rebuilds a unit whose output is missing. `test/cases/3196_stale_crate_artifacts_are_pruned_under_cargos_lock.sh`
proves each row, and that removing the lock or the process check makes it fail.

## Private build directories

A build that waits `MAJORDOMUS_BUILD_LOCK_WAIT` seconds behind another worktree's lock builds
again in `~/.cargo/build-private/<cksum of the tree's root>` (`mj_cargo_private_dir`; nothing
in `~/.cargo/config.toml` makes these — `mj_cargo_bounded` does, by exporting
`CARGO_BUILD_BUILD_DIR`). Each one is a cold build of every dependency, a few GiB, and on a
machine where many sessions build, most builds wait that long, so nearly every worktree that
ever built left one. On 2026-10-17 they held 178 GiB in 35 directories, 25 of them idle for
over two hours, and the volume reached 3.9 GiB.

The disk floor's check therefore also runs `mj_rust_reclaim_private`, at most once per ten
minutes per machine:

<div class="overflow-x-auto" tabindex="0">

| what | what happens |
|---|---|
| a private directory carrying cargo's mark, nothing in it written for over `MAJORDOMUS_PRIVATE_BUILD_IDLE_MIN` minutes (default 120), no profile lock held, no process naming it | renamed out of cargo's reach, then removed whole; one line names what was removed and the free space before and after |
| one with a recent file, a held lock or a process naming it; one with no mark of cargo's; a symlink; the one this shell builds in | kept |
| a listing that cannot be read whole | kept |

</div>


`test/cases/3197_idle_private_build_dirs_are_reclaimed.sh` proves each row.

All three sweeps are off in the test suite (`test/run.sh` sets `MAJORDOMUS_INCREMENTAL_IDLE_MIN`,
`MAJORDOMUS_DEPS_IDLE_MIN` and `MAJORDOMUS_PRIVATE_BUILD_IDLE_MIN` to 0): a case must not act on
the machine it measures. `scripts/ci/backlog-check` refuses a tree in which a build no longer
sweeps before it starts, a sweep no longer takes cargo's lock or asks the process table, or the
runner leaves them on (rule `project.accumulation-is-measured`).

## A path crate another worktree built

Sharing the build directory shares every unit cargo names by its path relative to the
workspace, so the same path crate compiled from two worktrees is one slot. `majordomus-cli`'s
build script reruns on `MAJORDOMUS_BUILD_TREE` (`.cargo/config.toml`), which makes its slot
stale in any tree but the one that wrote it. A path crate without that build script does not
get it: on 2026-10-17 batch 71b's build reused a `majordomus-core` another worktree had compiled
against different dependency versions and failed with "multiple different versions of crate
serde_json". Touching that crate's `src/lib.rs` forced the rebuild. Every path crate of the
workspace needs the same `cargo:rerun-if-env-changed=MAJORDOMUS_BUILD_TREE` in a build script
of its own.
{% endraw %}

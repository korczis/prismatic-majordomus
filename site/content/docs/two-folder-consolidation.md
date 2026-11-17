+++
title = "Two-folder consolidation"
description = "what the repository writes into a tree today, verified against the code and the running binary, and the ordered slices that reduce it to `.ai/**` and `.majordomus/**` with reversible bridges"
weight = 12
[extra]
source = "docs/TWO_FOLDER_CONSOLIDATION.md"
+++

{% raw %}

The phase 1 deliverable of the `two-folder-integration` milestone (issue I1101, decision ADR
0169). It lists every path Majordomus writes, generates or mutates when a project adopts it,
outside `.ai/` and `.majordomus/`. For each path it gives the code that writes it, whether
anything records the write, the class it belongs to, and what the two-folder contract does
with it.

The inventory was established on 2026-10-16 against `d1162668036` in two ways, and a row
appears here only when one of them found it:

- **Reading the writers.** Every write in `bin/majordomus`, `lib/*.sh`,
  `share/mods/claude-code/`, `site/static/install.sh` and `apps/majordomus-cli/src/**` was
  read: redirections, `mv`/`cp`/`mkdir`/`ln`, `git config`, `update-ref`, `push`,
  `std::fs::write`, `create_dir_all`, `rename` and `symlink`. Temporary directories that are
  removed before the command returns are left out, and so are writes that happen only inside
  the Majordomus source tree (`scripts/derive`, `scripts/ci/*`, `.just/*`).
- **Measuring an adopter.** A new `git init` repository with a README, one source file and
  one commit was adopted with `HOME` and the XDG directories pointed at an empty directory.
  It ran `init`, `update`, `capture install`, `start`, `check`, `finish`, `doctor`,
  `context`, `handover`, `serve ensure`/`stop`, `worktree create`, `update` again,
  `generate --check`, `generate`, `completion install` and `continuity publish`. After each
  step it recorded every changed path outside the two namespaces, every file in `.git/`
  except objects, `git config --local`, every file under `HOME`, and every sibling directory.
  The same run was repeated with `init --personal`.

The default part of the measurement is a test, not a memory: case
`test/cases/3113_the_adoption_footprint_is_inventoried.sh` adopts a new repository with
`init`, `update` and `capture install`. It then requires the set of paths outside the two
namespaces to equal the list in [the default adoption](#1-the-default-adoption), requires
this document to name each of them, and requires a personal adoption to leave `git status`
empty while its exclude block names every path it wrote. A writer that starts leaving a new
path behind fails the case until this document records it.

This replaces the first reading of 2026-09-06 (`948dbd93`, still in history as
`95d9a773c7f`). [What changed since that reading](#7-what-changed-since-the-first-reading)
lists what it got wrong or what has moved since.

## Classes

Every row carries one class. The class says why the path exists, and the contract decides on
that reason:

<div class="overflow-x-auto" tabindex="0">

| Class | Meaning |
|---|---|
| `canonical` | Once written, it is the person's file. Majordomus seeds it and then does not own it. Rewriting it would overwrite somebody's work. |
| `derived` | Computed from this repository's own `.ai/` content. It is committed, regenerable and stamped or bannered, and it is never read back as input. |
| `generated` | Laid down from the tool's distribution (`share/`). It is byte-identical in every repository at one version and follows the tool, not the repository. |
| `cache` | Recomputable state with no meaning of its own. Deleting it costs time and nothing else. |
| `runtime` | State of one clone, one machine or one process. It never travels through git as content, though it may live in git's own directory or refs. |
| `accidental` | Written in an adopter although it serves no purpose there: a projection of Majordomus's own source tree, or output that landed at the repository root for want of a namespace. |

</div>


The fate column uses three words. **Bridge** means the path stays where it is because a
consumer outside Majordomus reads it there, and the ownership ledger records it (I1109).
**Move** means it goes into `.ai/` or `.majordomus/`. **Stop** means it stops being written
in an adopter.

## 1. The default adoption

`majordomus init`, `majordomus update`, `majordomus capture install`: the three commands
[`ADOPTION.md`](@/docs/adoption.md) names. Case 3113 requires this list to be exact.

<div class="overflow-x-auto" tabindex="0">

| Path | Written by | Mode | Recorded by | Class | Fate |
|---|---|---|---|---|---|
| `.gitignore` — the lines `.ai/local/` and `.ai/repo/**/.tmp.*` | `init`: `lib/init.sh:349-356` (`mj_init_gitignore`, append-once per line); also `migrate` (`lib/migrate.sh:207`) | appends to the person's file and creates it if absent | the printed `created` list only (`lib/init.sh:354`) | `canonical` (the host file) | **move**: both lines express the boundary of `.ai/` and fit in an `.ai/.gitignore` (`local/`, `repo/**/.tmp.*`), which needs no root line at all |
| `AGENTS.md` | `update`: `lib/update.sh:134-135`, template `share/providers/agents.tmpl`, projection declared at `share/skeleton/policy.yaml:202-209` | whole file, stamp on line 1; refuses a hand edit without `--force` (`lib/update.sh:98-111`) | the stamp (policy hash + content hash); ledger event `projections.updated` with no target names (`lib/update.sh:146`) | `derived` | **bridge**: every agent provider reads it at the root; in the target state it becomes the one managed block (I1112) |
| `CLAUDE.md` | `update`, same code, `share/providers/claude-code.tmpl` | as `AGENTS.md` | as `AGENTS.md` | `derived` | **bridge** while Claude Code needs a root file of its own; one adapter per provider (I1114) |
| `GEMINI.md` | `update`, same code, `share/providers/gemini.tmpl` | as `AGENTS.md` | as `AGENTS.md` | `derived` | **bridge**, as `CLAUDE.md` |
| `.claude/settings.json` | `capture install`: `lib/capture.sh:2198-2200`, body at `:2393-2404` | **create-only**: an existing file is never rewritten; missing hooks are printed and the command exits 15 (`:2204-2220`) | nothing; ownership is inferred by grepping for the shim paths (`:2209`) | `canonical` (the person's settings after the first write) | **bridge**: Claude Code reads hooks only here |
| `.claude/hooks/majordomus-capture`, `.claude/hooks/majordomus-session-start`, `.claude/hooks/majordomus-session-end`, `.claude/hooks/majordomus-session-compact` | `capture install`: `lib/capture.sh:2255-2279`, table at `:249`, `:286` | create; rewritten only when the file carries the marker ``Written by `majordomus capture install` `` (`:2263`) | the marker line in the shim (`:2364`) | `generated` | **move**: `settings.json` names a command path, so the shims can live under `.majordomus/hooks/` and the settings file stays the only bridge |
| `.claude/skills/majordomus-brief/`, `.claude/skills/majordomus-handover/` | `capture install`: `lib/capture.sh:2326-2356`, copied from `share/mods/claude-code/*` | written while `session.brief_on_interval` / `session.handover_on_fill` are true (both true in the skeleton, `share/skeleton/policy.yaml:99-101`); **removed** file by file when the key is false (`:2311-2341`) | the marker on line 1 of `hooks/register.ts` | `generated` | **bridge**: Claude Code loads a mod only from `.claude/skills/` |

</div>


The measured run also produced `.claude/skills/majordomus-*/.claude-plugin/types/`, which
Claude Code lays down when it loads the mod and which each mod's own `.gitignore` excludes.

**A personal adoption** (`init --personal`) writes the same paths except `.gitignore`, and
adds one more:

<div class="overflow-x-auto" tabindex="0">

| Path | Written by | Mode | Recorded by | Class | Fate |
|---|---|---|---|---|---|
| `.git/info/exclude` — the block between `# >>> majordomus personal >>>` and `# <<< majordomus personal <<<` | `lib/common.sh:333-351` (`mj_personal_cover`), called through `mj_personal_guard` (`:356-361`) by `init`, `update` (`lib/update.sh:126`) and `capture install` (`lib/capture.sh:2199`, `2268`, `2275`, `2348`); removed by `init --shared` (`lib/common.sh:381-407`) | the block is rewritten whole (temp file and `mv`); every covered path is added with its `<path>.mj-tmp` sibling | **the block is the record**, and `mj_personal_verify` (`lib/common.sh:410-439`) checks it | `runtime` (clone configuration) | **bridge**: git reads nothing else for a clone-local ignore |
| `CLAUDE.local.md` instead of `CLAUDE.md`, when the project already tracks `CLAUDE.md` | `init --personal` rewrites the policy (`lib/init.sh:237-294`, `personal_target` at `share/providers.yaml:122`), then `update` writes it | as `AGENTS.md` | the stamp and the exclude block | `derived` | **bridge** |

</div>


`init --personal` exits 10 unless `git status` reads the same after it as before
(`lib/init.sh:166-171`). Case 3113 measures the same thing for the whole personal adoption.

## 2. What the lifecycle does after adoption

These writes happen after adoption, because the hooks `capture install` laid down are
running.

<div class="overflow-x-auto" tabindex="0">

| Effect | Written by | Mode | Recorded by | Class | Fate |
|---|---|---|---|---|---|
| A commit on the current branch carrying the episode's record under `.ai/repo/sessions/`: `git add` into the real index, then `commit-tree` and `update-ref HEAD` | the SessionEnd shim → `capture session --event end`: `lib/capture.sh:1908-1949` | plumbing, so **no commit hook runs**; gated by `session.commit_record_on_end` (true in the skeleton, `share/skeleton/policy.yaml:85`); skipped in personal mode (`:1915`) | the commit message `chore(session): record <id>` and the reflog entry `session record` | `runtime` (history mutation) | the path is inside `.ai/`; the **branch history and the index** are outside it, and the ledger has to name this as a mutation of history, not a file |
| `git fetch`, then `git push <remote> HEAD:refs/heads/<branch>` | `lib/capture.sh:1957-1986` | fast-forward only, through the repository's own pre-push hook | none | `runtime` | as above; it is gated by `session.push_record_on_end` (true in the skeleton, `:86`) |
| Loose objects from a temporary index | `finish --verify-command`: `lib/common.sh:68-79` (`mj_worktree_tree_id`), `lib/finish.sh:195-201` | adds unreachable objects; refs and the real index are not touched | the tree id in `MJ_FINISH_VTREE` | `cache` | stays: `git gc` collects them |

</div>


This is the largest footprint Majordomus has on an adopter that is not a file. With default
settings, every SessionEnd commits and tries to push.

## 3. Commands an adopter runs by choice

<div class="overflow-x-auto" tabindex="0">

| Path | Written by | Trigger | Mode | Recorded by | Class | Fate |
|---|---|---|---|---|---|---|
| `.claude/skills/<alias>/SKILL.md` | `apps/majordomus-cli/src/editors/claude.rs:26-42`, `editors/mod.rs:595-612` | `generate`, `generate editors` | refuses a file without the banner (`editors/mod.rs:885`); removes a stale bannered file (`:951-962`) | the banner (`editors/mod.rs:848`), sha256 and derivation key in `editors::plan`, `docs/generated/artifacts.json` | `derived` | **bridge** (the provider's discovery path). One skill per builtin capability alias is written **even in a repository with no skills**: the measured adopter got `.claude/skills/intent/SKILL.md` |
| `.agents/skills/<id>/SKILL.md` | `editors/mod.rs:75`, `:578-587` | `generate`, one per active skill under `.ai/repo/skills/` | as above | as above | `derived` | **bridge** |
| `.zed/tasks.json` | `editors/zed.rs:25,46` | `generate`, when a justfile has public recipes | banner-guarded overwrite | as above; **never removed** when its tasks disappear (the orphan scan covers only the two skills directories, `editors/mod.rs:923-948`) | `derived` | **bridge** |
| `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` | `apps/majordomus-cli/src/providers.rs:47-138` | `generate` | file mode overwrites the whole file **with no hand-edit check**, unlike the shell `update`; region mode splices between `<!-- majordomus:begin -->` and `<!-- majordomus:end -->` (`providers.rs:22-24`, `:94-108`) | the stamp (`providers.rs:141`), `artifacts.json` | `derived` | **bridge**, as in section 1; the two renderers are one defect (I1114) |
| `docs/generated/**` (86 files in the measured adopter: OpenAPI, CLI, registry, modules, benchmarks, providers, web, changelog, graph, economics, the manifest) | `apps/majordomus-cli/src/generate.rs:32` and the targets at `:574-650`, `:735-746`, `:942`, `:1158-1240`; manifest at `:1864-1990`; atomic write at `:2446-2470` | `generate` | overwrite through `.<name>.mj-tmp.<pid>` and `rename` | the provenance banner and `docs/generated/artifacts.json` | `derived` (the registry includes the adopter's declarative kinds) | **move** into `.majordomus/generated/`: `docs/` belongs to the adopter |
| `docs/INSTALL.md`, `site/static/install.sh`, `site/data/registry/distribution.json` | `generate.rs:900-1000`, `distribution/mod.rs:50-55` | `generate` | overwrite | `artifacts.json` | `accidental`: Majordomus's own installer and installation page, written into a foreign repository | **stop**: the guard that already keeps `share/*` and the design outputs out of an adopter (`share_is_here`, `crate_is_here` at `generate.rs:697`, `:823-830`) does not cover them |
| `site/data/registry/{registry,evidence-subjects,why,why-graph,product,product-graph}.json` | `generate.rs:36`, `:786-818`; `evidence/subject.rs:98`; `site.rs:849`, `:962`, `:1325` | `generate` | overwrite | `artifacts.json` | `accidental` in an adopter with no Zola site | **stop**, or **move** with `docs/generated/` |
| `deploy/Dockerfile`, `.dockerignore` | `deploy/render.rs:16-19`, `:348-361`; `generate.rs:837-860` | `generate`, only when the adopter declares `deployment` objects | overwrite | `artifacts.json` | `derived` | **bridge** (Docker reads them at those paths), opted into by declaring a deployment |
| `<parent>/<repo>-wt/` and `<repo>-wt/<branch>/` | `worktree/path.rs:21`, `:151-161`; `worktree/service.rs:922-986` | `worktree create`, `ensure`; measured: `adopter-shared-wt` appeared beside the adopter | create-only | the command's JSON report only | `runtime` | stays outside the repository by design (ADR 0021's topology); the ledger records that the container was created |
| `.envrc.local` in a linked worktree, a symlink to the primary's copy, made only when git ignores it | `worktree/direnv.rs:118-147` | `worktree create`, `ensure` | create-only | the report | `runtime` | stays (it is the person's file, shared) |
| direnv's approval store, through `direnv allow` | `worktree/direnv.rs:184-188` | `worktree create`/`ensure`/`migrate`, when the primary is already approved | direnv's own write | the report | `runtime` | stays; named so that uninstall knows it is not Majordomus's to undo |
| `tmp/archives/<repo>-<profile>-<date>.{zip,tar.gz}` | `lib/archive.sh:325-334` | `archive` | create; refuses an existing file without `--force` | none | `runtime` | **move**: `tmp/` is not ignored by `init`, so the archive shows as untracked in the adopter |
| `tmp/packs/<repo>-<profile>-<commit>/` with `pack.json` | `apps/majordomus-cli/src/pack/mod.rs:76`, `:1193-1282` | `pack` | replaces only inside a directory that already holds `pack.json` | `pack.json` | `runtime` | **move**, as `tmp/archives/` |
| `tmp/majordomus-migrate-backup/<utc>/` | `lib/migrate.sh:155-168` | `migrate` (pre-`.ai` layouts only) | create, verified copy | ledger event `layout.migrated` (`lib/migrate.sh:216`) | `runtime` | **move** |
| `target/web/**`, `target/site/` | `web/manifest.rs:19`, `:57-70`; `web/discover.rs:35-41`; `web/report/*`; `web/compose.rs:20`, `:42-134` | `web manifest`, `web report`, `web compose` | overwrite; **`target/site/` is wiped and recopied** | `surface.json` | `runtime` | **move**: in a Rust adopter `target/` is Cargo's |
| any path given to `--out` / `--file` | `commands/generate.rs:45-48`, `commands/evidence.rs:155-162`, `lib/usecase.sh:762`, `:814` | explicit | overwrite | none | — | stays: the person chose the path |

</div>


## 4. The git directory, config and refs

<div class="overflow-x-auto" tabindex="0">

| Effect | Written by | Trigger | Recorded by | Class | Fate |
|---|---|---|---|---|---|
| `.git/config`: `core.repositoryformatversion=1`, `extensions.worktreeConfig=true` | `apps/majordomus-cli/src/worktree/hooks.rs:265-281` | `worktree create`/`ensure`/`migrate`/`repair`, when the primary sets a `core.hooksPath` inside itself | the command's JSON (`HooksPath::Set`), not persisted | `runtime` | **bridge**: git reads it only there; the ledger has to cover clone configuration, not only files |
| `.git/worktrees/<n>/config.worktree`: `core.hooksPath` | `worktree/hooks.rs:219-223` | same | same | `runtime` | **bridge** |
| `refs/heads/<branch>`, `.git/worktrees/<n>/` | `worktree/service.rs:924-940` (through `git worktree add`) | `worktree create` (measured) | none | `runtime` | stays (git's own bookkeeping) |
| `.git/majordomus/locks/worktrees.lock` | `worktree/lock.rs:24`, `:60-96` | `worktree remove`/`repair`/`migrate` | none | `runtime` | stays: the git common directory is the one place every worktree shares |
| `.git/majordomus/integration/**`, `.git/majordomus/locks/integration-<base>.lock` | `integration/mod.rs:83`, `:116-150`; `integration/drain.rs:303-372`, `:823-832`, `:2403-2557` | `prs` | `events.jsonl` | `runtime` | stays |
| `refs/majordomus/continuity`, `refs/majordomus/remotes/<remote>/continuity`, and a push of the first to the remote | `continuity/store.rs:35-39`, `:245-294`, `:410-445` | `continuity publish`/`sync`/`resume` (measured: `continuity publish` created `refs/majordomus/continuity`) | the signed records themselves | `canonical` (a person's handovers, published) | stays: `refs/majordomus/` is a third namespace, in git's ref space |
| `refs/majordomus/prs/<n>` (forced mirror) | `integration/forge.rs:45`, `:1736-1809` | `prs refresh` | none | `cache` | stays under `refs/majordomus/` |
| `refs/offload/in-<tag>`, `refs/offload/<tag>-derived`; then files staged in the person's worktree | `offload/run.rs:387-388`, `:584-601`, `:655-659` | `offload`, only when `.ai/repo/offload/*.yaml` is declared | none | `runtime` | **move** to `refs/majordomus/offload/`: the only ref Majordomus writes outside its ref namespace |

</div>


There is no `git config` write in `bin/majordomus` or `lib/*.sh`. `core.hooksPath` and
`merge.derived.*` appear there only as advice that `doctor` prints (`lib/doctor.sh:157-161`,
`:228-230`). No command writes `.git/hooks/`. `init` prints the two hook lines and installs
neither (`lib/init.sh:200-226`), and the measured run confirms `.git/hooks/` unchanged.

## 5. The machine, outside the repository

<div class="overflow-x-auto" tabindex="0">

| Path | Written by | Trigger | Recorded by | Class | Fate |
|---|---|---|---|---|---|
| `$HOME/.local/bin/majordomus`, `$HOME/.local/bin/majordomus-mcp`, `$HOME/.local/share/majordomus/versions/<v>/`, `$HOME/.local/share/majordomus/.lock` | `site/static/install.sh:41-42`, `:580-626` | the installer | `RELEASE.json` in the version tree | `generated` | outside the contract: an installation, not an adoption. The installer edits no shell startup file and only prints PATH advice |
| `$XDG_STATE_HOME/majordomus/node.json`, else `$HOME/.local/state/majordomus/node.json` (an Ed25519 secret, mode 0600) | `apps/majordomus-cli/src/mesh/identity.rs:140-165` | a server start with `.ai/repo/mesh` enabled; any continuity write; `continuity device` | none | `runtime` | outside the contract (one per machine, shared by every repository). The measured adopter enables no mesh, and `serve ensure` wrote nothing under `HOME` |
| a managed block in `~/.zshrc`, `~/.bashrc` or `~/.config/fish/config.fish`, plus `<rc>.majordomus.bak` | `commands/completion.rs:36-37` (markers), `:44-148` | `completion install` (measured: the block appeared in `$HOME/.zshrc`) | the markers; `--remove` takes it out | `generated` | outside the contract; already reversible |
| host firewall rules | `mesh/firewall.rs:489`, `:595-650`, `:1362` | `sudo majordomus mesh firewall apply` | none | `runtime` | outside the contract; explicit and privileged |

</div>


## 6. What already exists, and which phase extends it

Each later phase has a mechanism to start from. A phase that builds a second one beside it
repeats the defect this milestone exists to remove.

<div class="overflow-x-auto" tabindex="0">

| Mechanism | Where | What it already does | The phase that extends it |
|---|---|---|---|
| The personal exclude block | `lib/common.sh:311-439` (`mj_personal_cover`, `mj_personal_verify`) | a per-clone, marker-delimited record of every path the layer wrote, rewritten whole, verified, and removed again by `init --shared` | **I1109, the ownership ledger.** It is the ledger's first form, without hashes. The ledger adds the installed hash and covers config and refs |
| The provider stamp and the refusal of a hand edit | `lib/update.sh:98-111`, `lib/common.sh:907-967`; `providers.rs:141` | the hash a target was written with, compared with what is on disk | **I1111, uninstall**: "refuses to touch a bridge a person has edited" is this comparison. The Rust renderer has to gain the refusal before it can be the one renderer |
| The editors plan: banner, `state_of` (Current/Stale/Absent/Conflict), `conflicts`, `orphans`, `remove_orphans`, sha256 and derivation key | `apps/majordomus-cli/src/editors/mod.rs:545-962`, capability `editors.status` | desired files computed from the model, diffed against the tree, conflicts refused, orphans removed | **I1108, desired state and plan**, and **I1110, idempotence**. This is already a reconciler for one family of files; `init` is to become the same shape |
| `capture install` as reconciliation | `lib/capture.sh:2255-2356` | the desired set from policy keys; marker-guarded rewrite; removal of a mod whose key turned false | I1108 and I1112. The rule that a file without the marker is never touched carries over unchanged |
| `docs/generated/artifacts.json` | `generate.rs:1864-1990` | path, document, source, bytes and sha256 of every artifact `generate` writes | **I1103 / I1106**: the inventory of derived artifacts. It does not cover the stage-B outputs of `scripts/generate-site-data` |
| The foreign-tree predicates `share_is_here`, `crate_is_here` | `generate.rs:697`, `:823-830`; `commands/generate.rs:145` | keep `share/*` and the design outputs out of an adopter | **I1112 / I1116**: the same predicate stops the accidental rows of section 3 |
| The completion block | `commands/completion.rs:36-148` | a managed block with markers, `--dry-run`, `--remove` and a backup | **I1111 / I1112**: the shape every managed root block should have, including the only working uninstall today |
| The before/after `git status` equality of a personal init | `lib/init.sh:166-171`; case 3113 | proves that an adoption changed nothing the repository shares | **I1110 / I1122**: the idempotence and foreign-repository proofs measure the same way |
| Ledger events `projections.updated`, `layout.migrated` | `lib/update.sh:146`, `lib/migrate.sh:216` | append-only records of two mutations | I1109. `projections.updated` names no target, and `init` and `capture install` emit no event at all |

</div>


## 7. What changed since the first reading

These corrections are measured against `948dbd93`:

- **Completion and environment exist.** `majordomus completion install` writes a managed
  block in the shell's startup file, and `majordomus env` describes the checkout. The first
  reading said there was neither.
- **There is now one uninstall path:** `completion install --remove`. Nothing else is
  reversible.
- **`capture install` writes more than hooks.** It writes two Claude Code mods under
  `.claude/skills/`, and removes them when their policy key is turned off. `generate` writes
  editor skills under `.claude/skills/` and `.agents/skills/` and a `.zed/tasks.json`. None
  of these were in the first reading.
- **`share/allow/commands.txt` and `share/allow/events.txt` now say what they are.** Both
  carry an `AUTHORED FILE — no generator derives it yet` banner. They are still authored
  files in a generated directory.
- **A personal adoption exists**, and it is the one adoption that already keeps a record of
  what it wrote (section 1).
- **Still true:** `init` refuses a second run with exit 15 (`test/cases/01_init.sh`); nothing
  reverses `init`, `update` or `capture install`; `share/events.yaml` has no `init` event;
  `README.md:476` still says every generated file comes out of the one executable, which
  `scripts/generate-site-data` still contradicts.

## 8. Defects this inventory found, recorded and not fixed here

This phase changes no path. Each defect below belongs to the phase named:

- `generate` in an adopter writes Majordomus's own `docs/INSTALL.md`, `site/static/install.sh`
  and site registry (section 3) → I1112 / I1116.
- The Rust provider renderer overwrites a hand-edited `AGENTS.md` in file mode, and `migrate`
  runs `update --force` (`lib/migrate.sh:220`), which does the same → I1111 / I1114.
- `update` writes `$MJ_ROOT/$target` without the `../` and absolute-path guard
  `mj_norm_path` (`lib/common.sh:694-707`) → I1102.
- A stale `.zed/tasks.json` is never removed → I1110.
- Worktree creation turns on `extensions.worktreeConfig` in the shared `.git/config` and says
  so only in its JSON → I1109.
- `refs/offload/*` sits outside `refs/majordomus/` → I1104.
- The SessionEnd commit bypasses the repository's commit hooks by construction → I1109
  records it; whether it stays default is a policy decision, not this inventory's.

## 9. Ordered slices

These are the milestone's issues, in the order the inventory supports:

1. **I1102 / I1104: the namespaces as a typed contract.** `.ai/`, `.majordomus/` and
   `refs/majordomus/` are the namespaces. `.ai/.gitignore` takes the boundary lines out of
   the root `.gitignore`.
2. **I1109: the ownership ledger**, grown out of the personal exclude block, covering files,
   clone configuration, refs and history mutations, each with the hash it was written with.
3. **I1108 / I1110: `init` as a reconciler** in the shape the editors plan already has:
   desired, actual, conflict, orphan. A second run changes nothing.
4. **I1112: the root reduced to bridges.** The `.claude/hooks/` shims move under
   `.majordomus/`, and the accidental rows of section 3 stop.
5. **I1111: uninstall**, using the stamp comparison for refusal and the completion block's
   `--remove` as the pattern.
6. **I1116: this repository's own footprint**, moved through the reconciler, with a dry run
   first.

## 10. Risks

- **An inventory that a writer outruns is worse than none.** It authorises what it no longer
  describes. That is why the default adoption is a test (case 3113), not only a table. The
  optional commands of sections 3 to 5 are held by this reading alone until the ledger
  records them.
- **The shell tool and the Rust executable are two implementations of one product.** The
  adoption lifecycle lives in the shell one, and `generate` lives in the Rust one. Both
  write the root provider files, and only the shell one refuses a hand edit. Any slice that
  moves a writer across that boundary has to leave one writer, not two.
{% endraw %}

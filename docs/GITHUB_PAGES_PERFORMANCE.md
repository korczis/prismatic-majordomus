# GitHub Pages performance

How a push to master becomes public, what it is allowed to cost, how that is measured, and
what stops the next change from quietly taking the speed back.

[`GITHUB_PAGES_ARCHITECTURE.md`](GITHUB_PAGES_ARCHITECTURE.md) says what the site *is* — the
canonical sources, the generators, the checks. This document is about the *path*: latency,
budgets and the evidence behind both. [`CI.md`](CI.md) is the validation pipeline beside it.
[`PAGES_STATUS.md`](PAGES_STATUS.md) carries the current numbers, generated; nothing here is
a measured value, on purpose.

## The objective

The number that matters is not how long a workflow runs. It is

```text
T_public = when the public site serves this commit  −  when the push was accepted
```

The target is `T_public < 60 s` whenever GitHub's own scheduling is not the limiting factor,
and the distinction in that sentence is load-bearing. Two halves:

```text
controlled   checkout, tool setup, build, check, the push to gh-pages
external     the Actions queue, runner allocation, GitHub's own "pages build and deployment"
```

Only the first is this repository's to spend. Both are measured; only the first is budgeted
and only the first can fail a run. A deployment whose controlled path was 30 s and whose
runner waited 4 minutes in a queue is not a regression here, and the job summary says so in
those words rather than reporting one number that hides which half moved.

## The shape

```text
push to master (on paths that can change the site)
        │
        ├─── validate.yml ── plan ─┬─ structure ─┬─ ci  (the status a branch rule requires)
        │                          ├─ suite      │
        │                          ├─ rust       │      decides whether a change may merge
        │                          ├─ coverage   │
        │                          ├─ bench      │
        │                          ├─ site       │      (build, check, browser probe)
        │                          └─ macos ─────┘
        │
        └─── pages.yml ──── one job ───────────────────  decides what the public site shows
                 checkout → setup → build → check → push gh-pages → measure publication
```

The two run beside each other on the same commit. Publication used to be a job *inside*
`validate.yml` that needed `plan`, `structure`, `suite`, `rust` and `site` — the set that
decides whether a change may merge, most of which cannot change a published byte. The site
therefore went public tens of minutes after the push that changed it.

What is on the publication path is everything that can make the published bytes wrong:

- the committed derived data is current for this tree,
- the site builds from it,
- every static check over the output passes (`scripts/site-check`: metadata and landmarks on
  every route, no unrendered template delimiter, every internal link resolving, no remote
  asset, no credential, every tile a page, the graphs connected, the served provenance this
  commit's).

What is not, and why it is safe: the behavioural suite, the crate's gates, coverage, the
macOS suite, the benchmark check and the browser probe. Each of those guards *the repository*
rather than *the bytes*, each still runs on the same commit in `validate.yml`, and each still
gates merging through the `ci` status. Moving a gate off the publication path is only sound
while it is still somewhere; `test/cases/97_pages_fast_path.sh` fails if one of them appears
in `pages.yml` or disappears from `validate.yml`.

The browser probe is the interesting case. It can find a real defect in the published bytes —
a route that overflows at 320 px — and it costs minutes. It stays in `validate.yml`, where it
gates merging, because a route that overflows is a defect to fix rather than a deployment to
block: holding every publication for it would cost every commit the full probe to protect
against the rare one that regresses, and the commit that regresses is not published any
faster by having waited.

## What made it slow, and the shape of each fix

Every value named below is measured, and none of them is written here. The current ones are
in the tracked baseline of the machine that measured them,
[`.ai/repo/benchmarks/pages/`](../.ai/repo/benchmarks/pages/), written by
`scripts/pages benchmark --write-baseline`; the before-and-after of each change is in the
commit that made it, which is where `project.performance-evidence` puts them. Where the time
goes inside one generation is `MJ_TIMING=1 scripts/generate-site-data`, ranked by phase.

### The generator ran three times for one commit

`site-build` called `generate-site-data`; `site-check`'s first assertion was
`generate-site-data --check`; and the `site` job ran `generate-site-data --check` before both
of them. Three full generations of the same data for one commit, each of them dominated by
`majordomus usecase run`, which executes all 31 use-case scenarios in a disposable repository
each — a test suite, sitting inside a data generator, on the deployment critical path.

The fix is not to make generation faster; it is to stop asking the same question three times.
`site-build --no-data` renders committed data; `site-check --no-sync` skips the sync
assertion; the `site` job keeps exactly one generation, the `--check` pass, which is the only
one of the three that actually *proves* anything (it stages a fresh generation from the
canonical sources and diffs it against what is committed).

The publication path needs the same answer and cannot afford even one generation, so it asks
a cheaper question with the same meaning:

```bash
scripts/generate-site-data --fingerprint
```

This is the generator's own `source_hash` — the hash of its own 467-entry canonical input
list, in the line format `source.json` has always been computed from — evaluated in one
hashing process instead of 467, which is the difference between a fifth of a second and the
better part of a minute. `mj_inputs_hash` is now the single implementation of it, so
`--fingerprint` and `source.json` cannot disagree. `scripts/pages current` compares the two:
equal means the committed projection is current for this tree and rendering it *is* rendering
the canonical sources.

A tree whose committed derived data is stale is **refused**, not regenerated. The derived data
is a projection; a projection that disagrees with its sources is a defect to fix at the
source, and a publication path that quietly papers over it would be publishing something no
gate had seen.

### The derived content is committed for the same reason

`site/data/generated` is only half of what the generator produces. The other half is the
derived *content* — `site/content/{docs,guarantees,commands,doctrines,plan,use-cases,
applications,skills,registry,profiles,supervises,why,outcomes}/` and the projections of
`site/content-src/*.md` — and Zola renders that, not the JSON. So it is committed too. A
fresh clone has no other way to obtain it: `site-build --no-data` does not run the generator,
which is the whole of why the build fits its budget, and generating the content instead would
cost about twice the twenty seconds `build` is allowed. Left ignored, the fast path could not
build a clone at all; the first link into a section that does not exist fails the whole render.

What keeps a committed projection honest is not ignoring it but the same gate the data lives
under. `scripts/generate-site-data --check` diffs every derived page against a fresh staging
generation and names every orphan, so an edited or stale page is refused exactly as stale data
is, and `scripts/site-check` asserts the sections are committed and unignored rather than the
reverse. Nothing under `site/content/` is anybody's to edit by hand: the file it is projected
from is the original, and `--check` says so the moment the projection moves.

### `site-check` spawned seven thousand processes

Five loops over the 416 built pages, each spawning grep, sed and awk per page: about eighteen
processes per route. `scripts/lib/site-pages.awk` carries all of those assertions in one
process. It is a transcription, not a redesign — output is byte-identical on this site, and
fifteen deliberately mutated pages (one per assertion, plus a control) produce the same
findings from the old implementation and the new.

### The result

```bash
scripts/pages benchmark -n 5                       # the distribution, as JSON
scripts/pages benchmark -n 5 --write-baseline      # and into this platform's baseline
cat .ai/repo/benchmarks/pages/baseline.*.json
```

An order of magnitude off the controlled build-and-check path, and almost all of what is
left is `site-check`. That is the next thing worth attacking, and it is named as such under
*Remaining bottlenecks* rather than left implicit.

## The model

Everything the path is allowed to cost, and everything it triggers on, is declared once in
[`.ai/repo/ci/pages.yaml`](../.ai/repo/ci/pages.yaml). Neither the workflow nor the script
carries a budget, a path list or a gate name of its own.

```bash
scripts/pages budget          # the controlled budgets
scripts/pages budget --json   # the same, for a reader that is not a person
scripts/pages paths           # what a push must touch for the site to be able to change
```

### Trigger invalidation

The trigger paths are **derived**, never written twice. The model names the gate that builds
the site (`site-build`); the classes of [`gates.yaml`](../.ai/repo/ci/gates.yaml) that name
that gate are the classes whose paths can change the site; their pathspecs plus the model's
own `extra` are the workflow's `paths:` block.

A class that escalates the whole plan (`gates: full`) is deliberately *not* one of them. That
escalation says how much validation a change to the pipeline deserves; it says nothing about
what the published bytes are derived from. Changing `scripts/ci-plan` or the `justfile` makes
every gate run and cannot move a pixel, so it starts no deployment. Zero work is faster than
optimised useless work.

`test/cases/97_pages_fast_path.sh` diffs the workflow's block against `scripts/pages paths`
and fails on any difference, so the two cannot drift. When you change a class in `gates.yaml`,
regenerate the block:

```bash
scripts/pages paths | sed 's/^/      - /'     # paste into the paths: block of pages.yml
```

### Budgets

Budgets are controlled latency only, in seconds, warm, on a GitHub-hosted Linux runner. They
were set from measurement — the local benchmark above and the timing rows every deployment
writes — and a change to one is expected to cite the run that justified it
(`project.performance-evidence`). A cold run pays for caches it cannot restore and has its own,
looser bound. The end-to-end target is reported against every deployment and **not** enforced,
because the larger half of it is GitHub's.

## Caches

Each domain is invalidated by its own inputs and nothing else, so a documentation change does
not evict a toolchain and a lockfile change does not evict the site.

| cache | key | what a hit saves |
|---|---|---|
| `node_modules` | runner, Node major, `package-lock.json` | the whole `npm ci`, not merely its downloads |
| npm's own cache | `actions/setup-node` | the downloads, when the first misses |
| Zola | `taiki-e/install-action` | the release download |


There is deliberately **no cache of generated site data or of rendered output**. The committed
projection already plays that role, it is a tracked file rather than a cache, and it is proved
current in 169 ms — a cache in front of that would be a second source of truth bought for
nothing (`project.cache-is-invisible`, `project.derived-once`).

To invalidate a cache on purpose, change its key input: bump the Node major or the lockfile,
or the Zola version in `.github/actions/setup-site/action.yml`. Deleting every cache must
produce the same published bytes; that is not an aspiration but an assertion —
`test/cases/97_pages_fast_path.sh` compares a build that skipped the generation with one that
did not, byte for byte, and `test/cases/51_derived_artifacts_committed.sh` regenerates every
derived artifact from its canonical sources and refuses a difference.

## How publication is measured

Workflow success is not publication. Every build writes `site/static/build.json`, served at
`/build.json`, naming the commit it was built from — a fact of the build, never a committed
file. After the push to `gh-pages`, the job polls that URL with the cache defeated until it
names this commit:

```bash
scripts/pages verify --commit "$(git rev-parse HEAD)"
```

That wait is GitHub's own "pages build and deployment", which serves the branch. It is
reported as external latency; it fails nothing, because GitHub being slow to serve is not this
repository failing.

The job summary of every deployment therefore carries the phases against their budgets, the
controlled total against its budget, the queue and the Pages build beside them, and one
verdict line:

```text
Pages SLO: PASS — controlled path <n> s / <budget> s, push to public <n> s / <target> s
```

and, when the controlled path is inside its budget and the total is not, the summary says so
in those words rather than reporting a failure: the remainder is the Actions queue and
GitHub's own Pages build, which this repository does not spend, and that is an external limit
rather than a regression here.

## Locally

The same commands CI runs. There is no GitHub-only build semantics.

```bash
scripts/pages current            # is the committed derived data current for this tree?
scripts/pages build              # render it
scripts/pages check              # every static check over the output
scripts/pages benchmark -n 5     # the controlled path, as JSON
scripts/site-deploy              # the full gate and the push, the way a person deploys
```

`scripts/pages build` refuses a tree whose derived data is stale and tells you the two hashes.
The cure is always the same: `scripts/generate-site-data` (or `just derive`) and commit the
result.

## Deploying by hand is still the fastest path

The site is served from the `gh-pages` branch, and whoever pushes that branch deploys
(`docs/claims/site-deploy-one-path.md`). A person at a terminal therefore has no Actions
queue in front of them at all:

```bash
scripts/site-deploy              # gate, build, check, push
```

This is why publication was **not** moved to the native Pages artifact flow
(`actions/configure-pages` → `upload-pages-artifact` → `deploy-pages`), which would shave the
seconds GitHub spends building the branch: pointing GitHub Pages at "GitHub Actions" as its
source would make a workflow run the *only* way to publish, and would put the Actions queue in
front of the one path that today has none. The branch source costs GitHub's own "pages build
and deployment" and keeps the hand deploy. What that costs is measurable rather than
memorable:

```bash
gh run list --workflow "pages-build-deployment" --limit 20 \
  --json createdAt,updatedAt --jq '.[] | (.updatedAt | fromdate) - (.createdAt | fromdate)'
```

If the queue ever stops being the dominant term and that becomes the binding constraint, the
trade is worth revisiting — with the measurement, not with the preference.

## Remaining bottlenecks, by expected benefit

1. **`site-check`, 9.7 s of a 10.3 s controlled build.** Its remaining cost is the per-record
   loops that shell out to `jq` once per claim, command, capability, use case and plan record.
   The same treatment that took the per-page loops from 25 s to under a second applies:
   one `jq` program per family instead of one process per record.
2. **The Actions queue.** On this repository, with several worktrees pushing at once, runs
   have waited minutes for a runner (`gh run list --json createdAt,startedAt`). Nothing in
   these files shortens that. The escalation, if the controlled path is already comfortably
   inside its budget and the queue is what keeps `T_public` over a minute, is a warm
   self-hosted runner labelled for this job only — evaluated on measurements, with its own
   security, isolation and maintenance cost stated, and not before.
3. **GitHub's own Pages build of `gh-pages`.** Removable only by giving up the hand deploy
   path, as above.
4. **`npm ci` on a cold cache.** Bounded by the `node_modules` cache; the cold path is what
   the cold budget covers.

## Changing any of this

- A new canonical input of the site generator goes into the `INPUTS` list of
  `scripts/generate-site-data` and nowhere else; the fingerprint, the freshness check and the
  trigger follow from it.
- A new path that can change the site goes into a class of `gates.yaml` that names the
  `site-build` gate; then regenerate the workflow's `paths:` block as above.
- A new budget, or a changed one, goes into `.ai/repo/ci/pages.yaml` and cites the run that
  justified it.
- A new check over the published bytes goes into `scripts/site-check`, which the publication
  path runs. A new check over the repository goes into the gate model, which it does not.

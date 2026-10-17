# `majordomus ui` — the product

How the delivery is shown to a person: what the UI is, how it relates to the Cockpit, where
every value on it comes from, how it stays fast and current, and how a screen is added. It
describes behaviour as implemented and tested; where the two disagree, this document is
wrong and changes in the same commit as the fix.

- The decision is
  [ADR 0135](https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/adrs/0135-majordomus-ui-is-the-primary-surface.md).
- The rule the UI is held to is `project.ui-derived-state`.
- The feature record is `.ai/repo/features/ui.md` (stable since 2026-10-12, when the owner
  accepted ADR 0135).
- The claim a reader can check is `ui-dashboard-cites-its-sources`
  (`docs/claims/ui-dashboard-cites-its-sources.md`), proven by
  `apps/majordomus-cli/tests/ui_surface.rs`.
- The owner's brief, with its reference screens, is
  `campaigns/majordomus-ui-complete-opus-fable-v3-20261011/`.

## The product and its technical reference

The shared server of a checkout serves two surfaces a person reads.

| Surface | Mount | What it is |
|---|---|---|
| The UI | `/ui` | the product: the delivery in the project's own words, dense, linked and clickable |
| The Cockpit | `/cockpit` | its technical reference: the registry rendered, one page per capability, object, graph and route |

The UI is where a person is sent first, and it is not a view of the registry: its navigation
is its screens, its words are the delivery's. It refers to the Cockpit only as the technical
view of what a screen read (ADR 0135), and in the frame:
- the "technical view" link on each source line, to the Cockpit's page for that capability;
- the technical view of an object a screen names and the UI has no screen of its own for (a
  document, a test, a declaring file, a feature's or a deployment's record), to the Cockpit's
  page for that object;
- the Cockpit entry among the other surfaces in the top bar, and the top bar's search, which
  is the Cockpit's search (`/cockpit/search`) until the UI has a search of its own.

Both surfaces are projections (ADR 0012): they hold no data, they compose capabilities
through the executor every transport calls, and the same process serves them under one
content-security policy. The UI is a native surface of the web topology (`web.surfaces`
lists it), declared beside the Cockpit in `apps/majordomus-cli/src/web/discover.rs`.

## The screens

| Screen | Route | Reads |
|---|---|---|
| Dashboard | `/ui` | `repository.info`, `health.report`, `release.version`, `distribution.releases`, `plan.status`, `plan.roadmap`, `mesh.peers`; the queue on its own route |
| Command | `/ui/command` | `commands.list` (its `origin` filter is the screen's tabs), `executions.list`; `capabilities.list` for the capability a command is |
| Intent workspace | `/ui/idd` | `intents.list`, `plan.roadmap`, `plan.issues`, `plan.waves`, `plan.next`: one intent (`?intent=`, else the first `intents.list` reports executing, then verifying, planned, declared — the rule is printed) with its inspector, the milestones it names in the roadmap's order with what each waits for, every issue of those milestones or naming the intent by wave with its status, priority, blockers and the move its status allows; `?issue=` opens that issue's move panel on the screen itself, the form the Cockpit's issue page renders, wired by the one module both surfaces load (`share/cockpit/moves.js`, named by the page as `data-moves-lib`) and made by `plan.transition` as an execution, which refuses a move the plan does not allow and names why, the waves those issues occupy with the scope conflicts that serialise them, and their dependency graph over a table of the same edges. The screen decides no status, wave or membership of its own |
| Roadmap | `/ui/roadmap` | `plan.roadmap`, `plan.status`, `intents.list`; one milestone (`?milestone=`) in the inspector, with `plan.issues`: its issues, what can start, its critical path and its neighbours; one intent (`?intent=`) in its inspector: its statement, invariants and criteria with their evidence, and the milestones and issues that serve it |
| Issues & PRs | `/ui/work` | `plan.status`, `plan.issues`; the queue on its own route, one pull request (`?pr=`) in its detail |
| Issues | `/ui/issues` | `plan.issues`, `plan.status`, `plan.roadmap`: every issue, narrowed by `?status=` (the plan's own statuses as tabs), `?milestone=`, `?priority=` and `?q=`, each leading to its inspector |
| PR Drain | `/ui/prs` | `plan.status`, `plan.issues` (one issue, `?issue=`); `integration.queue` and `integration.events` on its own route (`/ui/panels/drain`), narrowed by `?disposition=`, `?lane=` and `?q=`, one pull request (`?pr=`) in the inspector with what the executor did to it, the lane's recent activity, and its merges per day over the fortnight the trail ends on |
| Capabilities | `/ui/capabilities` | `repository.info`, `capabilities.list` by kind; one capability (`?id=`) from `capabilities.describe` |
| Mesh | `/ui/mesh` | `mesh.peers`, `mesh.events`; one machine (`?node=`) in the inspector |
| CI/CD & Releases | `/ui/delivery` | `distribution.releases`, `release.version`, `deploy.list`, `gates.model`, `integration.timings` (each check's median, 90th percentile and longest run, and the open heads that waited longest, each leading to its pull request) |
| Deployments | `/ui/deployments` | `deploy.list` (what the layer declares); `deploy.observed` (what the forge recorded: where each environment stands, every deployment to every environment — GitHub Pages among them — and every release, each commit, ref, environment, run and release a link); `delivery.report` on its own route |
| Evidence | `/ui/evidence` | `evidence.report`; one claim (`?claim=`) in its inspector from `evidence.claim`: what settles it, its last run, what changed since, how to reproduce it, and the claims its test also proves |
| Knowledge | `/ui/knowledge` | `knowledge_base.status`, `knowledge_base.candidates`: the candidates awaiting review, newest first and bounded with what is left out counted, the subjects they are about, and whether the deriver is still writing. The review's commands are named as text; the screen promotes and rejects nothing |
| Documentation | `/ui/docs` | `objects.list` (features, documents), `web.surfaces` |
| Governance | `/ui/governance` | `rules.report`, each rule named and leading to its inspector; one rule (`?rule=`) from `rules.show`: what it states, how it is enforced, the tests that prove it, what is missing, and the rules around it |
| Observatory | `/ui/observatory`, and `/ui/kiosk` as its preset | `mesh.peers`, `mesh.events`, `mesh.state`, `plan.roadmap`, `distribution.releases`, `release.version`, `health.report`, `economics.runs`, `repository.info`, `rules.report`, `evidence.report`, `quality.report`, `plan.waves`, `distribution.status`, `capabilities.list`: fifteen scenes, each drawn from what its sources answered or named as unavailable with the source's reason ([The Observatory](#the-observatory)) |

The navigation is `SCREENS` in `apps/majordomus-cli/src/ui/mod.rs` and nothing else. A test
compares it with the dispatcher's arms, so a screen without a route, or a route without a
screen, fails. A screen of the owner's concept that has no producer yet is not in the
navigation: a link to nothing is a dead end.

Every screen has the same shape:
- a hero band, with the screen's glyph, title, tagline and lead, the brand mark lit behind
  them, and a row of KPI tiles;
- a grid of panels;
- the frame around both: the sidebar of screens with the version card at its foot, and the top
  bar with the search, the time the screen was read, and the other surfaces (documentation,
  API, Cockpit).

KPI tiles lead to the screen that explains them. A panel whose whole is on another screen
says "View all". A milestone row leads to that milestone on the Roadmap, where the row it
lands on is lit. A capability row opens its detail (`?id=`), a pull request of the queue opens
its detail (`?pr=`), and a machine of the mesh — in its table or as a node of its graph — opens
its inspector (`?node=`); every selection is a URL, so it survives a reload and the browser's
Back.

## What a screen names is a link

Every entity a screen names leads somewhere (I2481):

| What is named | Where it leads |
|---|---|
| an issue, a pull request, a milestone, an intent, a claim, a rule, a capability, a machine | its inspector in the UI |
| a gate | its row in the CI model, `/ui/delivery#gate-<id>`; the gates it implies lead to theirs |
| a deployment | its row on Deployments, `/ui/deployments#deployment-<id>` |
| a deployed commit, the ref it was deployed from, an environment, the run that deployed it, a published release | the forge's page for it, built from the repository address the forge itself gave when it was observed (`deploy.observed`) |
| an issue or a milestone that a milestone's own prose names | its inspector; the prose is the record's, drawn with its markup (`ui/prose.rs`) |
| a command the registry also serves as a capability | that capability's inspector; the two are one when the capability's declared command-line path is the command's invocation |
| a release | the record the distribution publishes for it, at the address `distribution.releases` gives |
| a document, a test, a feature, a rule's or a deployment's declaration | its technical view, the Cockpit's page for that object |
| the public address the delivery report probed | that address |

A link to a technical view is drawn only when the index holds the object: the source said so
(a rule's own URI, a test the proof found on disk), or `objects.get` resolves it. What the
index does not hold is text, so no link leads to a page that answers 404. Two kinds of name
stay text because nothing derives where they lead: a commit outside the Deployments screen,
since no capability the other screens ask at first paint states the forge's address (the PR
Drain, which asks the queue, and Deployments, which asks what was observed, do link the
forge), and a path that is not an object of the index.

What the forge recorded is read from one observation, taken by `majordomus served forge` and
kept beside the pull-request observation, which `majordomus prs refresh` takes. A checkout
that has never asked says so, with that command, instead of drawing empty tables; the
published snapshot's job asks before it reads, so the public Deployments and PR Drain are not
empty.

A list a screen cuts short says so: "The first 8 of 31; 23 more not shown here", with a link
to the screen that shows them all when there is one. Case 1091 and
`apps/majordomus-cli/tests/ui_links.rs` read every screen through a real server and follow
every link under `/ui` and every technical view it draws; one that does not answer fails them.

A pull request's detail is read from the queue's own answer, so it costs no second call: what
the pull request is, every gate with its verdict, the evidence each verdict was read from, why
it stands where it stands, the next action that would move it, the paths it authors and the
factors of its rank. A number the queue did not observe is said to be unobserved.

## Where a value comes from

Every value is a reading: the value at an RFC 6901 pointer into one capability's answer
(`apps/majordomus-cli/src/ui/source.rs`). A tile shows its capability and pointer under its
value. A panel closes with its sources. Each source is named with how long it took, and with
every other way to ask the same question, as the capability declares them through
`capabilities.describe`:
- **API**: the route, linked, so a browser opens the same answer as JSON;
- **CLI**: the command a person types, `majordomus <path>`;
- **MCP**: the tool an agent calls;
- **technical view**: the capability's page in the Cockpit.

Nothing is computed on the UI's behalf:
- A list it shows is a list a capability returned.
- A count is the number at a pointer, or the length of the list there.
- A meter is a picture of the two numbers written beside it.
- A donut draws the counts its legend lists.
- A status word is the source's own word, styled by the design system's status vocabulary.
- A window is the source's own. The merge tiles are labelled with the queue's
  `throughput/window_days`, and the PR Drain's throughput chart counts the trail's merges per
  day over that same window; when the queue states no window, the tile says so and no day is
  counted. The chart's bars are the one thing a screen tallies itself, each from the trail's
  own `merge_succeeded` events of that day, with the count written beside the bar.

Classes that overlap, such as a rule report's coverage, are listed and never summed. What is
missing says so:

| The source | The UI shows |
|---|---|
| answered, and the pointer holds a value | the value (a fraction rounded to two places) |
| answered, and the pointer holds nothing | `none` |
| answered a row without its status word | `unknown` |
| could not answer | `not answered`, with the reason |

None of them is ever drawn as zero or healthy. The `ui-integrity` and `cockpit-projection`
gates hold the UI's sources exactly as they hold the Cockpit's:
`scripts/ci/cockpit-projection-check` scans both directories. Cases 389 and 547 prove it
refuses, in the UI, a read of the index or the registry and an absent count drawn as zero.

## Fast, and current

**A screen does not wait for its sources** (ADR 0142, rule
`project.no-screen-waits-for-a-source`). It asks for the *latest reading* of each one
(`Context::read`, `capability::readings`): what the process knows now, handed over from
memory. Handlers, git and files are touched by a worker behind the reader, never on the path
of the request.

| The source was | What the reader gets | What happens behind them |
|---|---|---|
| read in the repository as it is now | the reading, at once | read again once it is older than a second |
| read before the repository last changed | the new reading if it lands within 50 ms, otherwise the old one, marked stale | read again, with every other source read in the last five minutes |
| never read in this process | the first reading, waited for | nothing more; past a minute it is said to be pending |

The repository's own reload is not waited for either. When the repository has changed, a
screen is drawn from the generation the server has, opens with "The repository changed: this
screen shows it as it was, and is being read again", and the next generation loads behind it.
An answer over the API, MCP or the command line still waits for the generation its caller
saw.

A reading says how it stands on its source line. A current one older than five seconds says
its age ("read 12 s ago"). A stale one says "read 12 s ago, before the repository changed;
reading again", and its line carries `data-catching-up`. `ui.js` reads a screen with such a
line again after 0.4 s, then after twice that, up to 6.4 s, until none is left. A stale value
is never drawn as current.

The first reader of the UI starts one pass over every screen and panel in the background, so
the screen opened next has its sources read before it is asked for. The pass walks `SCREENS`
and `PANELS`; no other list of sources exists.

An answer is not a reading. `execute`, and so the API, MCP and the command line, still run
the handler or return exactly what it would return. Two switches settle a doubt:
- `MAJORDOMUS_CAPABILITY_CACHE=off` makes every reading run its handler and wait for it.
- `MAJORDOMUS_READING_PATIENCE_MS=wait` keeps the readings and makes every read wait for a
  current one. A number sets the 50 ms.

Sources that have no reading yet are asked at once, each on a thread of its own, so a first
paint costs its slowest source rather than the sum of them (`source::ask_all`).

A source too slow for a first paint is a **panel with a route of its own**. The pull-request
queue (`integration.queue`) is `/ui/panels/pull-requests`:
- with `?fragment` the route answers the panel alone, and `ui.js` puts it in place once the
  screen has painted;
- without JavaScript the placeholder is a link to the same route.

While a screen is visible, `ui.js` asks the server for it again every minute, puts the new
rendering in place, and updates the time in the top bar. That time is when the page was
rendered; each source line says when its own reading is older. This is a refresh and is said
as one; nothing claims a live stream. A streamed page and one stream of changed readings
replace it in I2487.

How fast is measured, not written here. `majordomus bench` derives a target from every
screen in `SCREENS` and every panel in `PANELS`, cold and warm:
- `system.http.ui.<route>` reads the page over a pinned context.
- `system.http.live.ui.<route>` reads a screen over the live view that `serve` answers from,
  so it pays the per-request git stamp. `system.http.live.index` measures that stamp alone,
  on `/`.
- A panel is measured only by `--profile full`, because its source is the reason it is a panel.

The targets are listed in [`generated/benchmarks.md`](generated/benchmarks.md).

**A screen over its budget fails the build** (I2489). The benchmark policy,
`.ai/repo/benchmarks/rust/policy.yaml`, declares that a warm screen answers within 100 ms at
p95, for both views. `majordomus bench --check` holds every budget with or without a
baseline and on every host, prints each bounded screen with its value and its bound, and
exits 10 when one is over. The `ui` job of CI runs it before the browser audit:

<!-- majordomus:unrun a benchmark of every screen over a real socket: the ui job of validate runs it, and case 1099 runs it against a fixture -->
```bash
majordomus bench system.http. --transport system --profile ci --no-write --check
```

A regression threshold could not do this: a screen that was always slow never regresses, and
a platform with no baseline compares nothing. The first run of the check found `/ui/command`
at 400-660 ms, where every other screen read 1-21 ms. The first request after the repository
moved is not measured by the bench yet (I2485).

## Relationship graphs

A relationship the source records is drawn as a graph: the dependencies between
milestones (laid out in their direction; a node opens that milestone's inspector), the open pull requests that depend on one another
(a dependency already merged is not drawn; one the queue does not hold is a small node that
leads to the forge), and the mesh's machines, the runtimes they run and the sessions those
hold (laid out by force, sessions as small leaves labelled when selected). The server puts the nodes and edges on the page (`view::graph`). The vendored
Cytoscape, loaded only by a screen that has a graph, draws them:
- in the tones of the design system, read from the stylesheet;
- with a layout that does not animate under reduced motion;
- with nodes that lead where their row is.

The table of the same edges stays beneath the graph, and it is what a reader without
JavaScript, or without the library, reads. Only nodes that take part in an edge are drawn.

## The Observatory

The other screens are rows for a person who came to decide something. The Observatory is the
same facts as a picture, for a display nobody is sitting at (ADR 0158). It is one screen and it
adds no capability:

| Route | What it is |
|---|---|
| `/ui/observatory` | the screen, inside the application's frame |
| `/ui/kiosk` | the same screen, projection and script, filling the window over the frame, for a display left running |
| `/ui/screensaver` | a permanent redirect to `/ui/kiosk`, its better name |
| `/ui/obsrvatory` | a permanent redirect to `/ui/observatory` |

The scenes are one list, `SCENES` in `apps/majordomus-cli/src/ui/observatory.rs`. Each
declares its label, the capabilities it requires, what its marks mean, the screen that shows
the same facts as rows, how long it stays, and its character:
- a **semantic** scene binds marks to records: a machine, an event, a milestone, a release;
- an **ambient** scene is decoration, is labelled as decoration, and stands for no fact.

| Scene | Label | Requires | Its rows |
|---|---|---|---|
| `mesh` | Mesh Cosmos | `mesh.peers` | Mesh |
| `activity` | Event River | `mesh.events` | Mesh |
| `intents` | Intent Nebula | `plan.roadmap` | Roadmap |
| `tokens` | Token Spectrum | `economics.runs` | Evidence |
| `heartbeat` | System Pulse | `health.report` | Dashboard |
| `delivery` | Delivery Tree | `distribution.releases` | CI/CD & Releases |
| `coop` | Cooperation Choreography | `mesh.state` | Mesh |
| `proof` | Proof Lattice | `rules.report` | Governance |
| `claims` | Claim Ledger | `evidence.report` | Evidence |
| `surface` | Surface Atlas | `quality.report` | Capabilities |
| `waves` | Wave Front | `plan.waves` | Roadmap |
| `matrix` | Release Matrix | `distribution.status` | CI/CD & Releases |
| `registry` | Registry Treemap | `capabilities.list` | Capabilities |
| `ladder` | Milestone Ladder | `plan.roadmap` | Roadmap |
| `ambient` | Ambient Intelligence | nothing: it is decoration | Dashboard |

The table is a reading aid; the list in the module is what the page, the script and case 3011
read, and the page prints each scene's own `requires`.

The last seven (`proof` to `ladder`, ADR 0160) read what the repository proves and need no
mesh, so they are drawn on the public site too. They are counted, never estimated: a mark is a
rule, a kind of claim, a measured count, a wave or a check, and where a source reports more
items than a display holds the scene groups them and says how many it drew. They are drawn with
p5 in instance mode onto a surface of their own that the director composes onto its stage, from
`share/ui/observatory-kiosk.js`; p5 is the pinned library `scripts/cockpit-assets` copies into
`share/ui/vendor/`, is not committed, and is loaded only when one of these scenes is shown. A
checkout built without it says "p5 is not built here" on the stage and keeps the same facts in the
table below it.

The page carries its whole projection as one document, `observatory/v1`, in the
`data-observatory` attribute of its section: when it was observed, the revision, every source
asked with the state it answered in (`answered` or `failed`, with the reason and the time
taken), and every scene with its model. The rule of the screen is in that document:

| A scene's sources | The scene |
|---|---|
| answered, with something to draw | `available`, with a model |
| answered with nothing to draw, or could not answer | unavailable: no model, the source's own reason, and the script does not rotate to it |

A scene is never drawn empty, never drawn with stand-in marks and never replaced by a picture
of health. An event of the mesh is projected as its identity (stream and sequence), its kind,
its node, its logical time and its wall time, and never as its body.

The status word says how the page was read:
- `LIVE`: read again within 90 seconds. The script asks for its own page every 60 seconds, and
  between those readings follows the mesh journal from a cursor every 3 seconds through
  `observatory.events`, which answers only the events above the cursor, projected exactly as
  the scene projects them. An event delivered twice is one mark, the river holds at most 200,
  and both readers back off to 120 seconds when the server does not answer;
- `RESYNCED`: for ten seconds after the cursor named a clock the journal never reached (another
  runtime's journal, or one that restarted); the page was read again at once;
- `STALE`: not read again within the bound;
- `REPLAY`: a scene of the mesh drawn from the repository's recording, with the time it was
  recorded;
- `SNAPSHOT`: the public site, which reads nothing again and names its revision.

Without JavaScript the same projection is a table: one row per scene, with its state in words
(`available`, `decorative` or `unavailable`), its meaning or its reason, what it requires and
the link to its rows.

A reader's choice of a scene holds for five minutes, then the rotation resumes. New events of
the journal interrupt the rotation for the river once the scene shown has had 12 seconds, and
at most once every 120 seconds, so a busy journal cannot hold the display; a reader's choice is
never interrupted. A scene whose
renderer throws is quarantined for the rest of the display's life: the rotation skips it, the
others go on, and the scene says `QUARANTINED` with what failed when a reader asks for it.
Nothing on the screen starts work or writes a record.

The display's own state is readable for an inspector or an evidence run: the section's
`observatory` property answers `data()` (the projection it is drawing now), `quarantined()`
and `trace()` (the last 24 choices of the rotation with their reasons). The evidence of the
gates is taken through it by `scripts/lib/ui-observatory.mjs`.

On the public site the process that takes the snapshot runs no mesh. There `mesh`, `activity`
and `coop` draw the recording the repository carries (`.ai/repo/observatory/recording.json`,
read by `observatory.replay`), and the status says `REPLAY` with the time it was recorded and
`not live`. A scene the recording does not carry is unavailable and says why; a recording that
cannot be read is a failed source, never a scene. `tokens` is unavailable while no run is
recorded.

The recording is the answer of `observatory.capture` on a server that runs the mesh, saved as
it came and committed. It names nothing by its identity: machines are `machine 1`, `machine 2`
in the order the mesh listed them, runtimes and sessions are ordinals, and an event keeps its
kind, its logical clock and its time. Its manifest names the capabilities it was read from,
the window of events it holds (first and last time and logical clock) and what was removed
and why; `observatory.replay` refuses a recording with an event that lacks its kind, clock or
time, naming the event. A recording is a moment; it is replaced by capturing again. What is still open is in [What it does not do yet](#what-it-does-not-do-yet).

## Without JavaScript

Every screen is complete HTML, and the menu of a narrow screen opens with a checkbox and its
label. `share/ui/ui.js` is the only script of the UI's own: it pulls in slow panels, refreshes
the screen, draws graphs and reveals the theme toggle. The one inline script is the theme
statement the policy allows by its digest, the same one the Cockpit ships. A served test
asserts that the UI and the Cockpit carry one identical policy.

## Assets and design

The UI's files live in `share/ui/`.

| File | How it is made | Committed |
|---|---|---|
| `ui.css` | compiled from `share/ui/src/ui.css` by `scripts/cockpit-assets`, beside the Cockpit's stylesheet; `--check` fails when either is stale | yes |
| `favicon.svg` | written from the canonical mark by `majordomus generate` | yes |
| `vendor/cytoscape.min.js` | copied from the pinned npm package by `scripts/cockpit-assets` | no |

The `ui` class of the CI model (`.ai/repo/ci/gates.yaml`) selects the asset check, the probe,
the design gate and the UI audit whenever a file under `share/ui/` or
`apps/majordomus-cli/src/ui/` changes.

The stylesheet imports the tokens, the status vocabulary, the primitives and the kit from
`share/design/`, where the site and the Cockpit import the same files. Every colour is a role,
a status or a tone; none is a literal. The markup names what a thing is, never a utility
class, because the stylesheet compiles only what is written in it. The icons are the kit's,
declared once in `site/data/kit/icons.toml` and compiled into the crate by `majordomus
generate` (`apps/majordomus-cli/src/ui/icons.toml`); a test holds the two equal.

## Choices are addresses

What a reader picks on a screen is in its URL: a tab, a search, a row, a dependency, an
issue. Every control is a link or a GET form to the same screen with the choice in its query,
so a view can be bookmarked, shared and reloaded, the back button undoes a choice, and the
screen needs no script to be used. A value a link carries is read as the source's own
vocabulary or dropped: a disposition or lane is a word of the queue (lower case, digits,
underscores), an issue is `I` and four digits, a pull request is a number; anything else is
not echoed. On the PR Drain the choices travel to the deferred panel's route, so the slow
part of the screen answers the same question as the rest of it.

## Adding a screen

1. Add a module under `apps/majordomus-cli/src/ui/` that asks its sources with
   `source::ask_all` and composes `view` components from readings, never a value of its own.
2. Add it to `SCREENS` in `ui/mod.rs` with its icon, and its arm to the dispatcher.
3. A source too slow for a first paint becomes a panel route in `PANELS`, drawn as a
   placeholder with `data-panel-src`.
   Either list makes the route a benchmark target; nothing else is edited.
4. Lead somewhere: a tile with `kpi_to`, a panel with `panel_to`, a row with a link.

## On the public site: a snapshot

The UI is a projection a running process answers, and the public site is files. So the site
carries the UI as a **snapshot** at `/ui/` (ADR 0141): the UI's own answers, read once at the
commit the site publishes, written down.

```sh
majordomus web report ui-snapshot --limit 20
```

It writes into `target/web/ui-snapshot`. `--limit` bounds the pages and `--budget-seconds`
the time spent on the pages the screens link to; the command's own help states the defaults.

The command builds the same UI the server routes to, in-process, and asks it for every screen
and panel, then for the pages those link to: one page per family in turn (every rule's
inspector, every claim's, every issue's) until `--limit` pages are written or
`--budget-seconds` have passed, whichever is first. The screens and panels are written whatever
they cost. It renders nothing itself. What it changes is only what a file cannot do that a route can:

| In the UI | In the snapshot |
|---|---|
| a link to another screen or inspector | a relative link to that page's file; an inspector's query is a directory (`governance/_rule/<id>/`) |
| a link to a page past the bound, a filter of several fields, a form, the API | the snapshot's page about the live UI, carrying the path the link meant |
| a link to a page of the Cockpit: the "technical view" of a source or of an object | that page of the Cockpit's own snapshot at `/cockpit/` when it holds it (ADR 0147), and the page about the live UI when it does not |
| a link to a surface a publication holds (`/rustdoc`, `/openapi.json`) | unchanged |
| a link into the served documentation (`/docs/…`) | the same page of the published site |
| the script reads the screen again every minute and pulls in slow panels | it does neither: the body carries `data-snapshot` |
| "Read at" in the top bar | also a notice on every page: the commit, the time, the executable, and a link to the live UI |

A snapshot knows what the UI could read where it was taken. A Pages runner has no mesh, no
recorded forge observation and no running execution, so those panels read `none`, `unknown`
or `not answered`, never zero. A snapshot is not evidence of the live state of anything: for
that, run `majordomus serve` in a checkout and open `/ui`.

The directory declares itself (`surface.json`) as the surface `ui-snapshot`, mounted at `/ui`,
`published-only`, built from the commit it was read at. A process serves the UI at `/ui` and a
publication holds the snapshot there, and `majordomus web validate` holds both without a
collision. `snapshot.json` beside it lists every page written, every page left to the live
UI, and what ended the reading (`complete`, `limit` or `budget`). `test/cases/1092_the_ui_snapshot_holds_every_screen.sh` and
`test/cases/1093_the_ui_snapshot_links_nothing_absent.sh` hold it to this.

The Cockpit is published the same way, by the same exporter (ADR 0147,
[`COCKPIT.md`](COCKPIT.md#on-the-public-site-a-snapshot)). It is written after the UI's
snapshot: it reads first the Cockpit pages the UI's snapshot wanted to link to, and then leads
those links to the pages it wrote, so a capability a screen cites is a page a reader can
open. Each surface is read once. `test/cases/2982_two_snapshots_link_what_the_other_holds.sh`
holds it.

The published snapshot is held to the trunk (`project.the-published-ui-is-the-trunks`). The
publication model declares that a publication holds it (`deploy.snapshots` in
`.ai/repo/ci/pages.yaml`), `scripts/site-deploy` refuses to publish a site without one read at
`HEAD`, and `scripts/pages verify-ui` asks the public site for it after a deploy and inside the
`pages-live` gate: the commit `surface.json`, the banner and `snapshot.json` name, and every
page the record lists. A snapshot that stopped at its bound passes and the probe states what
it left out; one that is missing, names an older commit or does not say what ended its
reading fails. `docs/GITHUB_PAGES_ARCHITECTURE.md` has the details and
`test/cases/2983_the_published_ui_is_the_trunks.sh` holds them.

The first screen leads only with figures the reader of a snapshot can decide. That process is
no server, so `health.report` says `unknown` for the attached clients and the mesh runtime and
therefore for the whole report. The Dashboard's health figure is the report's `repository`
verdict instead: the worst of the dimensions that describe the repository and not the process
serving it. The Health panel below it still lists every dimension with its own word. Where no
mesh runtime is active the mesh figure reads `Mesh active: no` and counts no machines. The
probe refuses a published first screen with an undecided figure, and
`test/cases/1092_the_ui_snapshot_holds_every_screen.sh` holds the snapshot to the same.

## What it does not do yet

- `majordomus ui` as a command, which ensures the shared server and opens this surface, is
  I2484. Until it lands, open `/ui` on the address `majordomus serve status` prints.
- Nothing on the UI starts work yet: launching a capability, an issue or a milestone through
  the executor and following its run live is the execution control plane of the V4 pack.
  An action on a mesh node is the same, behind the executor (V5); the inspector says so.
- There is no Settings screen: no capability holds a reader's settings.
- CI timings are what the open pull requests' latest runs took, measured from the recorded
  forge observation (`integration.timings`); there is no history of runs yet, so no trend is
  drawn.
- Milestones carry no dates, so the roadmap is a sequence and a graph, never a calendar.
- The Observatory's replay on the public site is as old as the last capture that was
  committed; nothing captures one on a schedule. Between readings of its page it follows the
  journal by asking for the events above its cursor; nothing is pushed to it. Its gates and
  the measurements behind them are in `.ai/repo/benchmarks/observatory/manifest.json`; a
  frame rate there is headless Chromium's on the machine it names, not a panel's.

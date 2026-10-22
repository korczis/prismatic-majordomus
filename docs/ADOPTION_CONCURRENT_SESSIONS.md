# Several sessions on one layer — findings

What a repository met on a day when several AI sessions worked on its Majordomus layer at once, finding
by finding, with how each was observed. `ADOPTION_FIRST_RUN.md` concerned the vendored rules of one
release and `ADOPTION_SECOND_RUN.md` a first day in a foreign repository. This one concerns the day
after adoption: an upgrade of a layer that already exists, two sessions changing the same tracked files,
and a collision the owner told the sessions to settle themselves with an independent review.

## 0. What this was measured against

Adopter: `/Users/korczis/dev/prismatic-platform` (an Elixir/Phoenix umbrella of about ninety applications,
GitLab, a primary checkout that several sessions share and one worktree per piece of work under a
container of its own). Tool: majordomus 0.22.0 (installed, the latest release that day). Date: 2026-10-13.
Sessions: two Claude Code sessions attached to the primary checkout's shared server (board peers `p1` and
`p2`), one of them also working in a worktree on the trunk. Every item was observed, not inferred, unless
it says so. "Reproduce" is run in the adopter.

Each finding was then read again in this repository at master `e506c041a4` (0.23.0). The plan holds one
issue per finding under the milestone `concurrent-sessions-are-served`. Four findings of the same day
repeat ones the second adoption already planned; they are listed in §4 with that issue and have none of
their own.

## 1. The findings and where they stand

| # | Issue | On master `e506c041a4` |
|---|---|---|
| H01 | I2620 — A file the skeleton seeded is compared with the skeleton it came from, and a refresh is a three-way proposal | still present |
| H02 | I2621 — A vendored package is named by its content, not by a label two different packages share | still present |
| H03 | I2622 — A command that writes tracked layer paths reads the claims on them first | still present |
| H04 | I2623 — What a peer is touching is derived from its checkout, and the board shows where that leaves what it announced | still present |
| H05 | I2625 — A second plan for an assessment does not select the advisor whose consultation for it just failed | still present |
| H06 | I2626 — An advisor's status says what was measured, and an account without credit is not a rate limit | still present |
| H07 | I2627 — The distribution carries what it takes to consult an advisor the catalogue names | still present |
| H08 | I2628 — A server that became ready after the start event gave up is not a standing doctor failure | still present |
| H09 | I2629 — A diagnostic class is said once per command, and an example environment file is not a tracked secret | still present |
| H10 | I2633 — doctor and watch say where their time went when they pass their budget | still present |
| H11 | I2630 — mesh verify says when nodes are discovered and trusted and none is linked, and why | still present |
| H12 | I2632 — A record that recovery writes says where it lands, and recovery in a shared checkout says whose it is | still present |
| H13 | I2631 — start names the uncommitted changes of the checkout that the task did not make | still present |
| H16 | I2624 — An overlap is an object, and the peers settle it with an independent review before the owner is asked | still present |

H14 and H15 were observations that turned out to be the adopter's own (a mistyped subcommand, and
subagents sharing one scratch directory); they have no issue.

## 2. What they have in common

Each finding is a place where the tool holds a fact twice, or holds a statement where it could hold a
derivation:

- **Declared where it can be derived.** A peer's scope is what it announced (H04), although its checkout
  says what it touches. A start failure is a line in a file (H08), although the server's lease says it
  became ready. A layer file's origin is nowhere (H01), although the skeleton it was seeded from is a
  revision. A package's identity is a label (H02), although its manifest carries a digest per rule.
- **Computed at one moment, needed at another.** Overlap is computed when a peer speaks and never when a
  command writes (H03). An advisor's standing is computed from presence and from runs of failures, and not
  from the consultation that failed a minute ago for the same question (H05, H06).
- **Joined by a person.** Settling a collision took a board line, a reasoning record, two reviews outside
  any plan, a mesh handover and a patch file, and nothing links one to the next (H16, H07).
- **Said in a way that reads as health.** `cooperation verified` with nobody linked (H11); `available`
  for an executable that does not start (H06); forty warnings before every answer, so that the session
  filtered all of them (H09).

The direction every issue of the milestone takes is the same: one source for each fact, every other
statement of it derived, and each derived statement linked to the records it came from.

## 3. The findings as observed

## H01 After an upgrade the layer says "nothing to add" while its seeded files are a release behind

`majordomus init --extend` printed `nothing to add; .ai/ already carries every file the skeleton seeds`.
`diff -rq <share>/skeleton/ai .ai` listed six seeded files that differ. In
`.ai/repo/knowledge/sources.yaml` the classes `context`, `gap`, `critique`, `intent`, `candidates`,
`domain` and `derive-offload` were absent, so files of those kinds are on disk and in no index. The day
before, the absent class `mesh-declaration` had kept the mesh off with a declaration present. Both
sessions copied skeleton files over the repository's by hand, without knowing of each other.
Reproduce: `majordomus init --extend; diff -rq "$(dirname "$(readlink -f "$(command -v majordomus)")")/../share/skeleton/ai" .ai`.

## H02 Two different rule packages print the same name

`majordomus rules vendor status` printed `vendored: 1 (0.6.0)`, `distribution: 1 (0.6.0)`,
`state: the distribution ships a different package`. Only `rules vendor diff` showed that
`deployment-contract.v1.md` and its manifest digest had changed.
Reproduce: `majordomus rules vendor status` under a tool whose `share/standard` text moved.

## H03 Three commands wrote paths another peer had claimed, and said nothing

Peer `p2` announced `.ai/repo/rules/vendor`, `.ai/repo/providers`, `AGENTS.md` and `CLAUDE.md` at 10:05Z.
Between 10:03Z and 10:25Z peer `p1` ran `rules vendor update`, `update` (twice) and `recover episodes`
in the same checkout. Each wrote; none named the claim. `mesh peers --full`, asked for by a person at
10:50Z, was the first output that showed both.
Reproduce: announce a scope from one MCP session; run `majordomus update` from a shell in that checkout.

## H04 The board compared two announcements while two working trees held the same changes

`p2` announced four paths; `git status` of its worktree showed nine modified layer files and an untracked
`.ai/repo/mesh/`. `p1` had announced one case directory. `majordomus_peers` reported no overlap until `p1`
announced again with the paths it had already written.
Reproduce: `majordomus_peers` beside `git -C <each checkout> status --short`.

## H05 The second plan chose the two advisors that had just failed

`reasoning record {"kind":"plan"}` for a `high` assessment selected `gemini` and `chatgpt` and excluded
`codex`, `ollama` and `lmstudio` (`the budget was spent on advisors earlier in the selection`). The
consultations were recorded as `unavailable` (HTTP 503) and `error` (HTTP 429, no credit). A second plan
for the same assessment returned the same selection and the same exclusions. A consultation of `codex`
is refused (`plan … did not select advisor`), so the conclusion was recorded with
`independent_review_count: 0` and two reviews cited as prior external opinions.
Reproduce: record an assessment, a plan, a failed consultation of each selected advisor, and a plan again.

## H06 "available" for an executable that does not start and an account that cannot pay

`reasoning advisors`: `gemini — available, executable gemini on PATH`; in the repository the executable
is a version-manager shim and exits `No version is set for command gemini`. `chatgpt — available,
credential variable OPENAI_API_KEY set`; the API answered HTTP 429 with an exhausted-quota body.
Reproduce: `gemini --version` in a directory whose `.tool-versions` pins a runtime without the CLI.

## H07 The catalogue names adapters the installed tool does not have

`share/advisors.yaml` of the installed tree names `scripts/lib/advisors/<adapter>.mjs`;
`find <prefix>/versions/0.22.0 -name '*.mjs' -path '*advisors*'` prints nothing. No command runs a
consultation. The session wrote the brief, the three invocations and the normalisation itself.
Reproduce: the `find` above.

## H08 A start that succeeded half a minute later failed `doctor` all day

The start event wrote `the server did not converge: … no server became ready within 20 second(s)`.
`majordomus serve ensure`, run next, answered `ready`. `doctor` reported
`FAIL session .session-context.log — 1 working context(s) could not be written` until the file was
removed by hand, which the message asks for.
Reproduce: start a session in a repository whose index takes longer to build than the wait.

## H09 Forty warnings before every answer

Each of `mesh status`, `mesh peers`, `rules vendor status`, `serve status` and `context` printed the
same `WARN discovered by class 'readme' but out of the repository scope` lines, one per root document,
and four `a secret is tracked (.env.*)` lines for example and template files. Every command of the day
was piped through a filter.
Reproduce: `majordomus mesh status 2>&1 | grep -c WARN` in a repository with many root documents.

## H10 `doctor` and `watch` take two to three times their budget and say only the total

`doctor`: 108 197 ms, 92 596 ms and 109 600 ms against 40 000 ms; `watch`: 80 058 ms. One WARN each.
Reproduce: `majordomus doctor` in the adopter.

## H11 `cooperation verified` with ten nodes present and nobody linked

`mesh status`: `nodes 10 (9 trusted, 10 present)`. `mesh verify`: `links 0 connected of 0 linked
peer(s); 0 seed(s) declared`, `verdict cooperation verified`.
Reproduce: both commands on a machine where several servers of several repositories run.

## H12 Three recovered records nobody owns

`recover episodes` wrote three files under `.ai/repo/sessions/` in the shared checkout; `doctor` then
warned `3 record(s) exist here and are committed nowhere`. The policy of the adopter commits no records.
Reproduce: `majordomus recover episodes` with stranded episodes in a checkout nobody commits from.

## H13 A task that could never finish was started without a word

A task scoped to one ignored directory ran in the shared checkout; `check` listed 68 scope findings, all
files of other workers; `finish` is refused and the push gate with it. The adopter's own rule for this
ends with "Nothing warns at `start`".
Reproduce: `majordomus start "x" --scope docs` in a checkout with a foreign uncommitted change elsewhere.

## H16 "Agree among yourselves" has no place to happen

Asked which session should land the refresh, the owner answered that the sessions should agree, using an
independent AI or a pool of them. What that took, in order: an announcement naming the other peer; a
neutral brief written by hand; `codex exec` and a second session with read access, both of which chose
the same option and corrected it the same way (restore file by file, since a wholesale restore would
have deleted an untracked directory); a reasoning assessment, two plans, two failed consultations, a
conclusion and a validation; a patch file; a second announcement with the outcome. The other peer has no
signal that any of it happened unless it reads the board.
Reproduce: none needed; the records are `.ai/local/state/reasoning/` of the adopter's task of that day.

## 4. Seen again here, planned by the second adoption

| Observed in prismatic-platform | Issue |
|---|---|
| `doctor` printed four FAIL lines and summarised `2 failure(s)`, then `1 failure(s)` with four | I2586 |
| `FAIL reasoning` for credentials named in `.github/workflows/archive/ci.yml` and for a sentence of an old ADR | I2587 |
| `FAIL bootstrap AGENTS.md — carries a rule corpus of its own` with the projection in `mode: region` | I2588 |
| The generated bootstrap's worktree layout had to be overridden in the provider templates | I2590 |

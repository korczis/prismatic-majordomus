# Doctrine

A rule is not enforced because it is written down.

A rule is enforced when it is declared, implemented, wired, executed, failure-propagating,
tested, CI-blocking, and linked to a claim. Anything less is documentation. This layer
exists to know the difference, and to say so in one command.

## The chain

```
principle          why the rule exists at all
  ↓
doctrine           what must be true          share/standard/majordomus/
  ↓
validator          whether it is true          mj_validate_<name> in lib/
  ↓
enforcement point  when the validator runs     the commands in enforced_by
  ↓
propagation        what a violation costs      the command's exit code
  ↓
test               proof of the behaviour      test/cases/
  ↓
CI                 proof it blocks integration .github/workflows/validate.yml
  ↓
claim              what may be promised        docs/CLAIMS.yaml
```

`majordomus doctor` walks that chain for every doctrine and reports the first link that
breaks. It reads the source, not the registry's own description of itself, so a registry
that lies about what it enforces fails here rather than in production.

## The registry

The registry is derived, never written: it is every rule of the repository's effective
set whose `x-majordomus` block names a validator, in resolved dependency order. The
effective set is the package the tool ships (`share/standard/majordomus/`), vendored into
the repository under `.ai/repo/rules/vendor/majordomus/`, plus the repository's own rules
under `.ai/repo/rules/project/`. A repository does not invent Majordomus doctrines,
because a doctrine is a statement about how the tool behaves and every user of the tool
is entitled to the same behaviour; it may add rules of its own, and it may enforce them
with a validator of its own, in the same format.

Each doctrine, read from its rule object, carries:

| field | meaning |
|---|---|
| `id` | the rule id, `majordomus.<name>` for the baseline; used by `--rule`, by the site, and by refusal messages |
| `title`, `description`, `statement` | what it means, in a sentence each |
| `class` | `blocking` or `advisory` — see below |
| `depends_on` | the rules it rests on, the principle it serves among them |
| `x-majordomus.validator` | the suffix of the `mj_validate_<validator>` function that decides it |
| `x-majordomus.category` | the finding category a violation is reported under |
| `x-majordomus.enforced_by` | the commands that dispatch it |
| `x-majordomus.exit_code` | the exit code a violation produces (from the existing contract; no doctrine invents one) |
| `x-majordomus.policy_key` | for finish doctrines, the name a repository uses in `verification.finish_requires` |
| `x-majordomus.claims` | the claim ids in `docs/CLAIMS.yaml` this doctrine backs |
| `x-majordomus.tests` | the cases that prove it |
## Rule objects

The rules are portable **rule objects**: one Markdown file per rule, YAML front matter on
the machine side, prose on the human side, under the repository's rules section. They are
readable without the tool, and the tool reads nothing else.

```text
.ai/repo/rules/
├── README.md                  the format, for whoever writes a project rule
├── vendor/majordomus/         the pinned Majordomus baseline; do not edit, upgrade explicitly
│   ├── manifest.yaml          every rule file, its identity and its content hash
│   └── rules/*.md
└── project/*.md               rules this repository wrote
```

### Front matter

Identity is `id` and `version`, never the file name. The generic fields are the ones any
reader of the format understands; the `x-majordomus` block is the tool's binding and is
present only on a rule the tool enforces.

```yaml
---
id: majordomus.scope-integrity      # namespaced by origin; project rules use another prefix
version: 1                          # an exact integer
kind: rule
title: Scope integrity
description: One sentence.
statement: The normative sentence a worker follows.
status: active                      # active | deprecated
class: blocking                     # blocking | advisory — the same two classes, no third
depends_on: [majordomus.state-consistency@1]   # exact id@version references, or []
tags: [scope, verification]
x-majordomus:
  validator: scope                  # mj_validate_<validator>
  category: scope                   # the finding category
  enforced_by: [check, finish, watch]
  policy_key: scope_respected       # finish doctrines only
  exit_code: 10
  claims: [scope-enforcement]       # ids in docs/CLAIMS.yaml
  tests: [test/cases/04_start_check.sh]
---
```

The allowed keys are listed in `share/allow/rule.txt`; a key outside that list is an
error, not a silent extra. A generic reader may ignore `x-majordomus` and still understand
the rule. A rule without the block is normative for whoever reads it and enforced by
nobody, and `majordomus rules list` says `not machine-enforced` for it rather than
implying otherwise. The class still says what a violation means.

A rule that is `class: blocking` *and* has no block is therefore a promise this tool cannot
keep by itself, and most of this repository's blocking rules are in that position — measure
it with `majordomus doctrine status`, which prints the registry's size beside the effective
set's and names the count on its own line. The count is large, it predates the report, and
the numbers must be read carefully: most of those rules *are* enforced, by a gate under
`scripts/ci/` or by a case under `test/cases/` named in their `# Verification` section. What
the doctrine registry cannot see is not the same as what nothing enforces, and a surface
that reports one while the reader hears the other is the defect
`project.a-verdict-states-its-subject` exists to stop.

`scripts/ci/rule-proof-check` is what decides the difference, and it is the only reader of
that population: a blocking rule must name, in its `x-majordomus` block, a validator the
dispatcher calls, a test that proves it, or a `reviewed_because:` saying why nothing
executable can express it. The rules that name none are ratcheted in
`.ai/repo/rule-proof-baseline.txt` so the set can shrink and cannot grow. It is answered by
wiring the enforcement, never by changing the rule's class — moving a promise is not keeping
it.

The other end of the same idea is a claim, not a rule, and it has its own gate:
`scripts/ci/claim-proof-check` refuses a claim marked `status: guaranteed` in
`docs/CLAIMS.yaml` whose named test does not contain the claim's id, so the link reads from
both ends rather than only from the claim's. The two gates are separate readers of separate
populations and each says so on every run. They were one script, and the rule half of it
accepted any `x-majordomus` block as proof — including a `reviewed_because:`, which declares
that a *person* enforces the rule. It announced those exemptions as promises that had gained
a proof. A second inventory of a property another gate owns does not stay a harmless
duplicate; it drifts, and then the two verdicts disagree about the thing this repository
exists to assert.

### Resolution

`majordomus rules list` resolves the effective set as a dependency graph, deterministically:
the vendored package in its manifest order, then project rules in file-name order, ordered
so that every dependency precedes the rule that depends on it. Each of these stops the
resolution with exit 10 and the reason, and nothing is applied partially:

- a dependency no rule provides, or one provided only by a deprecated rule,
- a dependency cycle,
- one `id@version` claimed by two files,
- a project rule whose id is in the `majordomus.` namespace,
- front matter that is absent, does not parse, lacks a required field, or carries an
  unknown key,
- an `x-majordomus` block with no validator, no enforcing command, or no test.

### Vendoring

The baseline under `vendor/majordomus/` is a copy of the package the executable ships in
`share/standard/majordomus/`, written by `init` and afterwards only by
`majordomus rules vendor update`. The shipped package's own manifest is written by its
maintainer with `scripts/rules-package write` and checked by `scripts/rules-package check`
in the test suite; no command that runs in a managed repository writes into the
distribution. The repository's copy is authoritative for that
repository: a newer executable reports a newer baseline through `rules vendor status` and
`rules vendor diff`, and never applies it. The manifest names every rule file with its
hash, so a hand edit under `vendor/` is detected by `rules vendor status` and refused by
`rules vendor update` until `--force`. The update is atomic and never touches
`rules/project/`.

### Composition: additive, no override

The effective set is every active vendored rule plus every active project rule. A project
rule may add a constraint. There is no override mechanism: nothing disables or weakens a
vendored rule, and a project rule may not reuse a vendored identity or its namespace. A
repository that needs a vendored rule gone changes the baseline explicitly, in the open,
with `rules vendor update`, or does not use the tool.

### What is authoritative

The rule objects, and nothing else. The dispatcher in `lib/doctrine.sh` loads the
repository's effective set through `lib/rules.sh` and treats every rule with an
`x-majordomus` block as a doctrine; `doctor`, `doctrine list`, `check --rule` and the site
read the same set. A set that does not resolve — a missing dependency, a cycle, two files
claiming one identity, a malformed file — stops the command that needed it with exit 10
and the reason, and nothing is enforced partially. The wiring chain — declared, validator
exists, the commands it names dispatch it, a blocking failure propagates, every test it
names exists, CI runs the suite — is recreated against the objects, and the reverse check
(every `mj_validate_*` function is declared by an effective rule) is kept. A hand-edited
vendored file is a `doctor` failure through `majordomus.rule-package-integrity`.
## Two classes, and no third

- **blocking** — a violation stops the command with a non-zero exit.
- **advisory** — a violation is reported and the command still succeeds.

The class is not a label on a diagram. `mj_doctrine_fail` reads it and routes the finding
accordingly, so changing `advisory` to `blocking` in the registry changes whether
`majordomus check` exits 0 — and `test/cases/17_doctrine_enforcement.sh` flips exactly
that value and asserts the exit code moves. Which doctrines are advisory today is derived, not written here: `majordomus doctrine list`
prints the class of each. Everything that is not advisory blocks.

`watch` asks the same doctrines a different question — not *is this wrong* but *has this
moved* — so under `watch` a violation is reported as `DRIFT` and the exit code is 11,
advisory doctrines included: watch never blocks work, so the class has nothing to decide
there. The rule, the validator and the message are the same.

Not every doctrine is watchable, and the registry says which by omitting `watch` from
`enforced_by`. An unresolved question is the clearest case: it is a recorded state, written
down on purpose, and the opposite of drift. A questions file that no longer *parses* is
drift, and that is a different doctrine.

## Three enforcement modes

A rule says how it is enforced in its own `x-majordomus` block, and the shape of that block
*is* the mode — there is no word to declare and no word to get wrong:

| mode | the block names | who runs it | what the repository can show |
|---|---|---|---|
| **dispatched** | `validator`, with `category`, `exit_code`, `enforced_by` and at least one test | `mj_doctrine_dispatch`, at the commands `enforced_by` names | the validator ran, and the cases prove it decides correctly |
| **gated** | `tests`, and no validator | a CI gate, or the behavioural suite | the cases exist and a gate runs them |
| **reviewed** | `reviewed_because`, and neither of the above | a person | nothing executable — and the reason why not |

The third exists because some rules genuinely have no machine expression. *The provenance
of what was written* is one; *whether a mechanism earns its cost* is another. Before it,
those sat in a debt list called `rule-proof-baseline.txt` alongside rules that merely had
not been got round to, and the two were indistinguishable — which is the worse failure,
because the list could only be read as "work still to do" and half of it never would be.

`reviewed_because` is the whole declaration. There is no boolean beside it, deliberately: a
flag can be set, and a reason cannot be set without writing one. An exemption is therefore
always a sentence somebody had to defend, and a reviewer reads the sentence rather than
counting entries in a list.

Two properties keep it from becoming a way out:

- **an executable proof always wins.** A rule that names a test is gated whether or not the
  reason is still in its front matter, so a rule acquiring a case stops being
  review-enforced without anyone remembering to delete anything.
- **it is counted apart, everywhere.** `rule-proof-check` reports every one as `NOTE` on
  every run and prints the count beside the measured total; the typed report counts them in
  `review_only`, never in `passing` and never in `named_proof`. This number going up is
  governance getting weaker, and no summary folds it into a total that would hide that.

## The proof graph

`majordomus doctrine` answers *does the repository satisfy this rule right now*. That is a
question about the tree. There is a second question — *is this rule in a state where it
could be satisfied at all* — which is about the rule, and until the proof graph nothing
asked it. `crate::rules` is the one typed reading that does, and `majordomus-cli rules`
projects it to every surface.

For each rule it joins the declaration to the tree and to the ledger of recorded runs
([`EVIDENCE.md`](EVIDENCE.md)), and reports one state:

| state | what it means |
|---|---|
| `proven` | every named artifact is in the tree, and every named case has a passing run that nothing has changed since |
| `inputs unchanged` | the same, except at least one run is only un-invalidated — nothing it names has changed, which is the absence of a known invalidation rather than proof against this commit |
| `stale` | a passing run exists, and a file it is about has changed since |
| `gated` | an executable check refuses violations and a CI gate runs it; this repository records no verdict for a gate, so what can be shown is the mechanism and not the result |
| `failing` | the most recent recorded run did not pass |
| `not run` | a runner owns the case and no run of it was ever recorded |
| `reviewed` | the rule declares, with its reason, that nothing executable can express it |
| `unrunnable` | the rule names a path no runner drives, so nothing can ever record it |
| `dangling` | the rule names a path that is not in the tree — it reads as proven, and is not |
| `unproven` | the rule names neither a validator nor a test nor a reason |

A rule's state is the **weakest of its parts**: one dangling case makes the rule dangling
however many of its other cases pass. The two states nothing else in this repository can
express are the last two, and they are the reason the graph exists. `dangling` is the
one-way relation this repository keeps rediscovering — a proof verified by name, with the
converse unowned, so a case renamed in January leaves a rule reading as enforced in June.
`unproven` is the half no other check knows to look for: nothing else knows that a blocking
rule is *supposed* to name anything at all.

A **finding** is a rule whose declared class the state does not support. `dangling` is a
finding at every class, because it reads as proof and is not. For a blocking rule,
`unproven`, `failing` and `unrunnable` are findings too: each makes the claim "a gate
refuses work that violates this" false. `gated`, `reviewed` and `not run` are not findings —
they are weaker states, said out loud and counted, which is a different thing from a defect.

Both directions are readable. `rules show` answers what proves a rule; `rules proves`
answers what a test proves, and names the rules that would be left with nothing at all if
it were deleted. That is the question that could not be asked while the relation ran one
way only, and it is the one to ask before renaming a case.

## The dispatcher

No command names a validator. Each one calls `mj_doctrine_dispatch <command>`, which walks
the registry and runs every doctrine whose `enforced_by` contains that command. A doctrine
added to the registry is enforced from that moment without any command changing.

Three things are configuration errors rather than rule results, and each says so in its own
words:

- a doctrine whose validator function does not exist,
- a doctrine whose class is neither `blocking` nor `advisory`,
- a validator that exits non-zero (a validator reports violations through
  `mj_doctrine_fail`, so a non-zero return means the validator itself broke).

All three fail closed. None of them is ever reported as a clean run.

## What `doctor` verifies

For every doctrine:

1. the validator function is defined somewhere in `lib/`;
2. every command in `enforced_by` exists and calls `mj_doctrine_dispatch`;
3. a blocking doctrine's commands can turn a failing finding into a non-zero exit;
4. the test file it names exists;
5. every claim it names is in `docs/CLAIMS.yaml`.

And in the other direction: every `mj_validate_*` function in `lib/` is declared by some
doctrine. An orphan validator is enforcement running under no rule, which is how a check
quietly stops being governed, so it fails.

And for the pipeline: `validate.yml` runs `test/run.sh`, does not swallow its exit code,
and `test/run.sh` globs `test/cases/` rather than listing cases by name — a runner that
lists cases is a runner a new case can be missing from.

`test/cases/18_doctrine_wiring.sh` breaks each of those nine links in a throwaway copy of
the tool and fails unless `doctor` goes red. A verifier that survives broken wiring proves
nothing.

## Finish is a doctrine bundle

`majordomus finish` does not carry its own list of contract lines. The contract is the set
of doctrines whose `enforced_by` names `finish`, and a repository's
`verification.finish_requires` selects which of them it applies — by `policy_key`, so the
two lists cannot drift apart silently. A requirement in the policy that no doctrine defines
is reported and refuses; it is not silently ignored.

A refusal names the doctrines responsible:

```
finish: refused, 1 unmet
blocking doctrines:
- majordomus.blocker-resolution
```

One line of the contract is not about the repository at all. `publication_current` asks
whether what the public is being served is still a projection of the trunk — a question no
gate in the CI model can be selected for, because no change to any file can answer it. The
doctrine `majordomus.publication-currency` runs the gates the CI model marks `at-finish`,
live, at the moment of the claim, and refuses `completed` while one of them says the
publication is owed. It is run and never recorded: evidence whose subject can change while
the tree stands still has no hash to expire against.

## Reading it from the command line

```bash
majordomus doctrine status        # derived counts: declared, blocking, advisory, unwired
majordomus doctrine list          # id, class, validator, the commands that enforce it
majordomus doctrine show <id>     # the full record, including claims and test
majordomus check --rule <id>      # run one doctrine
majordomus doctor                 # verify the whole chain
```

Counts are derived on every invocation. None is written down anywhere, here included.

## What was rejected

The design input for this layer was a large platform's enforcement system. Most of it does
not belong in a portable shell tool, and the parts left out are as deliberate as the parts
kept.

- **A severity ladder.** Blocking or advisory. A rule that is neither is a note in a
  document, not a doctrine.
- **Baselines and ratchets.** A ratchet lets legacy violations remain while blocking new
  ones. It is the right answer for a repository with accumulated debt and the wrong answer
  for a tool that has none: a baseline file is a place for a number to be quietly raised.
  Majordomus enforces absolutely. If a repository adopting it cannot satisfy a doctrine,
  the honest move is to say so, not to encode the gap as a permitted ceiling.
- **An override mechanism.** No `MAJORDOMUS_BYPASS`, no exemption trailer, no amnesty
  ledger. An override that exists gets used, and then the audit trail of overrides becomes
  the work. `finish` already has typed outcomes — `partial`, `blocked`, `failed`,
  `no_match` — which say *what actually happened* instead of pretending the contract was
  met.
- **An audit ledger of enforcement events.** The existing `state/ledger.jsonl` already
  records the finish contract line by line, under a retention cap. A second ledger of
  violations would grow without answering a question the first cannot.
- **A policy expression language.** `enforced_by` is a list of command names compared
  exactly. Anything that needs a grammar to express is a validator, not a rule.
- **Per-doctrine exit codes.** Findings map onto the existing contract — 10 contract unmet,
  11 drift, 12 missing artifact, 13 internal, 15 refused. A doctrine does not get to invent
  a number.
- **Hooks everywhere.** Enforcement runs through lifecycle commands. A repository chooses
  to wire those commands into git hooks and `doctor` verifies the wiring it declared; the
  tool does not install hooks behind a repository's back.

## The failure this exists to prevent

```
rule documented
+ script exists
+ test exists
+ nothing invokes it
= fake enforcement
```

Every one of those four is present in the failure state, which is why counting them is not
a check. `doctor` traces the invocation instead.

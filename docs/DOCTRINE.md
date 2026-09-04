# Doctrine

A rule is not enforced because it is written down.

A rule is enforced when it is declared, implemented, wired, executed, failure-propagating,
tested, CI-blocking, and linked to a claim. Anything less is documentation. This layer
exists to know the difference, and to say so in one command.

## The chain

```
principle          why the rule exists at all
  ↓
doctrine           what must be true          share/doctrines.yaml
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

`share/doctrines.yaml` ships with the tool. A repository configures Majordomus in
`.majordomus/policy.yaml`; it does not invent doctrines, because a doctrine is a
statement about how the tool behaves and every user of the tool is entitled to the
same behaviour.

Each entry carries:

| field | meaning |
|---|---|
| `id` | stable name, used by `--rule`, by the site, and by refusal messages |
| `title`, `summary` | what it means in a sentence |
| `class` | `blocking` or `advisory` — see below |
| `principle` | which of the ten operating principles it serves |
| `validator` | the suffix of the `mj_validate_<validator>` function that decides it |
| `category` | the finding category a violation is reported under |
| `enforced_by` | the commands that dispatch it |
| `exit_code` | the exit code a violation produces (from the existing contract; no doctrine invents one) |
| `policy_key` | for finish doctrines, the name a repository uses in `verification.finish_requires` |
| `claims` | the claim ids in `docs/CLAIMS.yaml` this doctrine backs |
| `test` | the case that proves it |

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
- blocker_resolution
```

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

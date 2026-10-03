# Target architecture

## Repository layout

```text
repository/
├── README.md
├── AGENTS.md
├── CLAUDE.md / GEMINI.md / ...     # optional thin provider adapters
│
├── .ai/
│   ├── README.md                    # protocol and discovery entrypoint
│   ├── manifest.yaml                # format version + section registry
│   │
│   ├── repo/                        # TRACKED
│   │   ├── README.md
│   │   ├── policy.yaml
│   │   ├── profiles/
│   │   ├── rules/
│   │   │   ├── README.md
│   │   │   ├── project/
│   │   │   └── vendor/
│   │   │       └── majordomus/
│   │   │           ├── manifest.yaml
│   │   │           └── rules/
│   │   ├── prompts/
│   │   ├── skills/
│   │   ├── workflows/
│   │   ├── knowledge/
│   │   │   ├── README.md
│   │   │   ├── sources.yaml
│   │   │   └── curated/
│   │   ├── adrs/
│   │   ├── project/
│   │   │   ├── project.yaml
│   │   │   ├── milestones/
│   │   │   └── issues/
│   │   └── templates/               # only repo-custom templates, if any
│   │
│   └── local/                        # WHOLE SUBTREE GITIGNORED
│       ├── prompts/
│       ├── cache/
│       ├── session-contexts/
│       └── state/
│           ├── current.yaml
│           ├── ledger.jsonl
│           ├── checkpoints/
│           ├── handovers/
│           ├── sessions/
│           ├── archive/
│           ├── completed/
│           ├── decisions.md
│           └── open-questions.md
│
├── docs/
├── source...
│
└── .majordomus/                     # OPTIONAL INSTALL LOCATION ONLY
    ├── bin/
    ├── lib/
    ├── share/
    └── ...
```

The last subtree is not created by `majordomus init` and is not part of the
repository data protocol.

## Tool distribution layout

Do not relocate product implementation into `.ai/`.

The Majordomus source/distribution may retain a conventional layout similar to:

```text
bin/
lib/
share/
├── ai-skeleton/
├── standard/
│   └── rules/
├── schemas/
├── migrations/
├── provider-adapters/
└── ...
test/
docs/
```

Exact distribution subdirectory names may remain close to current names if that
minimizes churn. Semantic ownership matters more than aesthetic renaming.

## Ownership test

For every file ask:

### A — Would this still mean something if Majordomus were replaced by another compatible tool?

If yes, it is a strong `.ai/` candidate.

### B — Is it specific to this repository?

If yes and tracked, `.ai/repo/`.
If yes and local/transient, `.ai/local/`.

### C — Is it implementation knowledge required to implement Majordomus itself?

If yes, distribution (`bin/lib/share/test/docs`), not `.ai/`.

### D — Is it a derived projection?

Keep it outside canonical sources. Prefer deterministic regeneration. If
persistence is necessary for a guarantee such as hand-edit protection, document
why and store the minimum evidence required.

## Fresh-clone invariant

A fresh clone of the target repository, before Majordomus is installed, must
contain enough tracked `.ai/` content and bootstrap files for a human or capable
LLM to understand the repository AI contract.

Local task/session history is intentionally absent on a fresh clone.

## Read-only tool invariant

Running Majordomus from:

```text
/usr/bin/majordomus
~/tools/majordomus/bin/majordomus
./.majordomus/bin/majordomus
```

must use the same repository `.ai/` data and produce the same behavior for the
same tool version.

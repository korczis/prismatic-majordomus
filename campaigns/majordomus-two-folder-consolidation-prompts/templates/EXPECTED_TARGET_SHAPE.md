# Expected target-project shape

This is conceptual, not a hard-coded template.

```text
target-project/
├── .ai/                    # portable semantic repository intelligence
├── .majordomus/            # Majordomus mechanics/runtime/ownership
├── AGENTS.md               # optional tiny managed bridge / existing user file
├── .envrc                  # optional tiny managed bridge / existing user file
└── ...                     # user's actual project
```

The source Majordomus repository may legitimately contain additional developer tooling. The invariant applies to **Majordomus-owned install footprint in target repositories**, not every file in the Majordomus source tree.

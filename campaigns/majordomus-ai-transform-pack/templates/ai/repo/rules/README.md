# Repository rules

Rules are portable Markdown documents with machine-readable YAML frontmatter.

## Locations

```text
project/                  repository-owned rules
vendor/majordomus/        pinned Majordomus baseline, do not edit directly
```

## Loading

Rule identity comes from frontmatter, not filename.

Resolve `depends_on` deterministically.

Missing dependencies, cycles and ambiguous duplicate identities are errors.

For format details see the repository's `.ai/README.md` and the vendored
Majordomus rule manifest.

## v1 composition

The v1 effective set is additive:

```text
vendored baseline + project rules
```

There is no implicit override mechanism.

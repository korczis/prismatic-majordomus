# The capability registry and its projections: what exists, where it came from, how it is
# exposed, and the committed files derived from it.

# Every capability with its projections (Rust registry). Extra arguments pass through (--kind, --exposure, --format).
[group('registry')]
capabilities *args: build
    "{{rust_bin}}" capabilities list {{args}}

# One capability by id: schemas, provenance, every projection.
[group('registry')]
describe id *args: build
    "{{rust_bin}}" capabilities describe "{{id}}" {{args}}

# The registry's invariants and every projection; exit 10 with every violation named.
[group('registry')]
validate: build
    "{{rust_bin}}" capabilities validate

# Regenerate every projection: docs/generated/, share/allow/, AGENTS.md/CLAUDE.md/... from the policy, site/data/registry/ from the registry.
[group('registry')]
generate: build
    "{{rust_bin}}" generate

# Exit 10 naming every stale generated projection; writes nothing.
[group('registry')]
generate-check: build
    "{{rust_bin}}" generate --check

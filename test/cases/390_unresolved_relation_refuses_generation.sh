# majordomus-covers: none
# A reference to nothing stops the graph being generated (ADR 0020).
#
# `covers: none` because what this case drives is the Rust executable's `generate`, which
# is not one of the shell tool's public commands the coverage check accounts for.
#
# The composed graph resolves what the layer names: a rule that depends on a rule, a
# decision that puts a thing in force, a use case that runs a command. A reference that
# names a kind this repository holds and resolves to nothing draws no edge — the builder
# drops an edge whose ends are not both present — so the defect never shows in the
# artifact. The file looks whole and is quietly short of one edge, and every regeneration
# reproduces it.
#
# `graph::unresolved_relations` has always decided this; its own contract says a consumer
# that generates an artifact refuses while it is not empty, and until now no consumer did.
# This case plants one dangling reference in a repository of its own and requires the
# generation to refuse, naming the file, the key, what was named and the correction, and
# then requires the refusal to go away when the reference does.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm fixture

# the same tree generates, so the refusal below is about the reference and nothing else
expect_exit 0 "$RB" generate || {
  printf '    a freshly initialised repository does not generate\n    output: %s\n' "$LAST_OUT"
  exit 1
}
expect_file docs/generated/graph.json

# A decision that names a rule this repository does not hold. Tracked, because the adr
# class is discovered through the version-control index: an untracked file is not an
# object, and a fixture that only wrote it would prove nothing and look like it had.
mkdir -p .ai/repo/adrs
cat > .ai/repo/adrs/0001-a-decision-that-names-nothing.md <<'ADR'
---
schema: adr/v1
id: adr-0001
kind: adr
title: A decision that names nothing
status: proposed
date: 2026-09-16
related:
  - rule:no-such-rule-lives-here
---

# 1. A decision that names nothing

## Context

A fixture for the one thing this case measures.

## Decision

Name a rule this repository does not hold.

## Consequences

The composed graph would carry a reference to nothing, so the generation refuses.
ADR
git add -A >/dev/null && git commit -qm "a dangling reference"

expect_exit 10 "$RB" generate || exit 1
# the refusal names the file, the key, the reference and what to do about it: a message
# that said only "unresolved reference" would send a reader looking for the wrong thing
for want in "0001-a-decision-that-names-nothing.md" "related" "no-such-rule-lives-here"; do
  case "$LAST_OUT" in
    *"$want"*) ;;
    *) printf '    the refusal does not name %s\n    output: %s\n' "$want" "$LAST_OUT"; exit 1 ;;
  esac
done

# and the refusal goes when the reference does
git rm -q .ai/repo/adrs/0001-a-decision-that-names-nothing.md
expect_exit 0 "$RB" generate || {
  printf '    a repository with no dangling reference still refuses to generate\n    output: %s\n' "$LAST_OUT"
  exit 1
}

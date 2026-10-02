# majordomus-covers: knowledge usecase
# majordomus-timeout: 300
# A scenario's recorded output is the same whether or not the machine recording it built an
# executable (#713).
#
# `knowledge nodes` checks every file against the schema of its kind by asking the
# executable's index, and it WARNed when there was none to ask. The suite job and a laptop
# have one, CI's site job does not, so the catalogue each derived differed by that line and
# `generate-site-data --check` called the committed one stale. The line used to be masked in
# the recorder's normalisation, which made the artifacts agree without making the recording
# reproducible. Now the scenario environment decides the question — the schema check is not
# asked for — and the reader says exactly that, on every machine; nothing is masked.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?

"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p docs
printf 'version: 1\nstatuses:\n  - id: guaranteed\n    meaning: Deterministic.\nclaims:\n  - id: exit-code-contract\n    claim: Exit codes mean one thing\n    source: docs/CLI.md\n    implementation: bin/majordomus\n    test: test/cases/00.sh\n    status: guaranteed\n    responsibility: none\n' > docs/CLAIMS.yaml
cat > .ai/repo/use-cases/list-the-nodes.md <<'MD'
---
id: list-the-nodes
kind: use-case
title: 'List the knowledge nodes'
summary: 'List the nodes of an installed repository.'
category: knowledge
status: active
target: guaranteed
actors: [operator]
difficulty: basic
commands: [knowledge]
doctrines: []
claims: [exit-code-contract]
responsibilities: []
applications: []
---

# Situation

What does the layer hold?

# Scenario

```yaml
setup: installed
given:
  - 'installed'
steps:
  - id: nodes
    run: ['knowledge', 'nodes']
    note: 'the nodes, and whether their files were checked against their schemas'
    expect:
      exit: 0
      stdout_contains: ['^knowledge nodes: [0-9]+ in scope all']
then:
  - 'the record reads the same on every machine'
```

# Outcome

The nodes.
MD
# what the claim names exists, so the layer is clean and the only finding is the schema's
mkdir -p bin test/cases && : > bin/majordomus && : > docs/CLI.md && : > test/cases/00.sh
git add -A >/dev/null && git commit -qm "one use case"
expect_exit 0 "$MJ" usecase validate

# --- the reader's own contract: told not to ask, it says so, and it never reads as a pass
MAJORDOMUS_BIN="$RB" MAJORDOMUS_KNOWLEDGE_SCHEMA=unasked "$MJ" knowledge nodes > with.txt 2>&1 \
  || { echo "    knowledge nodes failed with the check unasked"; cat with.txt; exit 1; }
MAJORDOMUS_BIN="$PWD/no-such-executable" MAJORDOMUS_KNOWLEDGE_SCHEMA=unasked "$MJ" knowledge nodes \
  > without.txt 2>&1 || { echo "    knowledge nodes failed with no executable"; cat without.txt; exit 1; }
cmp -s with.txt without.txt || {
  echo "    with the check unasked, the output still depends on whether an executable exists"
  diff with.txt without.txt | head -20; exit 1; }
grep -q '^INFO knowledge .* the schema check was not asked for' with.txt \
  || { echo "    the reader does not say the check was not asked for"; cat with.txt; exit 1; }
grep -q 'index could not be asked' with.txt && { echo "    an unasked check reads as a machine's shortfall"; exit 1; }
MAJORDOMUS_KNOWLEDGE_SCHEMA=unasked "$MJ" knowledge nodes --json > unasked.json 2>/dev/null || true
[ "$(jq -r '[.findings[] | select(.code == "schema_unasked" and .level == "INFO")] | length' unasked.json)" = 1 ] \
  || { echo "    --json does not carry the unasked check"; jq '.findings' unasked.json; exit 1; }
# and asked (the default), the index is consulted: with an executable there is no such line
MAJORDOMUS_BIN="$RB" "$MJ" knowledge nodes > asked.txt 2>&1 || { cat asked.txt; exit 1; }
grep -q 'schema check was not asked for\|index could not be asked' asked.txt \
  && { echo "    the default no longer asks the index"; cat asked.txt; exit 1; }

# --- the recorder: the same scenario, recorded with and without an executable, is the same
MAJORDOMUS_BIN="$RB" "$MJ" usecase run --out "$T/ev-with" list-the-nodes > run1.txt 2>&1 \
  || { echo "    the scenario failed with an executable"; cat run1.txt; exit 1; }
MAJORDOMUS_BIN="$PWD/no-such-executable" "$MJ" usecase run --out "$T/ev-without" list-the-nodes \
  > run2.txt 2>&1 || { echo "    the scenario failed with no executable"; cat run2.txt; exit 1; }
out_of() { jq -r '.steps[] | select(.id == "nodes") | .output' "$1"; }
[ "$(out_of "$T/ev-with/list-the-nodes.json")" = "$(out_of "$T/ev-without/list-the-nodes.json")" ] || {
  echo "    the recorded output depends on whether the recorder had an executable"
  diff <(out_of "$T/ev-with/list-the-nodes.json") <(out_of "$T/ev-without/list-the-nodes.json") | head -20
  exit 1; }
# not by hiding a line: the record says the check was not asked for
out_of "$T/ev-without/list-the-nodes.json" | grep -q '^INFO knowledge .* the schema check was not asked for' || {
  echo "    the record does not say the schema check was not asked for"
  out_of "$T/ev-without/list-the-nodes.json"; exit 1; }
# and the recorder's normalisation no longer carries a mask for the line
grep -q 'index could not be asked' "$ROOT/lib/usecase.sh" \
  && { echo "    lib/usecase.sh still masks the knowledge line"; exit 1; }
# and a duration is masked wherever it stands, not only at the end of a line: the finish line
# reads "exit 0, 0s, tree <sha>", and a recorder that took 1s wrote a different catalogue
norm="$(MJ_BIN_DIR="$ROOT/bin" MJ_LIB_DIR="$ROOT/lib" bash -c '. "$1/lib/usecase.sh" && printf "%s\n" "OK verification t-1 — true — exit 0, 7s, tree a9eff10ec68c" "OK verification t-1 — true — exit 0, 12s" | mj_uc_normalise "$2"' _ "$ROOT" "$T")"
case "$norm" in *"7s"*|*"12s"*) echo "    a recorded duration is not masked:"; printf '%s\n' "$norm"; exit 1 ;; esac
exit 0

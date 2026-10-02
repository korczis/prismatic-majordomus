# majordomus-covers: none
# A claim reaches the reader with the test that settles it, or the reader cannot check it.
#
# `docs/CLAIMS.yaml` gives every claim a `test:` — the case that decides it — and for a long
# time the product projection dropped that field one layer above the page: a feature listed
# what it guarantees, linked each claim, and never said what would fail if the guarantee did
# not hold. A reader could reach the evidence in two more clicks, which is another way of
# saying the compliance table showed exposure and not proof.
#
# So two things are decided here. The claim reference carries the test where the claim names
# one, and the matrix row counts how many of a feature's claims are guaranteed *and* carry it.
# A claim that names `-` — planned or rejected, nothing to run yet — carries nothing: a
# projection that rendered a dash as a test would be inventing evidence, which is worse than
# showing none.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    skip: no jq"; exit 0; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj341.XXXXXX")"; trap 'rm -rf "$S"' EXIT

"$MJ" init >/dev/null
grep -q 'kind: claim' .ai/repo/knowledge/sources.yaml \
  || { echo "    init seeded no claim source class; this fixture proves nothing"; exit 1; }

mkdir -p docs test/cases
claims() {
  cat > docs/CLAIMS.yaml <<YAML
version: 1
statuses:
  - id: guaranteed
    meaning: Implemented, and a behavioural test proves it.
  - id: planned
    meaning: Specified and not implemented.
claims:
  - id: proven-claim
    claim: The thing this feature guarantees
    source: docs/PROBE.md
    implementation: lib/probe.sh
    test: $1
    status: guaranteed
  - id: planned-claim
    claim: The thing it does not do yet
    source: docs/PROBE.md
    implementation: lib/probe.sh
    test: '-'
    status: planned
YAML
}
claims 'test/cases/01_probe.sh'
cat > .ai/repo/features/a-probe-feature.md <<'MD'
---
schema: feature/v1
id: a-probe-feature
kind: feature
title: A feature that names what proves it
short_title: Probe
headline: One claim it guarantees and one it has only planned, so the difference is visible.
summary: The feature exists to be read by the projection, and its two claims differ in exactly the way the reader has to be able to see.
status: stable
weight: 900
featured: false
claims: [proven-claim, planned-claim]
tags: [probe]
---

# A feature that names what proves it
MD
git add -A >/dev/null && git commit -qm probe >/dev/null

show() { "$RB" product show a-probe-feature --format json > "$S/f.json" 2>"$S/err" || { echo "    product show failed:"; tail -3 "$S/err" | sed 's/^/      /'; exit 1; }; }
matrix() { "$RB" product matrix --format json > "$S/m.json" 2>"$S/err" || { echo "    product matrix failed:"; tail -3 "$S/err" | sed 's/^/      /'; exit 1; }; }

# ---------------------------------------------------------------- 1. the claim carries its test
show
t="$(jq -r '[.claim_refs[] | select(.id == "proven-claim")][0].test // "absent"' "$S/f.json")"
[ "$t" = "test/cases/01_probe.sh" ] \
  || { echo "    the guaranteed claim reached the projection with test '$t', not the case that settles it"; exit 1; }
echo "    a guaranteed claim reaches the projection with the test that settles it"

# ---------------------------------------------------------------- 2. and a dash is not a test
t="$(jq -r '[.claim_refs[] | select(.id == "planned-claim")][0].test // "absent"' "$S/f.json")"
[ "$t" = absent ] \
  || { echo "    a claim whose test is '-' was projected as having one: '$t'"; exit 1; }
echo "    a claim that names no test carries none, rather than a dash a reader would read as one"

# ---------------------------------------------------------------- 3. the matrix counts the proof
matrix
p="$(jq -r '[.rows[] | select(.id == "a-probe-feature")][0].proven' "$S/m.json")"
c="$(jq -r '[.rows[] | select(.id == "a-probe-feature")][0].claims' "$S/m.json")"
[ "$p" = 1 ] && [ "$c" = 2 ] \
  || { echo "    the matrix row says $p of $c proven; one of two claims is guaranteed with a test"; exit 1; }
echo "    the matrix row counts what is proven, with its denominator beside it"

# ---------------------------------------------------------------- 4. the mutation: take the test away
# The field is what makes the count mean anything. A claim still guaranteed, still linked, and
# no longer naming a case must stop counting as proven — otherwise the column measures status.
claims "'-'"
git add -A >/dev/null && git commit -qm "the claim stops naming its test" >/dev/null
show; matrix
t="$(jq -r '[.claim_refs[] | select(.id == "proven-claim")][0].test // "absent"' "$S/f.json")"
p="$(jq -r '[.rows[] | select(.id == "a-probe-feature")][0].proven' "$S/m.json")"
[ "$t" = absent ] || { echo "    the claim kept a test after it stopped naming one: '$t'"; exit 1; }
[ "$p" = 0 ] || { echo "    the matrix still counts $p proven after the only test was taken away"; exit 1; }
echo "    a guarantee that names no case is not counted as proven"

# ---------------------------------------------------------------- 5. and it comes back
claims 'test/cases/01_probe.sh'
git add -A >/dev/null && git commit -qm "the claim names its test again" >/dev/null
show; matrix
[ "$(jq -r '[.rows[] | select(.id == "a-probe-feature")][0].proven' "$S/m.json")" = 1 ] \
  || { echo "    naming the case again did not restore the count"; exit 1; }
echo "    naming the case again restores it, so the count follows the declaration"

echo "    a claim shows the test that settles it, and the matrix counts what is proven"

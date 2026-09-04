# Changing a canonical file changes the graph, with nothing generated edited by hand.
#
# Consistency between a model and its projection is worth nothing if the projection is
# stale, and a graph that has to be hand-maintained becomes a second source of truth within
# days. Every mutation here edits exactly one canonical file and asserts what moved and,
# just as importantly, what did not.
. "$ROOT/test/lib.sh"

"$MJ" init >/dev/null
# This repository declares where its implementation and its cases live, so that the chain a
# claim states — claim, implementing file, proving case — resolves to nodes.
cat >> .ai/repo/knowledge/sources.yaml <<'SRC'

  - id: library
    kind: implementation
    discovery: vcs
    pathspec: ':(glob)lib/*.sh'
    required: false

  - id: case
    kind: test
    discovery: vcs
    pathspec: ':(glob)test/cases/*.sh'
    required: false
SRC
mkdir -p docs .ai/repo/project/milestones .ai/repo/project/issues lib test/cases
cat > .ai/repo/project/project.yaml <<'EOF'
schema_version: 1
name: Fixture
repository: example/fixture
default_branch: master
EOF
mk_milestone() {
  cat > ".ai/repo/project/milestones/$1.yaml" <<EOF
id: $1
title: Outcome $1
slug: outcome-$1
order: $2
priority: p1
problem: "A problem."
outcome: "An outcome."
acceptance_criteria:
  - It is reached
validation:
  - true
evidence_required:
  - proof
EOF
}
mk_issue() {
  cat > ".ai/repo/project/issues/$1.yaml" <<EOF
id: $1
milestone: $2
title: Issue $1
slug: issue-$1
priority: p1
profile: implementation
objective: "Do the piece called $1."
scope:
  - src/$1
acceptance_criteria:
  - Done
validation:
  - true
evidence_required:
  - proof
EOF
}
mk_milestone M100 0
mk_milestone M200 1
mk_issue I0101 M100
mk_issue I0102 M100
printf 'depends_on:\n  - I0101\n' >> .ai/repo/project/issues/I0102.yaml

cat > docs/CLAIMS.yaml <<'EOF'
version: 1
claims:
  - id: a-claim
    claim: Something is guaranteed
    source: docs/SPEC.md
    implementation: lib/one.sh
    test: test/cases/98_one.sh
    status: guaranteed
EOF
printf '# The specification\n' > docs/SPEC.md
printf '#!/usr/bin/env bash\n' > lib/one.sh
printf '#!/usr/bin/env bash\n' > lib/two.sh
printf '# case one\n' > test/cases/98_one.sh
git add -A >/dev/null && git commit -q -m install

snapshot() { "$MJ" knowledge edges > "$1"; "$MJ" knowledge nodes > "$1.n"; }
snapshot base.txt
expect_grep '^depends_on +issue:I0102 +issue:I0101' base.txt
expect_grep '^implemented_by +claim:a-claim +implementation:lib/one\.sh' base.txt

# --- moving one dependency edge in the canonical file moves it in the graph, and only it
sed 's/^  - I0101$/  - I0199/' .ai/repo/project/issues/I0102.yaml > i.tmp
mv i.tmp .ai/repo/project/issues/I0102.yaml
mk_issue I0199 M200
git add -A >/dev/null && git commit -q -m redirect
snapshot after.txt
expect_no_grep '^depends_on +issue:I0102 +issue:I0101' after.txt
expect_grep '^depends_on +issue:I0102 +issue:I0199 +\.ai/repo/project/issues/I0102\.yaml:depends_on\.0' after.txt
# every OTHER edge is untouched: a graph that reshuffles unrelated edges cannot be reviewed
grep -v 'issue:I0102' base.txt | grep -v '^knowledge edges:' | LC_ALL=C sort > b-rest.txt
grep -v 'issue:I0102' after.txt | grep -v '^knowledge edges:' | grep -v 'issue:I0199' | LC_ALL=C sort > a-rest.txt
cmp -s b-rest.txt a-rest.txt || { echo "    moving one edge disturbed the rest of the graph"; diff b-rest.txt a-rest.txt; exit 1; }

# --- moving an issue to another milestone moves its part_of edge
sed 's/^milestone: M100$/milestone: M200/' .ai/repo/project/issues/I0102.yaml > i.tmp
mv i.tmp .ai/repo/project/issues/I0102.yaml
git add -A >/dev/null && git commit -q -m remilestone
snapshot after2.txt
expect_no_grep '^part_of +issue:I0102 +milestone:M100' after2.txt
expect_grep '^part_of +issue:I0102 +milestone:M200' after2.txt

# --- repointing a claim's implementation repoints the edge
sed 's#implementation: lib/one.sh#implementation: lib/two.sh#' docs/CLAIMS.yaml > c.tmp
mv c.tmp docs/CLAIMS.yaml
git add -A >/dev/null && git commit -q -m reimplement
snapshot after3.txt
expect_no_grep '^implemented_by +claim:a-claim +implementation:lib/one\.sh' after3.txt
expect_grep '^implemented_by +claim:a-claim +implementation:lib/two\.sh' after3.txt

# --- adding a claim adds its node and its three edges, and nothing else moves
cat >> docs/CLAIMS.yaml <<'EOF'
  - id: b-claim
    claim: Something else is guaranteed
    source: docs/SPEC.md
    implementation: lib/one.sh
    test: test/cases/98_one.sh
    status: guaranteed
EOF
git add -A >/dev/null && git commit -q -m addclaim
snapshot after4.txt
expect_grep '^claim +shared +[0-9a-f]{12} +claim:b-claim' after4.txt.n
[ "$(grep -c 'claim:b-claim' after4.txt)" = 3 ] \
  || { echo "    a new claim did not produce exactly its three declared edges"; grep 'b-claim' after4.txt; exit 1; }
grep -v 'b-claim' after4.txt | grep -v '^knowledge edges:' | LC_ALL=C sort > a4-rest.txt
grep -v '^knowledge edges:' after3.txt | LC_ALL=C sort > a3-rest.txt
cmp -s a3-rest.txt a4-rest.txt || { echo "    adding a claim disturbed the existing graph"; diff a3-rest.txt a4-rest.txt; exit 1; }

# --- deleting a canonical file removes its node and its edges, and dangles what pointed at it
rm .ai/repo/project/issues/I0199.yaml
git add -A >/dev/null && git commit -q -m delete
expect_exit 10 "$MJ" knowledge edges
expect_grep 'FAIL +knowledge +issue:I0102 -> issue:I0199 — declared depends_on'
# nodes and edges are two views of one derivation, so a defect in either is a defect in
# both: the node view exits 10 on the same dangling edge rather than reporting a clean set
# of nodes while the graph over them is broken.
expect_exit 10 "$MJ" knowledge nodes
"$MJ" knowledge nodes > after5.n 2>&1 || true
grep -v '^FAIL' after5.n > after5-nodes.txt
expect_grep 'issue:I0102' after5-nodes.txt
expect_no_grep 'issue:I0199' after5-nodes.txt

# --- and nothing generated was ever edited to make any of that happen
before_status="$(git status --porcelain)"
"$MJ" knowledge edges >/dev/null 2>&1 || true
"$MJ" knowledge nodes >/dev/null 2>&1 || true
[ "$(git status --porcelain)" = "$before_status" ] \
  || { echo "    the graph was materialised into the working tree"; git status --porcelain; exit 1; }

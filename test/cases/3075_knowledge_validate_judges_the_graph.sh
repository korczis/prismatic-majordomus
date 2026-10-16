# The knowledge graph is judged by one validator, asked by `knowledge validate`, doctor and
# watch alike (I0812, majordomus.knowledge-graph).
#
# Each fault below is introduced on its own, on a clean graph, and taken out again before the
# next: a duplicate id, an edge with a missing end, a declared path that no longer exists, a
# tracked source deleted from the working tree, an edge without provenance, and a broken link
# in prose. The first four the command fails on, because each breaks a relationship
# Majordomus defines; a deleted source is reported, because a deletion in progress is work in
# progress; a link an author wrote never fails, because it may point outside the repository.
# No index is stored, so none can be stale: the command says so, and nothing it, doctor or
# watch runs leaves a file behind.
. "$ROOT/test/lib.sh"
# The provenance guard can only be exercised by breaking the extractor, so the case runs its
# own copy of the tool.
mkdir -p "$T/tool"
fixture_repo "$T/tool" docs
MJ="$T/tool/bin/majordomus"
AWK="$T/tool/lib/knowledge.awk"
"$MJ" init >/dev/null

cat >> .ai/repo/knowledge/sources.yaml <<'SRC'

  - id: library
    kind: implementation
    discovery: vcs
    pathspec: ':(glob)lib/*.sh'
    required: false
SRC
mkdir -p docs lib .ai/repo/project/milestones .ai/repo/project/issues
cat > .ai/repo/project/project.yaml <<'YML'
schema_version: 1
name: Fixture
repository: example/fixture
default_branch: master
YML
cat > .ai/repo/project/milestones/M100.yaml <<'YML'
id: M100
title: An outcome
slug: an-outcome
order: 0
priority: p1
problem: "A problem."
outcome: "An outcome."
acceptance_criteria:
  - It is reached
validation:
  - "true"
evidence_required:
  - proof
YML
for n in 1 2; do
  cat > ".ai/repo/project/issues/I010$n.yaml" <<YML
id: I010$n
milestone: M100
title: Issue $n
slug: issue-$n
priority: p1
profile: implementation
objective: "Do piece $n."
scope:
  - src/$n
acceptance_criteria:
  - Done
validation:
  - "true"
evidence_required:
  - proof
YML
done
cat > docs/CLAIMS.yaml <<'YML'
version: 1
claims:
  - id: a-claim
    claim: Something is guaranteed
    source: docs/SPEC.md
    implementation: lib/thing.sh
    status: guaranteed
YML
printf '# The specification\n\nSee [the guide](GUIDE.md).\n' > docs/SPEC.md
printf '# The guide\n' > docs/GUIDE.md
printf '#!/usr/bin/env bash\n' > lib/thing.sh
git add -A >/dev/null && git commit -q -m fixture
clean="$(git rev-parse HEAD)"
restore() { git reset -q --hard "$clean"; git clean -qfd -- docs lib .ai/repo/project >/dev/null; }

# --- a clean graph: no failure, the absence of an index stated, nothing written
expect_exit 0 "$MJ" knowledge validate
expect_grep '^knowledge validate: [0-9]+ node\(s\), [0-9]+ edge\(s\); 0 failure\(s\), 0 warning\(s\); no stored index'
expect_exit 0 "$MJ" --json knowledge validate
expect_grep '"index":"none"'
expect_grep '"failures":0,"warnings":0\}$'

# --- a duplicate id: a second file claims an issue's identity
sed 's/^id: I0102$/id: I0101/' .ai/repo/project/issues/I0102.yaml > .ai/repo/project/issues/I0103.yaml
git add -A >/dev/null && git commit -q -m duplicate
# The executable's index sees it first when it can be asked, and refuses both claimants; the
# compiler's own check is what answers where the index is not asked, as under doctor and
# watch. Either way it is an owned failure naming both files.
expect_exit 10 "$MJ" knowledge validate
expect_grep 'FAIL +knowledge +\.ai/repo/project/issues/I0103\.yaml — refused_source \(owned\): the index refused it \(duplicate_identity\): majordomus://issue/I0101 is claimed by'
expect_exit 10 env MAJORDOMUS_KNOWLEDGE_SCHEMA=unasked "$MJ" knowledge validate
expect_grep 'FAIL +knowledge +issue:I0101 — duplicate_id \(owned\): claimed by \.ai/repo/project/issues/I0101\.yaml and by \.ai/repo/project/issues/I0103\.yaml +\[reproduce: majordomus knowledge validate\]'
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT +knowledge +issue:I0101 — duplicate_id \(owned\)'
restore

# --- an edge with a missing end: a dependency on an issue that does not exist
printf 'depends_on:\n  - I0199\n' >> .ai/repo/project/issues/I0101.yaml
git add -A >/dev/null && git commit -q -m dangle
expect_exit 10 "$MJ" knowledge validate
expect_grep 'FAIL +knowledge +issue:I0101 -> issue:I0199 — dangling_edge \(owned\): declared depends_on'
expect_exit 10 "$MJ" --json knowledge validate
expect_grep '"level":"FAIL","code":"dangling_edge","owner":"owned","subject":"issue:I0101 -> issue:I0199".*"reproduce":"majordomus knowledge validate"'
# watch reports the same finding as drift and exits 11; doctor, where the rule is advisory,
# warns. The message and the reproduce are the validator's under all three.
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT +knowledge +issue:I0101 -> issue:I0199 — dangling_edge \(owned\)'
"$MJ" doctor > doctor.txt 2>&1 || true
expect_grep 'WARN +knowledge +issue:I0101 -> issue:I0199 — dangling_edge \(owned\).*\[reproduce: majordomus knowledge validate\]' doctor.txt
rm -f doctor.txt
restore

# --- a declared path that no longer exists: the claim's implementation is gone
sed 's#implementation: lib/thing.sh#implementation: lib/gone.sh#' docs/CLAIMS.yaml > c.tmp && mv c.tmp docs/CLAIMS.yaml
git add -A >/dev/null && git commit -q -m gone
expect_exit 10 "$MJ" knowledge validate
expect_grep 'FAIL +knowledge +claim:a-claim — missing_target \(owned\): declares implemented_by lib/gone\.sh'
restore

# --- a tracked source deleted from the working tree: reported, never a failure under the
#     command, and drift under watch
rm lib/thing.sh
expect_exit 0 "$MJ" knowledge validate
expect_grep 'WARN +knowledge +lib/thing\.sh — source_missing \(source\): is a tracked source the working tree no longer holds'
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT +knowledge +lib/thing\.sh — source_missing'
restore

# --- an edge without provenance is refused by the compiler and failed by the validator
cp "$AWK" "$AWK.orig"
sed 's#p ":claims\." n "\.implementation"#""#' "$AWK.orig" > "$AWK"
grep -q '"implemented_by", "")' "$AWK" \
  || { echo "    the provenance mutation did not take"; grep -n 'implemented_by' "$AWK" | head; exit 1; }
expect_exit 10 "$MJ" knowledge validate
expect_grep 'FAIL +knowledge +claim:a-claim -> implementation:lib/thing\.sh — edge_incomplete \(owned\)'
cp "$AWK.orig" "$AWK"

# --- a broken link an author wrote is reported as external and never fails, under the
#     command or under watch; doctor summarises it in one line
printf '\nA [missing thing](NOWHERE.md).\n' >> docs/SPEC.md
git add -A >/dev/null && git commit -q -m broken
expect_exit 0 "$MJ" knowledge validate
expect_grep 'WARN +knowledge +document:docs/SPEC\.md — broken_link \(external\): links to docs/NOWHERE\.md'
# (the fixture has no projections generated, so watch drifts on those and exits 11; what
# matters here is that nothing about the knowledge graph is drift)
LAST_OUT="$("$MJ" watch 2>&1)" || true
expect_no_grep 'DRIFT +knowledge'
expect_grep 'INFO +knowledge +links — 1 link\(s\) written in prose resolve to nothing'
restore

# --- doctor says whether the graph can be built here, and leaves nothing behind: no file in
#     the tree, and no temporary file of its own
mkdir -p "$T/tmp"
before="$(git status --porcelain --untracked-files=all)"
TMPDIR="$T/tmp" "$MJ" doctor > doctor.txt 2>&1 || true
expect_grep 'OK +knowledge +graph — buildable here: [0-9]+ node\(s\), [0-9]+ edge\(s\), compiled into a temporary file and removed' doctor.txt
rm -f doctor.txt
[ "$(git status --porcelain --untracked-files=all)" = "$before" ] \
  || { echo "    doctor changed the working tree"; git status --porcelain; exit 1; }
left="$(find "$T/tmp" -name 'mj.kv*' | head -n 3)"
[ -z "$left" ] || { echo "    the graph validator left a temporary file behind: $left"; exit 1; }
# a source list that does not parse makes the graph unbuildable, and doctor says so rather
# than stopping
printf 'version: [\n' > .ai/repo/knowledge/sources.yaml
"$MJ" doctor > doctor.txt 2>&1 || true
expect_grep 'WARN +knowledge +graph — the graph cannot be built in this checkout' doctor.txt
rm -f doctor.txt
restore

# --- the doctrine is declared, enforced by both commands, and names this case
expect_exit 0 "$MJ" doctrine list
expect_grep '^majordomus\.knowledge-graph +advisory +mj_validate_knowledge_graph +doctor,watch$'

# --- read-only: every run of the command leaves git status byte-identical
before="$(git status --porcelain --untracked-files=all)"
"$MJ" knowledge validate >/dev/null 2>&1 || true
"$MJ" --json knowledge validate >/dev/null 2>&1 || true
[ "$(git status --porcelain --untracked-files=all)" = "$before" ] \
  || { echo "    knowledge validate changed the working tree"; git status --porcelain; exit 1; }

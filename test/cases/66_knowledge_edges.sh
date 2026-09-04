# An edge exists only where a file states the relationship, and only with the place it was
# stated.
#
# The rule this enforces is a refusal: an edge nobody can trace back to a line is not a fact,
# it is a guess wearing a fact's clothes — and it is a guess the person best placed to notice
# it is wrong will never see, because nothing points them at where it came from.
. "$ROOT/test/lib.sh"
# Two of the guards can only be exercised by breaking the extractor, so the case runs its own
# copy of the tool.
mkdir -p "$T/tool"
fixture_repo "$T/tool" docs
MJ="$T/tool/bin/majordomus"
AWK="$T/tool/lib/knowledge.awk"

"$MJ" init >/dev/null
mkdir -p docs .majordomus/project/milestones .majordomus/project/issues
cat > .majordomus/project/project.yaml <<'EOF'
schema_version: 1
name: Fixture
repository: example/fixture
default_branch: master
EOF
cat > .majordomus/project/milestones/M100.yaml <<'EOF'
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
  - true
evidence_required:
  - proof
claims:
  - a-claim
EOF
for n in 1 2; do
  cat > ".majordomus/project/issues/I010$n.yaml" <<EOF
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
  - true
evidence_required:
  - proof
EOF
done
printf 'depends_on:\n  - I0101\n' >> .majordomus/project/issues/I0102.yaml

cat > docs/CLAIMS.yaml <<'EOF'
version: 1
claims:
  - id: a-claim
    claim: Something is guaranteed
    source: docs/SPEC.md
    implementation: lib/thing.sh
    test: test/cases/99_thing.sh
    status: guaranteed
EOF
mkdir -p lib test/cases
printf '# The specification\n\nSee [the guide](GUIDE.md) and [outside](https://example.com/x).\n' > docs/SPEC.md
printf '# The guide\n' > docs/GUIDE.md
printf '#!/usr/bin/env bash\n' > lib/thing.sh
printf '# a case\n' > test/cases/99_thing.sh
git add -A >/dev/null && git commit -q -m install

"$MJ" knowledge edges > e.txt
"$MJ" knowledge nodes > n.txt

# --- the relationships the repository already states, and each one's provenance
expect_grep '^part_of +issue:I0101 +milestone:M100 +\.majordomus/project/issues/I0101\.yaml:milestone$' e.txt
expect_grep '^depends_on +issue:I0102 +issue:I0101 +\.majordomus/project/issues/I0102\.yaml:depends_on\.0$' e.txt
expect_grep '^declares +milestone:M100 +claim:a-claim +\.majordomus/project/milestones/M100\.yaml:claims\.0$' e.txt
expect_grep '^specified_by +claim:a-claim +document:docs/SPEC\.md +docs/CLAIMS\.yaml:claims\.0\.source$' e.txt
expect_grep '^implemented_by +claim:a-claim +implementation:lib/thing\.sh +docs/CLAIMS\.yaml:claims\.0\.implementation$' e.txt
expect_grep '^tested_by +claim:a-claim +test:test/cases/99_thing\.sh +docs/CLAIMS\.yaml:claims\.0\.test$' e.txt

# --- EVERY edge carries a provenance. Not most of them.
awk '$1 != "knowledge" && NF > 0 && $0 !~ /^knowledge edges:/ { if (NF < 4) { print "row with no provenance: " $0; bad = 1 } }
     END { exit bad + 0 }' e.txt || { echo "    an edge was emitted without provenance"; exit 1; }

# --- an inline link becomes an edge; a link with a scheme never does
expect_grep '^references +document:docs/SPEC\.md +document:docs/GUIDE\.md +docs/SPEC\.md:3$' e.txt
expect_no_grep 'example\.com' e.txt

# --- a path inside a fenced code block is an example of a path, not a reference to one
cat > docs/SPEC.md <<'EOF'
# The specification

Real link to [the guide](GUIDE.md).

```
see [not a link](GUIDE.md) and cat docs/GUIDE.md
```

~~~markdown
[also not a link](GUIDE.md)
~~~
EOF
git add -A >/dev/null && git commit -q -m fences
"$MJ" knowledge edges --type references > refs.txt
n="$(grep -cE 'document:docs/SPEC\.md +document:docs/GUIDE\.md' refs.txt || true)"
[ "$n" = 1 ] || { echo "    fenced code produced $n reference edges, expected 1"; cat refs.txt; exit 1; }

# --- a broken relative link is reported and names where it was written; it never gates,
#     because a document may deliberately point outside the repository
printf '\nA [missing thing](NOWHERE.md).\n' >> docs/SPEC.md
git add -A >/dev/null && git commit -q -m broken
expect_exit 0 "$MJ" knowledge edges
expect_grep 'WARN +knowledge +document:docs/SPEC\.md — links to docs/NOWHERE\.md'
# a link to a directory is ordinary prose and is not reported
printf '\nThe [cases](test/cases) live here.\n' >> docs/SPEC.md
git add -A >/dev/null && git commit -q -m dirlink
expect_exit 0 "$MJ" knowledge edges
expect_no_grep 'links to test/cases'

# --- a DECLARED relationship to a file that does not exist is a different severity: that is
#     a broken promise, not an author's typo in prose
sed 's#implementation: lib/thing.sh#implementation: lib/gone.sh#' docs/CLAIMS.yaml > c.tmp && mv c.tmp docs/CLAIMS.yaml
git add -A >/dev/null && git commit -q -m gone
expect_exit 10 "$MJ" knowledge edges
expect_grep 'FAIL +knowledge +claim:a-claim — declares implemented_by lib/gone\.sh'
git revert -q --no-edit HEAD >/dev/null 2>&1 || { git checkout -q HEAD~1 -- docs/CLAIMS.yaml; git commit -q -am restore; }

# --- a dependency on an issue that does not exist dangles, and the report names both ends
printf 'depends_on:\n  - I0199\n' >> .majordomus/project/issues/I0101.yaml
git add -A >/dev/null && git commit -q -m dangle
expect_exit 10 "$MJ" knowledge edges
expect_grep 'FAIL +knowledge +issue:I0101 -> issue:I0199 — declared depends_on'
git checkout -q HEAD~1 -- .majordomus/project/issues/I0101.yaml
git commit -q -am undangle

# --- an edge with no provenance is refused, not emitted with a blank. A blank provenance
#     reads as "source unknown" and is indistinguishable from one nobody recorded.
cp "$AWK" "$AWK.orig"
sed 's#p ":claims\." n "\.test"#""#' "$AWK.orig" > "$AWK"
grep -q 'path_edge(from, f(p, "claims." n ".test"),            "tested_by",      "")' "$AWK" \
  || { echo "    the provenance mutation did not take"; grep -n 'tested_by' "$AWK" | head; exit 1; }
expect_exit 10 "$MJ" knowledge edges
expect_grep 'FAIL +knowledge +claim:a-claim -> .* an edge needs all four'
expect_no_grep '^tested_by +claim:a-claim'
cp "$AWK.orig" "$AWK"

# --- an undeclared edge type is an error, not a new vocabulary word
sed 's#"tested_by",      p ":claims\."#"proves",      p ":claims."#' "$AWK.orig" > "$AWK"
grep -q '"proves"' "$AWK" || { echo "    the edge-type mutation did not take"; exit 1; }
expect_exit 10 "$MJ" knowledge edges
expect_grep 'FAIL +knowledge +proves — is not a declared edge type'
cp "$AWK.orig" "$AWK"

# --- read-only
rm -f e.txt n.txt refs.txt
before_status="$(git status --porcelain)"
"$MJ" knowledge edges >/dev/null 2>&1 || true
"$MJ" knowledge edges --type part_of >/dev/null 2>&1 || true
"$MJ" --json knowledge edges >/dev/null 2>&1 || true
[ "$(git status --porcelain)" = "$before_status" ] \
  || { echo "    deriving edges changed the working tree"; git status --porcelain; exit 1; }

# --- JSON names the provenance, so a caller can check an edge without re-deriving it
expect_exit 0 "$MJ" --json knowledge edges --type part_of
expect_grep '"from":"issue:I0101","to":"milestone:M100","type":"part_of","provenance":"\.majordomus/project/issues/I0101\.yaml:milestone"'

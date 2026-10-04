# majordomus-covers: none
# A kind the tool's own skeleton declares is a kind its knowledge graph knows.
#
# `share/skeleton/ai/repo/knowledge/sources.yaml` declares `feature`, `moment`, `area`,
# `audience`, `deployment`, `intent`, `workspace` and more, and `lib/knowledge.awk` listed none
# of them: every feature was an `unknown` node with no edge to the claims, rules and documents
# its front matter names, and no repository setting could add the rule. This repository held
# 100 unknown nodes; OSCILLA found it while adopting the tool (2026-10-04).
#
# The case builds a repository from the skeleton, gives it one domain and one feature naming
# that domain, a vendored rule, a claim and a document, and requires both to be typed nodes,
# the domain to be a `part_of` edge, each other reference to be a `references` edge carrying
# the line that stated it, and no `unknown_kind` finding at all.
. "$ROOT/test/lib.sh"

mkdir -p "$T/tool"
fixture_repo "$T/tool" docs
MJ="$T/tool/bin/majordomus"

"$MJ" init >/dev/null

mkdir -p .ai/repo/features/domains docs
cat > docs/CLAIMS.yaml <<'EOF'
version: 1
claims:
  - id: a-claim
    claim: Something is guaranteed
    source: docs/SPEC.md
    status: guaranteed
EOF
printf '# The specification\n' > docs/SPEC.md
cat > .ai/repo/features/domains/a-domain.md <<'EOF'
---
schema: domain/v1
id: a-domain
kind: domain
title: One area of the product
status: stable
---

# One area of the product
EOF
cat > .ai/repo/features/a-feature.md <<'EOF'
---
schema: feature/v1
id: a-feature
kind: feature
title: One thing the product does
status: stable
domain: a-domain
rules: [majordomus.roadmap-integrity]
claims: [a-claim]
docs: [docs/SPEC.md]
---

# One thing the product does
EOF
git add -A >/dev/null && git commit -q -m install

"$MJ" knowledge nodes > n.txt 2>&1
"$MJ" knowledge edges > e.txt 2>&1

echo "    the feature is a typed node with its declared identity and title"
expect_grep '^feature +shared +[0-9a-f]{12} +feature:a-feature +One thing the product does' n.txt
expect_no_grep '^unknown ' n.txt

echo "    the domain is a typed node too"
expect_grep '^domain +shared +[0-9a-f]{12} +domain:a-domain +One area of the product' n.txt

echo "    no declared kind is left without a rule"
expect_no_grep 'unknown_kind|has no rule for' n.txt
expect_no_grep 'unknown_kind|has no rule for' e.txt

echo "    the feature is part of the domain it names"
expect_grep '^part_of +feature:a-feature +domain:a-domain +\.ai/repo/features/a-feature\.md:domain$' e.txt

echo "    each reference is an edge, with the line that stated it"
expect_grep '^references +feature:a-feature +rule:majordomus\.roadmap-integrity +\.ai/repo/features/a-feature\.md:rules\.0$' e.txt
expect_grep '^references +feature:a-feature +claim:a-claim +\.ai/repo/features/a-feature\.md:claims\.0$' e.txt
expect_grep '^references +feature:a-feature +document:docs/SPEC\.md +\.ai/repo/features/a-feature\.md:docs\.0$' e.txt

echo "    and every one of them resolves"
expect_no_grep 'unresolved_reference.*feature:a-feature|missing_target.*feature:a-feature' e.txt

# majordomus-covers: doctor
# majordomus-negative: doctor
# claims: bootstrap-chain
# A generated bootstrap names only rules in force.
#
# The 0.12 provider templates rendered "The rule is `project.worktree-topology`" into every
# adopter's AGENTS.md, CLAUDE.md and GEMINI.md, and an adopter has no such rule. doctor said
# nothing: its links check reads Markdown links, and a rule id is not one. doctor now reads
# every rule id a generated bootstrap names and fails each one the effective rule set (what
# `majordomus rules list` lists) does not provide, naming the file, the line and the id.
#
# The fixture does not depend on what the shipped templates say. Those are being changed to
# stop naming the rule (#783), so a case that waited for the stock text would go blind the day
# that lands. Instead it overrides the agents template with today's distribution template plus
# a line of its own naming `project.worktree-topology`, and the region-mode CLAUDE.md with
# today's claude-code template plus one more, so the reference is in the generated content
# whatever the stock text says. Every line of a generated file that names the rule must be
# reported — the case's own line always, and the stock line too while the template carries it.
#
# Then the policy around the finding: a vendored id in force is not a finding and a vendored
# id that is not is one; `@<version>` resolves against that exact identity; a fenced example,
# a path and a host document's own prose around a region are not read as claims. Adding the
# rule clears every finding, and deprecating it brings them back: a deprecated rule is not in
# force.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
# hooks wired, so a doctor with nothing else wrong exits 0 and the exit code means this
printf '#!/bin/sh\n%s doctor || exit $?\n' "$MJ" > .git/hooks/pre-commit
printf '#!/bin/sh\n%s finish --check || exit $?\n' "$MJ" > .git/hooks/pre-push
chmod +x .git/hooks/pre-commit .git/hooks/pre-push

# CLAUDE.md is a region projection in a host document of the repository's own
sed 's/^    target: CLAUDE.md$/    target: CLAUDE.md\
    mode: region/' .ai/repo/policy.yaml > policy.new && mv policy.new .ai/repo/policy.yaml
grep -q '^    mode: region$' .ai/repo/policy.yaml || { echo "    the fixture policy did not take mode: region"; exit 1; }
printf '# CLAUDE.md\n\nThe host names `project.host-prose-only`, which is not generated.\n\n' > CLAUDE.md

mkdir -p .ai/repo/providers
# a directory of the layer says what it is for
cat > .ai/repo/providers/README.md <<'MD'
---
schema: context/v1
id: ai.repo.providers
kind: context
title: Provider overrides
description: The fixture's own provider templates, each the distribution's with a line added.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
---

# Provider overrides
MD
cp "$ROOT/share/providers/agents.tmpl" .ai/repo/providers/agents.tmpl
cat >> .ai/repo/providers/agents.tmpl <<'TMPL'

Case 899 names `project.worktree-topology`, a rule this fixture does not have.
It also names `majordomus.bootstrap-integrity` and `majordomus.bootstrap-integrity@1`, which are in force,
`majordomus.bootstrap-integrity@9` and `majordomus.no-such-rule`, which are not,
and the paths docs/project.notes.md and https://example.org/project.elsewhere/, which are not rule ids.

```yaml
id: project.fenced-example     # an example is quoted, not promised
```
TMPL
cp "$ROOT/share/providers/claude-code.tmpl" .ai/repo/providers/claude-code.tmpl
printf '\nCase 899 names `project.worktree-topology` inside the region too.\n' >> .ai/repo/providers/claude-code.tmpl
expect_exit 0 "$MJ" update
grep -q 'project.host-prose-only' CLAUDE.md && grep -q 'majordomus:begin' CLAUDE.md \
  || { echo "    CLAUDE.md is not a region inside the host document"; exit 1; }

# --- unresolved: one FAIL per line, with the file, the line, the id and a reproduce command
expect_exit 10 "$MJ" doctor
out="$LAST_OUT"
own="$(grep -n '^Case 899 names `project.worktree-topology`, a rule' AGENTS.md | cut -d: -f1)"
[ -n "$own" ] || { echo "    the override's line is not in AGENTS.md"; exit 1; }
expect_grep "^FAIL rule-refs +AGENTS.md — AGENTS.md line $own names rule project.worktree-topology, which is not in the effective rule set .*\[reproduce: sed -n '${own}p' AGENTS.md; majordomus rules show project.worktree-topology\]$"
reported=0
for f in AGENTS.md CLAUDE.md GEMINI.md; do
  for n in $(grep -n 'project\.worktree-topology' "$f" | cut -d: -f1); do
    LAST_OUT="$out"
    expect_grep "^FAIL rule-refs +$f — $f line $n names rule project.worktree-topology, "
    reported=$((reported+1))
  done
done
[ "$reported" -ge 2 ] || { echo "    expected the reference in AGENTS.md and in the CLAUDE.md region, found $reported"; exit 1; }
LAST_OUT="$out"
expect_grep "^FAIL rule-refs +AGENTS.md — AGENTS.md line [0-9]+ names rule majordomus.no-such-rule, "
expect_grep "^FAIL rule-refs +AGENTS.md — AGENTS.md line [0-9]+ names rule majordomus.bootstrap-integrity@9, .*majordomus rules show majordomus.bootstrap-integrity\]$"
expect_no_grep 'names rule majordomus.bootstrap-integrity(@1)?,'
expect_no_grep 'project\.(fenced-example|notes|elsewhere|host-prose-only)'
expect_no_grep 'OK +bootstrap +[0-9]+ projection'
# the line a finding names is the file's, not the region's: CLAUDE.md's host text comes first
region="$(grep -n 'inside the region too' CLAUDE.md | cut -d: -f1)"
expect_grep "^FAIL rule-refs +CLAUDE.md — CLAUDE.md line $region names rule project.worktree-topology, "
# the reproduce command reproduces it
expect_exit 12 "$MJ" rules show project.worktree-topology
expect_grep "no rule 'project.worktree-topology' in the effective set"

# --- the rule in force clears every finding about it; the other two remain until removed
mkdir -p .ai/repo/rules/project
cat > .ai/repo/rules/project/worktree-topology.v1.md <<'RULE'
---
id: project.worktree-topology
version: 1
kind: rule
title: Worktree topology
description: A linked worktree lives at the canonical path for its branch.
statement: A linked worktree lives at the canonical path for its branch.
status: active
class: advisory
depends_on: []
---

# Rationale

The fixture's own rule, so the bootstrap that names it names a rule in force.
RULE
grep -v -e 'majordomus.no-such-rule' .ai/repo/providers/agents.tmpl | sed 's/ and `majordomus.bootstrap-integrity@1`, which are in force,/ and `majordomus.bootstrap-integrity@1`, which are in force./; /majordomus.bootstrap-integrity@9/d' > tmpl.new
mv tmpl.new .ai/repo/providers/agents.tmpl
grep -q 'bootstrap-integrity@9\|no-such-rule' .ai/repo/providers/agents.tmpl && { echo "    the fixture still names the unresolved vendored ids"; exit 1; }
expect_exit 0 "$MJ" update
expect_exit 0 "$MJ" doctor
expect_no_grep 'rule-refs'
expect_grep 'OK +bootstrap +3 projection\(s\) — each points at .ai/README.md, carries no rule of its own and names only rules in force'

# --- a deprecated rule is not in force
sed 's/^status: active$/status: deprecated/' .ai/repo/rules/project/worktree-topology.v1.md > rule.new
mv rule.new .ai/repo/rules/project/worktree-topology.v1.md
expect_exit 10 "$MJ" doctor
expect_grep "^FAIL rule-refs +AGENTS.md — AGENTS.md line $own names rule project.worktree-topology, "

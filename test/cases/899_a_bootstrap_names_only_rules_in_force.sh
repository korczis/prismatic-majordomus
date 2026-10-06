# majordomus-covers: doctor
# majordomus-negative: doctor
# claims: bootstrap-chain
# A generated bootstrap names only rules in force.
#
# The 0.12 provider templates rendered "The rule is `project.worktree-topology`" into every
# adopter's AGENTS.md, CLAUDE.md and GEMINI.md, and an adopter has no such rule. doctor said
# nothing: its links check reads Markdown links, and a rule id is not one. doctor now reads
# every rule id a generated bootstrap names and reports each one the effective rule set (what
# `majordomus rules list` lists) does not provide, naming the file, the line and the id.
#
# The grade is whether `majordomus update` fixes it, because an adopter's pre-commit hook is
# doctor and a FAIL that appeared on upgrade would refuse every commit:
#   WARN (exit 0)  a stale bootstrap: the content matches its stamp, so an older template wrote
#                  it, and the current template no longer names the id
#   FAIL (exit 10) the current template names the id itself, so update would write it again;
#                  or the line was written by hand into a generated file
#
# The fixture does not depend on what the shipped templates say. They stopped naming the rule
# in #783 and may change again, so a case that waited for the stock text would go blind. It
# overrides the agents template with today's distribution template plus lines of its own, and
# the region-mode CLAUDE.md with today's claude-code template plus one more, so every id it
# asserts on is in the generated content whatever the stock text says. A stale region is made
# the way an upgrade makes one: render, then take a line out of the template without updating.
#
# Then the policy around the finding: a vendored id in force is not a finding and a vendored
# id that is not is one; `@<version>` resolves against that exact identity; a fenced example,
# a path and a host document's own prose around a region are not read as claims; the line is
# the file's, not the region's. Adding the rule clears its findings, and deprecating it brings
# them back: a deprecated rule is not in force.
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
description: The fixture's own provider templates, each the distribution's with lines added.
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
Case 899 once named `project.retired-by-a-newer-template`, which a newer template drops.
It also names `majordomus.bootstrap-integrity` and `majordomus.bootstrap-integrity@1`, which are in force.
It names `majordomus.bootstrap-integrity@9` and `majordomus.no-such-rule`, which are not.
And the paths docs/project.notes.md and https://example.org/project.elsewhere/, which are not rule ids.

```yaml
id: project.fenced-example     # an example is quoted, not promised
```
TMPL
cp "$ROOT/share/providers/claude-code.tmpl" .ai/repo/providers/claude-code.tmpl
printf '\nCase 899 names `project.worktree-topology` inside the region too.\n' >> .ai/repo/providers/claude-code.tmpl
expect_exit 0 "$MJ" update
grep -q 'project.host-prose-only' CLAUDE.md && grep -q 'majordomus:begin' CLAUDE.md \
  || { echo "    CLAUDE.md is not a region inside the host document"; exit 1; }

# the upgrade: the template drops a line, and the file rendered from the old one stays
grep -v 'retired-by-a-newer-template' .ai/repo/providers/agents.tmpl > tmpl.new
mv tmpl.new .ai/repo/providers/agents.tmpl
# and somebody writes a line into a generated file by hand
cp GEMINI.md gemini.orig
printf 'A hand names `project.typed-by-hand` here.\n' >> GEMINI.md

# --- FAIL: what update would write again, and what a hand wrote; WARN: what update removes
expect_exit 10 "$MJ" doctor
out="$LAST_OUT"
own="$(grep -n '^Case 899 names `project.worktree-topology`, a rule' AGENTS.md | cut -d: -f1)"
[ -n "$own" ] || { echo "    the override's line is not in AGENTS.md"; exit 1; }
expect_grep "^FAIL rule-refs +AGENTS.md — AGENTS.md line $own names rule project.worktree-topology, which is not in the effective rule set, and the current template .ai/repo/providers/agents.tmpl names it too, so majordomus update would write it again .*\[reproduce: sed -n '${own}p' AGENTS.md; majordomus rules show project.worktree-topology\]$"
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
# the line a finding names is the file's, not the region's: CLAUDE.md's host text comes first
region="$(grep -n 'inside the region too' CLAUDE.md | cut -d: -f1)"
expect_grep "^FAIL rule-refs +CLAUDE.md — CLAUDE.md line $region names rule project.worktree-topology, .*current template .ai/repo/providers/claude-code.tmpl"
expect_grep "^FAIL rule-refs +AGENTS.md — AGENTS.md line [0-9]+ names rule majordomus.no-such-rule, .*update would write it again"
expect_grep "^FAIL rule-refs +AGENTS.md — AGENTS.md line [0-9]+ names rule majordomus.bootstrap-integrity@9, .*majordomus rules show majordomus.bootstrap-integrity\]$"
hand="$(grep -n 'project.typed-by-hand' GEMINI.md | cut -d: -f1)"
expect_grep "^FAIL rule-refs +GEMINI.md — GEMINI.md line $hand names rule project.typed-by-hand, which is not in the effective rule set, in content that no longer matches its stamp, .*\[reproduce: sed -n '${hand}p' GEMINI.md; majordomus rules show project.typed-by-hand\]$"
retired="$(grep -n 'retired-by-a-newer-template' AGENTS.md | cut -d: -f1)"
expect_grep "^WARN rule-refs +AGENTS.md — stale bootstrap: AGENTS.md line $retired names rule project.retired-by-a-newer-template, which is not in the effective rule set; an older template wrote it and the current one does not, so run majordomus update  \[reproduce: majordomus update\]$"
expect_no_grep '^FAIL rule-refs .*retired-by-a-newer-template'
expect_no_grep 'names rule majordomus.bootstrap-integrity(@1)?,'
expect_no_grep 'project\.(fenced-example|notes|elsewhere|host-prose-only)'
expect_no_grep 'OK +bootstrap +[0-9]+ projection'
# the reproduce command reproduces it
expect_exit 12 "$MJ" rules show project.worktree-topology
expect_grep "no rule 'project.worktree-topology' in the effective set"

# --- WARN alone: the hand is undone, the rule is in force, and the two unresolved vendored ids
#     leave the template; the file still carries three lines the current template does not
#     write, and each is a stale bootstrap that does not refuse the commit
mv gemini.orig GEMINI.md
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
grep -v 'majordomus.no-such-rule' .ai/repo/providers/agents.tmpl > tmpl.new
mv tmpl.new .ai/repo/providers/agents.tmpl
grep -q 'bootstrap-integrity@9\|no-such-rule\|retired-by' .ai/repo/providers/agents.tmpl \
  && { echo "    the template still names an unresolved id"; exit 1; }
expect_exit 0 "$MJ" doctor
expect_grep 'doctor: 0 failure'
expect_no_grep '^FAIL rule-refs'
expect_grep '^WARN rule-refs +AGENTS.md — stale bootstrap: AGENTS.md line [0-9]+ names rule project.retired-by-a-newer-template, '
expect_grep '^WARN rule-refs +AGENTS.md — stale bootstrap: AGENTS.md line [0-9]+ names rule majordomus.no-such-rule, '
expect_grep '^WARN rule-refs +AGENTS.md — stale bootstrap: AGENTS.md line [0-9]+ names rule majordomus.bootstrap-integrity@9, '
expect_no_grep 'names rule project.worktree-topology'

# --- update is the whole fix: every finding clears
expect_exit 0 "$MJ" update
expect_exit 0 "$MJ" doctor
expect_no_grep 'rule-refs'
expect_grep 'OK +bootstrap +3 projection\(s\) — each points at .ai/README.md, carries no rule of its own and names only rules in force'

# --- a deprecated rule is not in force, and the current template still names it
sed 's/^status: active$/status: deprecated/' .ai/repo/rules/project/worktree-topology.v1.md > rule.new
mv rule.new .ai/repo/rules/project/worktree-topology.v1.md
expect_exit 10 "$MJ" doctor
expect_grep "^FAIL rule-refs +AGENTS.md — AGENTS.md line $own names rule project.worktree-topology, "

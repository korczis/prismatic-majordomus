# majordomus-covers: rules doctor
# A project's rule is enforced by the project's own command (I4255). An adopter wrote
# nineteen blocking project rules and every one reported "no validator; see the rule",
# because a validator must be an mj_validate_* function in this tool's source, while the
# commands that did check them were already declared as the policy's enforcement entries.
#
#   a rule naming the policy entry that checks it loads and is listed as enforced by it;
#   doctor confirms the entry is declared, and refuses a name the policy does not declare;
#   a rule naming both a validator and an entry is refused: one thing checks a rule.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
P=.ai/repo/rules/project
mkdir -p "$P"

# prule FILE ID [BLOCK-LINE ...] — a valid blocking project rule with an x-majordomus block
prule() {
  local f="$1" id="$2"; shift 2
  { cat <<Y
---
id: $id
version: 1
kind: rule
title: Rule $id
description: What $id requires, in one sentence.
statement: The normative sentence $id asks a worker to follow.
status: active
class: blocking
depends_on: []
tags: [fixture]
Y
    printf '\nx-majordomus:\n'; printf '  %s\n' "$@"
    printf -- '---\n\n# Rationale\n\nA fixture.\n\n# Required behaviour\n\nThe fixture holds.\n\n# Failure behaviour\n\nThe fixture is reported.\n\n# Verification\n\nThe entry runs it.\n'
  } > "$P/$f"
}

# the project's own check, declared as the policy declares any enforcement
mkdir -p scripts
printf '#!/bin/sh\nexit 0\n' > scripts/check-links; chmod +x scripts/check-links
sed -i.bak 's/^enforcement:$/enforcement:\
  - name: links-on-commit\
    path: scripts\/check-links\
    args: []\
    wired_by: manual/' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
grep -q 'name: links-on-commit' .ai/repo/policy.yaml || { echo "    the fixture could not declare the entry"; exit 1; }

prule links.v1.md project.links 'enforcement: links-on-commit'
git add -A && git commit -qm "a rule backed by the project's command"
expect_exit 0 "$MJ" rules list
expect_grep 'project\.links .* enforced by policy entry links-on-commit'
expect_no_grep 'project\.links .* no validator'
"$MJ" rules list --json > "$T/rules.json" 2>/dev/null || true
grep -q '"id":"project.links"[^}]*"mode":"entry"' "$T/rules.json" \
  || { echo "    the JSON listing does not report the entry mode:"; cat "$T/rules.json"; exit 1; }

"$MJ" doctor > "$T/doctor.out" 2>&1 || true
grep -q 'rule(s) enforced by a policy entry the policy declares' "$T/doctor.out" \
  || { echo "    doctor did not confirm the entry:"; grep -i 'rules' "$T/doctor.out"; exit 1; }

# a name the policy does not declare is enforcement nothing performs
prule links.v1.md project.links 'enforcement: links-on-push'
git add -A && git commit -qm "a rule naming an undeclared entry"
"$MJ" doctor > "$T/doctor.out" 2>&1 || true
grep -q "FAIL.*project.links.*names enforcement entry 'links-on-push'" "$T/doctor.out" \
  || { echo "    doctor did not refuse an undeclared entry:"; grep -i 'links' "$T/doctor.out"; exit 1; }

# one thing checks a rule
prule links.v1.md project.links 'validator: links' 'category: fixture' 'exit_code: 10' \
  'enforced_by: [doctor]' 'enforcement: links-on-commit'
expect_exit 10 "$MJ" rules list
expect_grep 'names a validator and an enforcement entry'
echo "ok: a project rule is enforced by the policy entry it names, and only by a declared one"

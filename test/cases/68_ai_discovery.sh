# Discovery of the AI layer is driven by the manifest and the declared sources, never by
# walking .ai/. A file nobody registered carries no authority, the local half is never a
# source, and the answer is the same twice.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
git add -A >/dev/null; git commit -qm install

# --- a registered rule source is discovered, in the rules class
"$MJ" knowledge sources --scope shared > shared.txt
grep -q '^rule .*\.ai/repo/rules/vendor/majordomus/rules/scope-integrity\.v1\.md$' shared.txt \
  || { echo "    a vendored rule was not discovered as a rule source"; exit 1; }
cat > .ai/repo/rules/project/example.v1.md <<'EOF'
---
id: project.example
version: 1
kind: rule
title: Example
description: An example project rule.
statement: Do the example thing.
status: active
class: advisory
depends_on: []
tags: [example]
---

# Rationale

Because.
EOF
git add -A >/dev/null; git commit -qm rule
"$MJ" knowledge sources --scope shared > shared.txt
grep -q '^rule .*\.ai/repo/rules/project/example\.v1\.md$' shared.txt || { echo "    a project rule was not discovered"; exit 1; }
"$MJ" rules list | grep -q '^project.example ' || { echo "    the project rule is not in the effective set"; exit 1; }

# --- an unregistered file under .ai/repo/ is not a source and not a rule, tracked or not
printf '# Not registered\n\nA rule-looking sentence: never do X.\n' > .ai/repo/notes.md
mkdir -p .ai/repo/rules/extra && cp .ai/repo/rules/project/example.v1.md .ai/repo/rules/extra/other.v1.md
sed -i.bak 's/^id: project.example$/id: project.other/' .ai/repo/rules/extra/other.v1.md; rm -f .ai/repo/rules/extra/other.v1.md.bak
git add -A >/dev/null; git commit -qm stray
"$MJ" knowledge sources --scope shared > shared.txt
expect_no_grep '\.ai/repo/notes\.md' shared.txt
"$MJ" rules list > rules.txt
expect_no_grep '^project\.other ' rules.txt
# the manifest is what registers a section: a section it does not name is not looked at
grep -q '^sections:' .ai/manifest.yaml
rm -rf .ai/repo/rules/extra .ai/repo/notes.md

# --- the local half is never a shared source, whatever it contains
"$MJ" start "work" --scope docs >/dev/null
printf 'progress\n' | "$MJ" checkpoint >/dev/null
printf '# looks like a rule\n' > .ai/local/rule-looking.md
"$MJ" knowledge sources --scope shared > shared.txt
expect_no_grep '\.ai/local/' shared.txt
"$MJ" knowledge sources --scope operational > op.txt
expect_no_grep 'rule-looking' op.txt
grep -q '\.ai/local/state/checkpoints/' op.txt || { echo "    the checkpoint was not discovered as an operational record"; exit 1; }

# --- deterministic: twice the same, and the order is the declared class order
"$MJ" knowledge sources > a.txt; "$MJ" knowledge sources > b.txt
cmp -s a.txt b.txt || { echo "    two runs disagreed"; exit 1; }
first="$(awk 'NR==1{print $1}' a.txt)"
[ "$first" = policy ] || { echo "    the first class reported is $first, expected policy (the declared order)"; exit 1; }
"$MJ" rules list > r1.txt; "$MJ" rules list > r2.txt
cmp -s r1.txt r2.txt || { echo "    the rule order differed between two runs"; exit 1; }

# --- a manifest section pointing nowhere is a doctor failure, not a silent empty answer
sed -i.bak 's#^  workflows: repo/workflows$#  workflows: repo/nowhere#' .ai/manifest.yaml; rm -f .ai/manifest.yaml.bak
rc=0; "$MJ" doctor > doctor.out 2>&1 || rc=$?
[ "$rc" != 0 ] || { echo "    doctor passed a manifest section that points nowhere"; exit 1; }
grep -qE "FAIL layout +\.ai/repo/nowhere — named by the manifest as section 'workflows' but absent" doctor.out \
  || { echo "    doctor did not name the absent section"; cat doctor.out; exit 1; }

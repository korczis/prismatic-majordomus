# majordomus-covers: doctor
# majordomus-negative: doctor
# claims: bootstrap-chain
# A region projection's host document is the adopter's own, and doctor's rule-corpus check
# reads only the generated region.
#
# An adopter that keeps its own agent catalogue in AGENTS.md — `### <agent>` headings, bold
# bullets — and lets Majordomus own a region inside it failed doctor with "carries a rule
# corpus of its own", although no generator wrote a line of that and no `update` could ever
# clear it. In a repository whose pre-commit hook is doctor that is a FAIL on every commit.
# The rule the check enforces (no rule lives only in a provider's file) is about generated
# content; the region is all of it.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
printf '#!/bin/sh\n%s doctor || exit $?\n' "$MJ" > .git/hooks/pre-commit
printf '#!/bin/sh\n%s finish --check || exit $?\n' "$MJ" > .git/hooks/pre-push
chmod +x .git/hooks/pre-commit .git/hooks/pre-push

sed 's/^    target: CLAUDE.md$/    target: CLAUDE.md\
    mode: region/' .ai/repo/policy.yaml > policy.new && mv policy.new .ai/repo/policy.yaml
grep -q '^    mode: region$' .ai/repo/policy.yaml || { echo "    the fixture policy did not take mode: region"; exit 1; }

# the host document: every shape the check looks for, all of it the adopter's own prose
cat > CLAUDE.md <<'MD'
# CLAUDE.md

## Our agents

### Rules

- **reviewer** reads diffs and reports.
- **investigator** works the OSINT adapters.

| profile | when |
|---|---|
| `routine` | small chores |

MD
expect_exit 0 "$MJ" update
grep -q 'majordomus:begin' CLAUDE.md || { echo "    update did not write a region into CLAUDE.md"; exit 1; }
grep -q '^- \*\*reviewer\*\*' CLAUDE.md || { echo "    update did not keep the host prose"; exit 1; }

expect_exit 0 "$MJ" doctor
expect_no_grep 'FAIL bootstrap +CLAUDE.md'
expect_grep 'OK   bootstrap .*carries no rule of its own'

# the same lines inside the region are generated content, and the check still holds there
awk '{ print } /majordomus:begin/ { print "### Rules"; print "- **always** run the suite." }' CLAUDE.md > c.new && mv c.new CLAUDE.md
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL bootstrap +CLAUDE.md — carries a rule corpus of its own \(a profile table, rule bullets or a rules section\) inside its generated region'

# a file-mode projection is generated content end to end, and is read end to end
"$MJ" update --force >/dev/null
expect_exit 0 "$MJ" doctor
printf '\n### Rules\n' >> AGENTS.md
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL bootstrap +AGENTS.md — carries a rule corpus of its own \(a profile table, rule bullets or a rules section\); rules live'

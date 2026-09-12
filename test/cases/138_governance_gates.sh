# majordomus-covers: rules
# majordomus-negative: rules
# The two gates that decide rules nothing could decide before, proved by mutation.
#
# `project.english-only` and `project.finding-carries-reproduce` were both blocking rules
# whose Verification section said "Review." — normative sentences with nothing behind them,
# sitting in rule-proof-baseline.txt next to rules that merely had not been got round to.
# Each now names an executable check, and a check is only worth the mutation it survives:
# a gate that has never been shown to go red is a gate nobody has reason to believe.
#
# Both are ratchets over a baseline, and both have the same three modes, so the shape of
# this case is the same twice: measure a tree, change one fact, watch the gate name it,
# undo, watch the gate go quiet.
#
# What it proves, in order:
#   1  english-only: a Czech letter in an authored file is named, with the file and the line
#   2  ...and a whole other script is too — the check is not a list of one language
#   3  ...and a declared proper noun is stripped, so English prose naming one is English
#   4  ...and a declared fixture is exempt, but only with its reason: a fixture entry with
#      no reason makes the check refuse to run rather than quietly pass
#   5  ...and the generated trees are out of scope, because a projection carrying a foreign
#      word carries it from its source, and the source is where the finding belongs
#   6  finding-reproduce: a reporting call with no reproduce command is named, with its line
#   7  ...and a forwarding call is not, because its count belongs to whoever called it
#   8  ...and the helper named inside a string is prose about the helper, not a call to it —
#      without this the check reports itself, which is how it was first written
#   9  ...and a call the reader cannot parse is `unreadable` and counted, never passed
#  10  both: the ratchet excuses recorded debt, --strict ignores the baseline, and clearing
#      debt is reported so a baseline cannot outlive what it excused
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
EN="$ROOT/scripts/ci/english-only-check"
FR="$ROOT/scripts/ci/finding-reproduce-check"
[ -x "$EN" ] || { echo "    scripts/ci/english-only-check is not executable"; exit 1; }
[ -x "$FR" ] || { echo "    scripts/ci/finding-reproduce-check is not executable"; exit 1; }
# both gates measure whatever tree MJ_ROOT names; the fixture is this one
export MJ_ROOT="$PWD"
ENB=".ai/repo/english-only-baseline.txt"
FRB=".ai/repo/finding-reproduce-baseline.txt"

# The fixture is a git repository: english-only-check reads `git ls-files`, because what it
# measures is what a person committed and not what a build left lying about.
mkdir -p docs lib scripts/ci
printf '# Alpha\n\nOne sentence of English prose.\n' > docs/ALPHA.md
printf '#!/usr/bin/env bash\n# a library\n' > lib/alpha.sh
git add -A >/dev/null && git commit -qm fixture

# The allow list lives beside the check, not in the tree under measurement, so the fixture
# cannot declare its own exemptions. That is deliberate: an exemption is a property of this
# repository's vocabulary, and a fixture that could mint one would prove nothing about it.
expect_exit 0 "$EN" --strict
expect_grep 'every authored file is spelled in English'

# ---------------------------------------------------------------- 1. the alphabet
# Proves `authored-files-are-english`: sections 1 to 5 are the whole claim — a foreign
# letter in an authored file is named with its file and line, a declared proper noun is
# English prose, and a declared fixture is exempt only when it carries its reason.
# The mutation the check exists for. A paragraph in the language this repository is worked
# in reads as perfectly fine to whoever wrote it and as noise to everyone else, and review
# is what was catching it, which is to say nothing was.
printf '# Alpha\n\nJedna věta, která není anglicky.\n' > docs/ALPHA.md
git add -A >/dev/null && git commit -qm czech
expect_exit 10 "$EN" --strict
expect_grep 'FAIL english-only docs/ALPHA\.md:3'
expect_grep 'which does not occur in English'

# 2. and it is not a list of one language: a whole script is unambiguous on sight
printf '# Alpha\n\nЭто не по-английски.\n' > docs/ALPHA.md
git add -A >/dev/null && git commit -qm cyrillic
expect_exit 10 "$EN" --strict
expect_grep 'FAIL english-only docs/ALPHA\.md'

# ...and typographic punctuation is not a finding. This repository's prose uses the em dash
# and the curly apostrophe throughout; a check that flagged them would be turned off within
# the hour, and a check that is turned off decides nothing.
printf '# Alpha\n\nEnglish prose — with an em dash, an ellipsis … and a curly apostrophe.\n' > docs/ALPHA.md
git add -A >/dev/null && git commit -qm punctuation
expect_exit 0 "$EN" --strict

# ---------------------------------------------------------------- 5. what is out of scope
# A generated tree is not authored. A projection carrying a foreign word carries it from its
# source, and reporting it here would send a reader to fix the output of a generator.
mkdir -p site/data/generated docs/generated
printf '{"note":"Это сгенерировано"}\n' > site/data/generated/x.json
printf '# Generated\n\nJedna věta.\n' > docs/generated/x.md
git add -A >/dev/null && git commit -qm generated
expect_exit 0 "$EN" --strict
expect_no_grep 'generated'

# ...and an untracked file is not authored either: the subject is what was committed.
printf '# Scratch\n\nJedna věta.\n' > docs/SCRATCH.md
expect_exit 0 "$EN" --strict
rm -f docs/SCRATCH.md

# ---------------------------------------------------------------- the ratchet, on one gate
# Debt that is recorded blocks nobody; --strict ignores the baseline, which is how the debt
# is measured rather than kept; and clearing it is reported, so a baseline cannot quietly
# outlive what it excused.
printf '# Alpha\n\nJedna věta, která není anglicky.\n' > docs/ALPHA.md
git add -A >/dev/null && git commit -qm debt
expect_exit 10 "$EN"
expect_grep 'new debt, not in'
expect_exit 0 "$EN" --write-baseline
expect_exit 0 "$EN"
expect_grep 'no new debt'
expect_exit 10 "$EN" --strict
grep -q '^docs/ALPHA.md$' "$ENB" || { echo "    the baseline does not name the file it excused"; exit 1; }
printf '# Alpha\n\nOne sentence of English prose.\n' > docs/ALPHA.md
git add -A >/dev/null && git commit -qm cleared
expect_exit 0 "$EN"
expect_grep 'debt cleared, run --write-baseline'
expect_grep 'docs/ALPHA\.md'
expect_exit 0 "$EN" --write-baseline

# ---------------------------------------------------------------- 6. the reproduce command
# Proves `a-finding-names-how-to-see-it-again`: every finding either carries the command
# that reproduces it or the gate names the finding that does not.
# `mj_finding` prints the finding without a reproduce command when none is given, so a caller
# that forgot one produced a finding that reads exactly like a finding that could not have
# one. Nothing could tell them apart, so nothing did.
cat > lib/alpha.sh <<'BASH'
#!/usr/bin/env bash
alpha_one() { mj_fail alpha "subject" "message" "the command that shows it again"; }
BASH
git add -A >/dev/null && git commit -qm reproduce
expect_exit 0 "$FR" --strict
expect_grep 'every finding names how to reproduce it'

cat > lib/alpha.sh <<'BASH'
#!/usr/bin/env bash
alpha_one() { mj_fail alpha "subject" "message"; }
BASH
expect_exit 10 "$FR" --strict
expect_grep 'FAIL finding-reproduce lib/alpha\.sh:2'
expect_grep 'names no command to reproduce it'

# 7. a forwarding call passes whatever it was given; the count is the caller's, and the
# caller is measured where it is written
cat > lib/alpha.sh <<'BASH'
#!/usr/bin/env bash
alpha_fwd() { mj_fail "$@"; }
alpha_two() { mj_warn beta "subject" "message" "reproduce me"; }
BASH
expect_exit 0 "$FR" --strict

# 8. the helper named inside a string is prose about the helper, not a call to it. Without
# this the check reports itself — which is how it was first written, and the reason the scan
# walks each line outside its quoted runs rather than matching the name anywhere.
cat > lib/alpha.sh <<'BASH'
#!/usr/bin/env bash
alpha_doc() { echo "pass the command to mj_fail as its fifth argument"; }
# mj_fail cat subj msg   <- a comment about the helper is not a call either
BASH
expect_exit 0 "$FR" --strict

# 9. a call the reader cannot parse is named as unreadable and counted. An unparsed call
# that read as compliant would be this very defect, one level up.
cat > lib/alpha.sh <<'BASH'
#!/usr/bin/env bash
alpha_bad() { mj_fail alpha "an unterminated quote; }
BASH
expect_exit 10 "$FR" --strict
expect_grep 'cannot parse; put the call on one line'

# ...and the doctrine helper is measured with the other three: it takes the same four
# arguments and decides the level from the rule's class, so a violation there is the same
# violation.
cat > lib/alpha.sh <<'BASH'
#!/usr/bin/env bash
alpha_doc() { mj_doctrine_fail alpha "subject" "message"; }
BASH
expect_exit 10 "$FR" --strict
expect_grep 'FAIL finding-reproduce lib/alpha\.sh:2'

# ...and a statement is not a finding: there is nothing to reproduce about a line saying a
# thing is fine, and requiring one would make every OK line carry a command nobody runs.
cat > lib/alpha.sh <<'BASH'
#!/usr/bin/env bash
alpha_ok() { mj_ok alpha "subject" "message"; mj_info alpha "subject" "message"; }
BASH
expect_exit 0 "$FR" --strict

# 10. the same ratchet on the second gate, so neither is a special case
cat > lib/alpha.sh <<'BASH'
#!/usr/bin/env bash
alpha_one() { mj_fail alpha "subject" "message"; }
BASH
expect_exit 10 "$FR"
expect_grep 'new debt, not in'
expect_exit 0 "$FR" --write-baseline
expect_exit 0 "$FR"
expect_grep 'no new debt'
grep -q '^lib/alpha.sh$' "$FRB" || { echo "    the baseline does not name the file it excused"; exit 1; }
expect_exit 10 "$FR" --strict

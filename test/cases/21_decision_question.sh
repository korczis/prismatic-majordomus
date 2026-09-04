# Decisions and open questions: the two stores the finish contract reads. Both must be
# written by a command, validated by the gates, and impossible to corrupt silently.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base

# ---------------------------------------------------------------- decisions
expect_exit 2 "$MJ" decision
expect_grep 'usage: majordomus decision add'
expect_exit 2 "$MJ" decision nonsense
expect_grep "unknown subcommand 'nonsense'"
expect_exit 2 "$MJ" decision add "no reason given"
expect_grep 'why is required'
expect_exit 0 "$MJ" decision list
expect_grep '^\(none\)$'          # the commented template is not an entry

"$MJ" start "t1" --scope lib >/dev/null
id=$(sed -n 's/^id: //p' .majordomus/state/current.yaml)

expect_exit 0 "$MJ" decision add "Normalise the callback URI before comparing state" \
  --why "the mismatch is a trailing slash, not a forged state parameter" \
  --rejected "relaxing the comparison" --evidence "test/auth_test:41"
expect_grep '^recorded: Normalise'
expect_grep "^Task: $id\$" .majordomus/state/decisions.md
expect_grep '^Head: '"$(git rev-parse HEAD)"'$' .majordomus/state/decisions.md
expect_grep '^Rejected: relaxing the comparison$' .majordomus/state/decisions.md
expect_grep '"event":"decision.recorded"' .majordomus/state/ledger.jsonl

# multi-line text is refused: one entry is one line per field, and the gates grep for them
expect_exit 2 bash -c "'$MJ' decision add \"$(printf 'two\nlines')\" --why x"
expect_grep 'single-line'
# ... and the guard actually rejects, rather than matching everything or nothing
expect_exit 0 "$MJ" decision add "single line is accepted" --why "proves the guard is not inverted"

# list is newest first, --task filters, --limit caps
expect_exit 0 "$MJ" decision list
expect_grep '^## [0-9-]+ — single line is accepted'
expect_exit 0 "$MJ" decision list --limit 1
[ "$(printf '%s\n' "$LAST_OUT" | grep -c '^## ')" = 1 ]
expect_exit 0 "$MJ" decision list --task nosuch
expect_grep '^\(none\)$'
expect_exit 2 "$MJ" decision list --limit x

# show finds one entry; a miss is exit 12, not a silent empty success
expect_exit 0 "$MJ" decision show "callback URI"
expect_grep '^Why: the mismatch'
expect_no_grep 'single line is accepted'
expect_exit 12 "$MJ" decision show "no such decision"

# supersession points at something real, and never edits the superseded entry
expect_exit 2 "$MJ" decision add "replacement" --why w --supersedes "a decision nobody made"
expect_grep 'matches no recorded decision'
expect_exit 0 "$MJ" decision add "Compare the raw callback URI after all" --why "normalising hid a real mismatch" \
  --supersedes "Normalise the callback URI before comparing state"
expect_grep '^Supersedes: Normalise the callback URI' .majordomus/state/decisions.md
[ "$(grep -c '^## 20' .majordomus/state/decisions.md)" = 3 ]   # nothing was rewritten

# a malformed entry is reported by check as a warning: nothing will ever find it
printf '\n## 2026-01-01 — hand written, no fields\n' >> .majordomus/state/decisions.md
expect_exit 0 "$MJ" check
expect_grep 'WARN records +decisions.md .* lacks Task, Head or Why'

# ---------------------------------------------------------------- open questions
expect_exit 0 "$MJ" question list
expect_grep 'no open questions'
expect_exit 2 "$MJ" question add "$(printf 'two\nlines')"
expect_grep 'single-line'
expect_exit 2 "$MJ" question add "contains — the field separator"
expect_grep 'must not contain'

expect_exit 0 "$MJ" question add "Does the legacy mobile callback need the old URI form?"
expect_grep "^opened for $id"
expect_grep "^- \[unresolved\] $id — Does the legacy" .majordomus/state/open-questions.md
expect_grep '"event":"question.opened"' .majordomus/state/ledger.jsonl
expect_exit 0 "$MJ" question add "Which environments still run the old client?"
expect_exit 0 "$MJ" question list
expect_grep '^1  \[unresolved\]'
expect_grep '^2  \[unresolved\]'

# an unresolved question blocks check and a completed finish
expect_exit 10 "$MJ" check
expect_grep 'FAIL blockers'
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null
echo b >> lib/a
expect_exit 10 "$MJ" finish --outcome completed --verify-command true
expect_grep 'FAIL blockers'

# resolving requires an answer, refuses an ambiguous or absent selector
expect_exit 2 "$MJ" question resolve 1
expect_grep 'answer is required'
expect_exit 12 "$MJ" question resolve 9 --answer x
expect_grep 'matches no unresolved question'
expect_exit 0 "$MJ" question resolve 1 --answer "no; it was retired in 4.2"
expect_grep "^resolved for $id"
expect_grep "^- \[resolved [0-9-]+\] $id — Does the legacy.*— no; it was retired in 4.2$" .majordomus/state/open-questions.md
expect_grep '"event":"question.resolved".*"answer":"no; it was retired in 4.2"' .majordomus/state/ledger.jsonl
# text selectors work too
expect_exit 0 "$MJ" question resolve "old client" --answer "staging only"
expect_exit 0 "$MJ" question list
expect_grep 'no open questions'
expect_exit 0 "$MJ" question list --all
expect_grep 'resolved'

# with both resolved, check passes and finish accepts
expect_exit 0 "$MJ" check
expect_grep 'OK +blockers'
expect_exit 0 "$MJ" finish --outcome completed --verify-command true

# a malformed unresolved entry FAILS: a gate that cannot read an entry can be bypassed
git add -A && git commit -qm t1
"$MJ" start "t2" --scope lib >/dev/null
printf -- '- [unresolved] this line has no separator or date\n' >> .majordomus/state/open-questions.md
expect_exit 10 "$MJ" check
expect_grep 'FAIL blockers +open-questions.md .* do not parse'
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL records +open-questions.md'
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT records +open-questions.md'

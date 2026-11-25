# majordomus-covers: question doctor
# majordomus-negative: question doctor
# An owner's answer is a tracked, verbatim record with its provenance (I4669, rule
# majordomus.owner-answers-are-recorded). On 2026-11-25 a lead session relayed three owner
# answers to its workers. The relay said that two English questions had been translated, and
# it changed a double space in a free-text answer to a single one. So the record is written
# from the provider's transcript where there is one, held byte for byte with digests the
# validator recomputes, and it says how strong its provenance is when the text was typed.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add . && git commit -qm base

# A transcript in the provider's shape: a question-tool call, its result carrying two labels
# and free text with a double space and a trailing one, and one plain chat turn.
cat > "$T/session.jsonl" <<'J'
{"type":"assistant","sessionId":"sess-4669","timestamp":"2026-11-25T05:06:23.177Z","message":{"content":[{"type":"tool_use","id":"toolu_4669","name":"AskUserQuestion","input":{"questions":[{"question":"Which lessons should I implement?","header":"H","multiSelect":true,"options":[{"label":"Slots","description":"build slots"},{"label":"Answers","description":"owner answers"},{"label":"Gates"}]},{"question":"Second?","multiSelect":false,"options":[{"label":"Yes"},{"label":"No"}]}]}}]}}
{"type":"assistant","sessionId":"sess-4669","timestamp":"2026-11-25T05:07:00.000Z","message":{"content":[{"type":"tool_use","id":"toolu_open","name":"AskUserQuestion","input":{"questions":[{"question":"Unanswered?","options":[{"label":"A"}]}]}}]}}
{"type":"user","sessionId":"sess-4669","timestamp":"2026-11-25T05:53:09.117Z","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_4669","content":"The user answered: \"Which lessons should I implement?\"=\"Slots, Answers, \"consult  others \"\", \"Second?\"=\"No\". Read the answers carefully."}]}}
{"type":"user","sessionId":"sess-4669","uuid":"msg-1","timestamp":"2026-11-25T06:06:37.664Z","message":{"content":"can you  improve it?"}}
J
TR="$T/session.jsonl"

# ---------------------------------------------------------------- usage and refusals
expect_exit 2 "$MJ" question
expect_grep 'question record --slug'
expect_exit 2 "$MJ" question nonsense
expect_grep 'add\|resolve\|list\|record\|answers\|answer'
expect_exit 2 "$MJ" question record --slug
expect_grep 'needs a value'
expect_exit 2 "$MJ" question record --bogus x
expect_grep 'unknown argument'
expect_exit 2 "$MJ" question record --slug a --transcript "$TR"
expect_grep 'needs --locator'
expect_exit 2 "$MJ" question record --slug a --locator x
expect_grep 'needs --transcript'
expect_exit 12 "$MJ" question record --slug a --transcript "$T/none.jsonl" --locator 'toolu_4669#0'
expect_grep 'no transcript'
expect_exit 2 "$MJ" question record --slug a --transcript "$TR" --locator 'toolu_4669#x'
expect_grep 'must be a number'
expect_exit 12 "$MJ" question record --slug a --transcript "$TR" --locator 'toolu_none#0' --selected A
expect_grep 'no tool use toolu_none'
expect_exit 12 "$MJ" question record --slug a --transcript "$TR" --locator 'toolu_open#0' --selected A
expect_grep 'has no result'
expect_exit 12 "$MJ" question record --slug a --transcript "$TR" --locator 'toolu_4669#7' --selected Slots
expect_grep 'does not carry the question'
expect_exit 12 "$MJ" question record --slug a --transcript "$TR" --locator msg-none
expect_grep 'no message msg-none'
# the answer is checked against the result, never parsed out of it, and never altered
expect_exit 15 "$MJ" question record --slug a --transcript "$TR" --locator 'toolu_4669#0'
expect_grep 'is required: the answer is checked'
expect_exit 15 "$MJ" question record --slug a --transcript "$TR" --locator 'toolu_4669#0' --selected Gates
expect_grep 'label "Gates" is not in the answer'
expect_exit 15 "$MJ" question record --slug a --transcript "$TR" --locator 'toolu_4669#0' --selected Slots --free-text 'consult others '
expect_grep 'free text is not in the answer the transcript holds byte for byte'
expect_exit 15 "$MJ" question record --slug a --transcript "$TR" --locator 'toolu_4669#0' --selected Slots --question 'Which lessons?'
expect_grep 'question differs from the transcript'
expect_exit 15 "$MJ" question record --slug a --transcript "$TR" --locator 'toolu_4669#0' --selected Slots --option Slots
expect_grep 'options differ from the transcript'
expect_exit 15 "$MJ" question record --slug a --transcript "$TR" --locator msg-1 --free-text 'can you improve it?'
expect_grep 'free text differs from the transcript message'
[ ! -d .ai/repo/answers ] || { echo "    a refused record left a directory behind"; exit 1; }

# ---------------------------------------------------------------- a record from the transcript
expect_exit 0 "$MJ" question record --slug which-lessons --transcript "$TR" --locator 'toolu_4669#0' \
  --selected Slots --selected Answers --free-text 'consult  others ' --provider claude-code \
  --settles issue:I4669 --settles 'pr:owner/repo#12' \
  --translation-language en --translation-answer 'consult the others'
expect_grep '^recorded A-20261125-which-lessons \(transcript-checked\)'
expect_grep 'cite it as owner-answer:A-20261125-which-lessons'
F=.ai/repo/answers/A-20261125-which-lessons.yaml
expect_file "$F"
expect_file .ai/repo/answers/README.md
expect_grep '^question: "Which lessons should I implement\?"$' "$F"
expect_grep '^free_text: "consult  others "$' "$F"
expect_grep '^  session: "sess-4669"$' "$F"
expect_grep '^  session_source: "transcript"$' "$F"
expect_grep '^  at: "2026-11-25T05:06:23.177Z"$' "$F"
expect_grep '^answered_at: "2026-11-25T05:53:09.117Z"$' "$F"
expect_grep '^  - label: "Gates"$' "$F"
expect_grep '^  locator: "toolu_4669#0"$' "$F"
expect_grep '^  transcript: "session.jsonl"$' "$F"
expect_grep '^  by: ".+ <.+>"$' "$F"
expect_no_grep "$T" "$F"          # the transcript's path is never written, only its name
sha="$( { sed -n 1p "$TR"; sed -n 3p "$TR"; } | { sha256sum 2>/dev/null || shasum -a 256; } | cut -c1-64)"
expect_grep "^  sha256: \"$sha\"\$" "$F"
# the registry reads the kind from this tree's distribution (its share/, whichever executable
# answers) and finds nothing wrong with the record
RB="$(rust_bin)" || rust_bin_exit $?
git add .ai/repo/answers && git commit -qm answer
MAJORDOMUS_SHARE="$ROOT/share" run_quiet "$T/kinds.err" "$RB" entity kinds > "$T/kinds.out"
grep -q 'owner-answer *1 ' "$T/kinds.out" || { echo "    the registry does not list the owner-answer record"; cat "$T/kinds.out"; exit 1; }
if grep -q 'schema_violation' "$T/kinds.err"; then echo "    the record breaks its own schema"; cat "$T/kinds.err"; exit 1; fi

# a single-select question of the same call, and a chat turn whose text is the answer
expect_exit 15 "$MJ" question record --slug second --transcript "$TR" --locator 'toolu_4669#1' --selected Yes
expect_grep 'label "Yes" is not in the answer'
expect_exit 15 "$MJ" question record --slug single --question 'Q?' --option Yes --option No --selected Yes --selected No --asked-in s-2
expect_grep 'several labels selected on a single-select'
expect_exit 0 "$MJ" question record --slug second --transcript "$TR" --locator 'toolu_4669#1' --selected No
expect_exit 0 "$MJ" question record --slug improve --transcript "$TR" --locator msg-1 --settles 'other:the generic tool evolves'
expect_grep 'A-20261125-improve \(transcript-checked\)'
expect_grep '^free_text: "can you  improve it\?"$' .ai/repo/answers/A-20261125-improve.yaml
expect_grep '^channel: "chat"$' .ai/repo/answers/A-20261125-improve.yaml
expect_grep '^  - "other:the generic tool evolves"$' .ai/repo/answers/A-20261125-improve.yaml

# ---------------------------------------------------------------- typed records say so
expect_exit 15 "$MJ" question record --slug which-lessons --transcript "$TR" --locator 'toolu_4669#0' --selected Slots
expect_grep 'exists; a record is never rewritten'
expect_exit 2 bash -c "unset MAJORDOMUS_PROVIDER_SESSION; '$MJ' question record --slug typed --question 'Q?' --option A --selected A"
expect_grep 'no asking session'
expect_exit 15 "$MJ" question record --slug typed --question 'Q?' --option A --selected B --asked-in s-2
expect_grep '"B" is not one of the offered options'
expect_exit 15 "$MJ" question record --slug typed --question 'Q?' --channel question-tool --free-text x --asked-in s-2
expect_grep 'carries the options the tool offered'
expect_exit 15 "$MJ" question record --slug typed --option A --selected A --asked-in s-2
expect_grep 'the question is required'
expect_exit 15 "$MJ" question record --slug typed --question 'Q?' --channel terminal --asked-in s-2
expect_grep 'an empty answer'
expect_exit 15 "$MJ" question record --slug typed --question 'Q?' --channel pigeon --free-text x --asked-in s-2
expect_grep 'unknown channel pigeon'
expect_exit 15 "$MJ" question record --slug 'Bad Slug' --question 'Q?' --channel terminal --free-text x --asked-in s-2
expect_grep 'slug is required'
expect_exit 2 "$MJ" question record --slug typed --question 'Q?' --channel terminal --free-text x --asked-in s-2 --settles nowhere
expect_grep 'settles nowhere'
expect_exit 2 "$MJ" question record --slug typed --question 'Q?' --channel terminal --free-text x --asked-in s-2 --supersedes A-20991231-none
expect_grep 'names no record'
expect_exit 12 "$MJ" question record --slug typed --from "$T/none.json"
expect_exit 2 bash -c "printf '[1]' | '$MJ' question record --slug typed --from -"
expect_grep 'one JSON object'
# a document carries multi-line verbatim text without shell quoting
printf '{"question":"line one\\n  line two","channel":"terminal","free_text":"yes","asked_session":"s-3","answered_at":"2026-11-26T10:00:00Z","supersedes":"A-20261125-second"}' > "$T/typed.json"
expect_exit 0 "$MJ" question record --slug typed --from "$T/typed.json" --multi-select --asked-at 2026-11-26T09:59:00Z
expect_grep 'A-20261126-typed \(typed\)'
expect_grep '^question: "line one\\n  line two"$' .ai/repo/answers/A-20261126-typed.yaml
expect_grep '^  session_source: "declared"$' .ai/repo/answers/A-20261126-typed.yaml
expect_grep '^supersedes: "A-20261125-second"$' .ai/repo/answers/A-20261126-typed.yaml
expect_exit 0 env MAJORDOMUS_PROVIDER_SESSION=env-sess "$MJ" question record --slug from-env --question 'Q?' --channel forge --free-text ok --answered-at 2026-11-27T00:00:00Z
expect_grep '^  session: "env-sess"$' .ai/repo/answers/A-20261127-from-env.yaml
expect_grep '^  session_source: "environment"$' .ai/repo/answers/A-20261127-from-env.yaml

# ---------------------------------------------------------------- reading them back
expect_exit 0 "$MJ" question answers
expect_grep '^A-20261125-which-lessons  2026-11-25T05:53:09.117Z  transcript-checked  Which lessons should I implement\?'
expect_grep '^A-20261125-improve .* can you  improve it\?'
expect_exit 0 "$MJ" question answers --settles issue:I4669
expect_grep 'which-lessons'
expect_no_grep 'improve'
expect_exit 0 "$MJ" question answers --settles issue:none
expect_grep '^no owner answers settle issue:none$'
expect_exit 2 "$MJ" question answers --settles
expect_exit 2 "$MJ" question answers --bogus
expect_exit 2 "$MJ" question answer
expect_exit 12 "$MJ" question answer A-20991231-none
expect_exit 0 "$MJ" question answer A-20261125-which-lessons
expect_grep 'checked the question byte for byte against the provider transcript session.jsonl \(toolu_4669#0\)'
expect_grep 'asked by session sess-4669 \(transcript\)'
expect_exit 0 "$MJ" question answer A-20261126-typed
expect_grep 'typed by the recorder, unverified'
cp "$F" .ai/repo/answers/A-20261128-payload.yaml
sed -i.bak -e 's/^id: .*/id: "A-20261128-payload"/' -e 's/^text_source: .*/text_source: "provider-payload"/' .ai/repo/answers/A-20261128-payload.yaml
rm -f .ai/repo/answers/*.bak
expect_exit 0 "$MJ" question answer A-20261128-payload
expect_grep "provider's own hook payload"

# ---------------------------------------------------------------- a blocking question closes on a record
"$MJ" session start --provider-session sess-4669 >/dev/null
"$MJ" start "t4669" --scope lib >/dev/null
"$MJ" question add "Which lessons?" >/dev/null
expect_exit 12 "$MJ" question resolve 1 --owner-answer A-20991231-none
expect_grep 'names no record'
expect_exit 2 "$MJ" question resolve 1 --owner-answer
expect_exit 0 "$MJ" question resolve 1 --owner-answer A-20261125-which-lessons
expect_grep 'owner-answer:A-20261125-which-lessons' .ai/local/state/open-questions.md

# ---------------------------------------------------------------- the validator
# The rule is advisory: its findings are WARN and never change an exit code, so the case reads
# what the doctrine said rather than how the command (which judges a fixture without hooks) ended.
doctrine() { LAST_OUT="$("$MJ" "$1" 2>&1)" || true; }
git add .ai/repo && git commit -qm answers
doctrine doctor
expect_grep 'OK +records +owner answers — 6 record\(s\) hold the text their digests name'
mkdir -p .ai/repo/knowledge/curated
printf -- '---\nschema: knowledge/v1\n---\n# Owner decision\n\nSettled by owner-answer:A-20261125-which-lessons and owner-answer:A-20991231-gone.\n' > .ai/repo/knowledge/curated/cites.md
git add .ai/repo/knowledge && git commit -qm cites
doctrine check
expect_grep 'WARN +records +.ai/repo/knowledge/curated/cites.md — cites owner-answer:A-20991231-gone, which names no record'
expect_no_grep 'cites owner-answer:A-20261125-which-lessons'
# a paraphrase put over the verbatim text after the fact is refused, and so is a forged label
sed -i.bak 's/^free_text: .*/free_text: "consult others"/' "$F"
sed -i.bak 's/^  - "No"$/  - "Maybe"/' .ai/repo/answers/A-20261125-second.yaml
sed -i.bak 's/^question: .*/question: "Can you?"/' .ai/repo/answers/A-20261127-from-env.yaml
sed -i.bak 's/^id: .*/id: "A-20261125-other"/' .ai/repo/answers/A-20261125-improve.yaml
sed -i.bak '/^  sha256: /d' .ai/repo/answers/A-20261128-payload.yaml
sed -i.bak 's/^text_source: .*/text_source: "transcript-checked"/' .ai/repo/answers/A-20261128-payload.yaml
printf 'not: [a, record\n' > .ai/repo/answers/A-20261129-broken.yaml
rm -f .ai/repo/answers/*.bak
doctrine check
expect_grep 'WARN +records +.ai/repo/answers/A-20261125-which-lessons.yaml — the answer is not the text its digest was taken of'
expect_grep 'WARN +records +.ai/repo/answers/A-20261125-second.yaml — .*(not the text its digest|selects a label that was not offered)'
expect_grep 'WARN +records +.ai/repo/answers/A-20261127-from-env.yaml — the question is not the text its digest was taken of'
expect_grep 'WARN +records +.ai/repo/answers/A-20261125-improve.yaml — its id is not its file name'
expect_grep 'WARN +records +.ai/repo/answers/A-20261128-payload.yaml — it says transcript-checked without a locator and a digest'
expect_grep 'WARN +records +.ai/repo/answers/A-20261129-broken.yaml — it is not in the shape question record writes'
git checkout -q -- .ai/repo/answers
rm -f .ai/repo/answers/A-20261129-broken.yaml
# negative control: the label check alone, with digests left consistent
python_free_label() { sed 's/^  - "No"$/  - "Maybe"/' "$1"; }
python_free_label .ai/repo/answers/A-20261125-second.yaml > "$T/second.yaml" && mv "$T/second.yaml" .ai/repo/answers/A-20261125-second.yaml
d="$(printf '{"selected":["Maybe"],"free_text":""}' | { sha256sum 2>/dev/null || shasum -a 256; } | cut -c1-64)"
sed -i.bak "s/^  answer: .*/  answer: \"$d\"/" .ai/repo/answers/A-20261125-second.yaml; rm -f .ai/repo/answers/*.bak
doctrine check
expect_grep 'A-20261125-second.yaml — it selects a label that was not offered'

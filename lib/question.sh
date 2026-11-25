#!/usr/bin/env bash
# shellcheck disable=SC2034  # MJ_DOCTRINE_SKIPPED is read by the dispatcher in doctrine.sh
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_question:-}" ] && return 0 || MJ_LIB_question=1
# question — open questions as explicit state instead of prose buried in a handover.
#
# One mutable index: .ai/local/state/open-questions.md. Resolving edits its line, because
# an index of what is still open must not accumulate; the append-only record of opening and
# resolving lives in the ledger. Any unresolved entry for the active task blocks
# `finish --outcome completed`, which is why this file has a machine-written line format.

mj_cmd_question() {
  local sub="${1:-}"; [ $# -gt 0 ] && shift
  case "$sub" in
    add) mj_question_add "$@" ;;
    resolve) mj_question_resolve "$@" ;;
    list) mj_question_list "$@" ;;
    record) mj_answer_record "$@" ;;
    answers) mj_answer_list "$@" ;;
    answer) mj_answer_show "$@" ;;
    --help|-h|"") cat <<H
usage: majordomus question add "<question>"
       majordomus question resolve <n|"<text>"> (--answer "<answer>" | --owner-answer <id>)
       majordomus question list [--all] [--task <id>]
       majordomus question record --slug <slug> [--from <file.json|->]
                                  [--transcript <file.jsonl> --locator <tool-use id>#<n>|<message id>]
                                  [--question "<text>"] [--option "<label>"]... [--selected "<label>"]...
                                  [--free-text "<text>"] [--channel <c>] [--provider <p>]
                                  [--asked-in <session>] [--asked-at <time>] [--answered-at <time>]
                                  [--settles <kind>:<ref>]... [--supersedes <id>]
                                  [--translation-language <l> --translation-answer "<text>"]
       majordomus question answers [--settles <ref>]
       majordomus question answer <id>
  add       appends "- [unresolved] <task id> — <question> (<date>)" to open-questions.md
  resolve   rewrites that one line to "[resolved <date>]" with the answer; n is the number
            printed by \`question list\` for the active task; --owner-answer resolves it with
            a tracked owner-answer record, refused when the id names none
  list      every unresolved entry; --task narrows to one task, --all includes resolved
  record    writes one owner's answer, verbatim, as a tracked record .ai/repo/answers/<id>.yaml
            (kind owner-answer, schema owner-answer/v1). With --transcript the question, the
            options, the session and both times are read from the provider's transcript and the
            selected labels and free text are checked against its result (text_source
            transcript-checked); without it the text is typed and the record says so. The
            recording session, time, git identity and both sha256 digests are computed
  answers   every tracked owner-answer record, one line each; --settles narrows to one ref
  answer    one record as committed, with how strong its provenance is in words
  every unresolved entry on this branch refuses \`finish --outcome completed\`
H
      [ "$sub" = "" ] && return "$MJ_EX_USAGE"; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "question: unknown subcommand '$sub' (add|resolve|list|record|answers|answer)" ;;
  esac
}

mj_question_file() { printf '%s' "$MJ_STATE_DIR/open-questions.md"; }
# The store is checkout-local and absent on a fresh clone. A writer seeds it from the
# template; a reader treats absence as "no questions", which is the true answer.
mj_question_require() {
  mj_require_installed
  MJ_Q="$(mj_question_file)"
  [ -f "$MJ_Q" ] || { mkdir -p "$MJ_STATE_DIR"; cp "$MJ_SKELETON_DIR/templates/open-questions.md" "$MJ_Q"; }
}
mj_question_require_read() {
  mj_require_installed
  MJ_Q="$(mj_question_file)"
}

mj_question_add() {
  local text=""
  while [ $# -gt 0 ]; do case "$1" in
    -*) mj_die "$MJ_EX_USAGE" "question add: unknown option $1" ;;
    *) [ -z "$text" ] || mj_die "$MJ_EX_USAGE" "question add: the question must be one argument (quote it)"; text="$1"; shift ;;
  esac; done
  [ -n "$text" ] || mj_die "$MJ_EX_USAGE" "question add: the question text is required"
  mj_is_multiline "$text" && mj_die "$MJ_EX_USAGE" "question add: the question must be single-line" 
  case "$text" in *" — "*) mj_die "$MJ_EX_USAGE" "question add: the question must not contain ' — '; it separates the fields" ;; esac
  mj_question_require
  # A question is opened against a task: without one there is nothing for it to block (ADR
  # 0164 changes only who it is attributed to, "none" when the active task is another live
  # session's).
  mj_load_current || mj_die "$MJ_EX_MISSING" "no active task; a question is opened against a task"
  mj_record_attribution
  local id="${MJ_RECORD_TASK:-none}" fields=""
  [ -z "$MJ_RECORD_TASK" ] || fields="\"task_id\":\"$MJ_RECORD_TASK\","
  printf -- '- [unresolved] %s — %s (%s)\n' "$id" "$text" "$(date -u +%Y-%m-%d)" >> "$MJ_Q"
  mj_ledger_append question.opened "${fields}\"question\":\"$(mj_json_esc "$text")\"" "$MJ_RECORD_SESSION"
  printf 'opened for %s: %s\n' "$id" "$text"
}

# unresolved lines for a task, in file order
mj_question_unresolved() { grep -nE "^- \[unresolved\] $1 " "$2" 2>/dev/null || true; }

# Every unresolved entry, whoever opened it, with line numbers. A question refuses a
# completed finish anywhere on this branch — the store is tracked, so git is what scopes
# it — and this is the list that does the refusing. The file's own template lives in an
# HTML comment and is not an entry; a fresh install ships one, so a scan that did not skip
# it would block the first completed finish anybody attempted.
mj_question_unresolved_any() {
  [ -f "$1" ] || return 0
  awk '/<!--/ { c=1 } /-->/ { c=0; next } c { next }
       /^- \[unresolved\] / { printf "%d:%s\n", NR, $0 }' "$1" 2>/dev/null || true
}

mj_question_resolve() {
  local sel="" answer="" owner=""
  while [ $# -gt 0 ]; do case "$1" in
    --answer) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--answer needs text"; answer="$2"; shift 2 ;;
    --answer=*) answer="${1#--answer=}"; shift ;;
    --owner-answer) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--owner-answer needs an id"; owner="$2"; shift 2 ;;
    -*) mj_die "$MJ_EX_USAGE" "question resolve: unknown option $1" ;;
    *) [ -z "$sel" ] || mj_die "$MJ_EX_USAGE" "question resolve: one selector only"; sel="$1"; shift ;;
  esac; done
  [ -n "$sel" ] || mj_die "$MJ_EX_USAGE" "question resolve: a number or matching text is required"
  if [ -n "$owner" ]; then
    # The answer is the record, cited by id: a question resolved by the owner names the
    # tracked record that holds what the owner said, and an id that names none is refused.
    mj_require_installed
    [ -f "$(mj_answer_dir)/$owner.yaml" ] || mj_die "$MJ_EX_MISSING" "question resolve: owner-answer:$owner names no record under $(mj_rel "$(mj_answer_dir)")/"
    answer="${answer:+$answer }owner-answer:$owner"
  fi
  [ -n "$answer" ] || mj_die "$MJ_EX_USAGE" "question resolve: --answer is required (resolving without an answer loses the answer)"
  mj_is_multiline "$answer" && mj_die "$MJ_EX_USAGE" "question resolve: the answer must be single-line" 
  mj_question_require_read
  # Any unresolved question can be resolved, not only one the active task opened. The gate
  # blocks on all of them, and a gate nobody can clear is a gate that gets worked around.
  # The ledger still records which task did the resolving.
  mj_record_attribution
  local id="${MJ_RECORD_TASK:-none}" fields=""
  [ -z "$MJ_RECORD_TASK" ] || fields="\"task_id\":\"$MJ_RECORD_TASK\","

  local open; open="$(mj_question_unresolved_any "$MJ_Q")"
  [ -n "$open" ] || mj_die "$MJ_EX_MISSING" "no unresolved question"
  local hits line_no
  case "$sel" in
    ''|*[!0-9]*) hits="$(printf '%s\n' "$open" | grep -F -- "$sel" || true)" ;;
    *) hits="$(printf '%s\n' "$open" | sed -n "${sel}p")" ;;
  esac
  [ -n "$hits" ] || mj_die "$MJ_EX_MISSING" "question resolve: '$sel' matches no unresolved question"
  [ "$(printf '%s\n' "$hits" | wc -l | tr -d ' ')" = 1 ] || mj_die "$MJ_EX_USAGE" "question resolve: '$sel' matches more than one question; be more specific"
  line_no="${hits%%:*}"

  local tmp; tmp="$(mktemp "${TMPDIR:-/tmp}/mj.q.XXXXXX")"
  awk -v n="$line_no" -v d="$(date -u +%Y-%m-%d)" -v a="$answer" '
    NR==n { sub(/^- \[unresolved\]/, "- [resolved " d "]"); print $0 " — " a; next }
    { print }' "$MJ_Q" > "$tmp" && mv "$tmp" "$MJ_Q"
  local text; text="$(printf '%s' "$hits" | sed -e 's/^[0-9]*:- \[unresolved\] [^ ]* — //' -e 's/ ([0-9-]*)$//')"
  mj_ledger_append question.resolved "${fields}\"question\":\"$(mj_json_esc "$text")\",\"answer\":\"$(mj_json_esc "$answer")\"" "$MJ_RECORD_SESSION"
  printf 'resolved for %s: %s\n' "$id" "$text"
}

mj_question_list() {
  local all=0 want=""
  while [ $# -gt 0 ]; do case "$1" in
    --all) all=1; shift ;;
    --task) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--task needs an id"; want="$2"; shift 2 ;;
    --task=*) want="${1#--task=}"; shift ;;
    *) mj_die "$MJ_EX_USAGE" "question list: unknown option $1" ;;
  esac; done
  mj_question_require_read
  # Default: every unresolved question, because every one of them refuses a completed
  # finish here. --task narrows to what one task opened; --all adds the resolved ones.
  # The numbering matches what `question resolve <n>` selects.
  local n=0 line
  if [ -z "$want" ] && [ "$all" != 1 ]; then
    while IFS= read -r line; do n=$((n+1)); printf '%s  %s\n' "$n" "$(printf '%s' "$line" | sed 's/^- //')"; done < <(mj_question_unresolved_any "$MJ_Q" | sed 's/^[0-9]*://')
  elif [ "$all" = 1 ] && [ -z "$want" ]; then
    # the file's own template lives in an HTML comment and is not an entry
    while IFS= read -r line; do n=$((n+1)); printf '%s\n' "$line"; done < <(awk '/<!--/{c=1} /-->/{c=0;next} !c && /^- \[(unresolved|resolved )/' "$MJ_Q" 2>/dev/null || true)
  else
    while IFS= read -r line; do n=$((n+1)); printf '%s  %s\n' "$n" "$(printf '%s' "$line" | sed 's/^- //')"; done < <(mj_question_unresolved "$want" "$MJ_Q" | sed 's/^[0-9]*://')
    if [ "$all" = 1 ]; then grep -E "^- \[resolved [0-9-]+\] $want " "$MJ_Q" 2>/dev/null | sed 's/^- /-  /' && n=$((n+1)); fi
  fi
  [ "$n" = 0 ] && printf 'no open questions%s\n' "${want:+ for $want}"
  return 0
}

# line numbers of entries that look like questions but do not parse. A gate that cannot
# read an entry is a gate that can be bypassed by mistyping one, so this is a failure,
# not a warning. Lines inside an HTML comment are the file's own documented example.
mj_question_malformed() {
  [ -f "$1" ] || return 0
  awk '
    /<!--/ { c=1 } /-->/ { c=0; next }
    c { next }
    /^- \[/ {
      if ($0 ~ /^- \[unresolved\] [^ ]+ — .+ \([0-9]{4}-[0-9]{2}-[0-9]{2}\)$/) next
      if ($0 ~ /^- \[resolved [0-9]{4}-[0-9]{2}-[0-9]{2}\] [^ ]+ — .+$/) next
      printf "%s ", NR
    }' "$1"
}

# ---------------------------------------------------------------- owner answers (I4669)
# An owner's answer is a durable fact, not a blocker: it outlives the task and the session
# that asked, and other records cite it. So it is not a line of open-questions.md but a
# tracked record of its own kind, .ai/repo/answers/<id>.yaml (owner-answer/v1), which the
# registry validates against its schema and serves as majordomus://owner-answer/<id>.
#
# What makes it worth more than prose:
#   * the text is held as JSON strings, one key per line, so no YAML folding or editor
#     trimming can lose a byte, and its sha256 digests are computed here and recomputed by
#     the validator, so an edit or a paraphrase after the fact is refused;
#   * the record says how the text was obtained (text_source) instead of claiming proof it
#     cannot have: transcript-checked when the question was found byte for byte in the
#     provider's transcript and the answer in its result, typed when a recorder typed it;
#   * the recording session, time and identity are computed, never typed, and the asking
#     session is labelled with where it came from (environment, transcript or declared).
# It has no summary or paraphrase field: a translation sits beside the verbatim text and
# never replaces it.

mj_answer_dir() { printf '%s' "$MJ_AI_REPO_DIR/answers"; }

# A record as one JSON object. The writer emits block YAML whose every scalar is a JSON
# literal (a double-quoted JSON string is a YAML double-quoted scalar), in the one shape below,
# so reading it back is a transcription of the lines, never a YAML parse that could re-fold:
#   key: <json>            key:              key:
#                            sub: <json>       - <json>
#                                              - sub: <json>
#                                                sub: <json>
mj_answer_json() {
  awk '
    function close_item() { if (item) { printf "}"; item = 0 } }
    function close_top() { close_item(); if (cont == "o") printf "}"; else if (cont == "l") printf "]"; cont = "" }
    function sep_top() { if (ntop++) printf ","; }
    /^#/ || /^[[:space:]]*$/ { next }
    /^[a-z0-9_]+:$/ { close_top(); sep_top(); k = substr($0, 1, length($0) - 1); printf "\"%s\":", k; cont = "?"; n = 0; next }
    /^[a-z0-9_]+: / { close_top(); sep_top(); i = index($0, ": "); printf "\"%s\":%s", substr($0, 1, i - 1), substr($0, i + 2); next }
    /^  - [a-z0-9_]+: / { if (cont == "?") { printf "["; cont = "l" } close_item(); if (n++) printf ","; s = substr($0, 5); i = index(s, ": ");
                       printf "{\"%s\":%s", substr(s, 1, i - 1), substr(s, i + 2); item = 1; next }
    /^    [a-z0-9_]+: / { s = substr($0, 5); i = index(s, ": "); printf ",\"%s\":%s", substr(s, 1, i - 1), substr(s, i + 2); next }
    /^  - / { if (cont == "?") { printf "["; cont = "l" } if (n++) printf ","; printf "%s", substr($0, 5); next }
    /^  [a-z0-9_]+: / { if (cont == "?") { printf "{"; cont = "o" } if (n++) printf ","; s = substr($0, 3); i = index(s, ": ");
                     printf "\"%s\":%s", substr(s, 1, i - 1), substr(s, i + 2); next }
    { bad = 1 }
    BEGIN { printf "{" }
    END { close_top(); printf "}"; if (bad) exit 1 }' "$1"
}
# one value of a record: a string as its bytes, anything else as compact JSON
mj_answer_get() { mj_answer_json "$1" 2>/dev/null | jq -j --arg k "$2" '.[$k] // empty | if type == "string" then . else tojson end' 2>/dev/null || true; }
mj_answer_raw() { mj_answer_json "$1" 2>/dev/null | jq -c --arg k "$2" '.[$k] // empty' 2>/dev/null || true; }
# the writer's side of the same shape
mj_answer_yaml() {
  jq -r 'to_entries[] |
    if (.value | type) == "object" then "\(.key):", (.value | to_entries[] | "  \(.key): \(.value | tojson)")
    elif (.value | type) == "array" then "\(.key):", (.value[] |
      if type == "object" then (to_entries | (.[0] | "  - \(.key): \(.value | tojson)"), (.[1:][] | "    \(.key): \(.value | tojson)"))
      else "  - \(tojson)" end)
    else "\(.key): \(.value | tojson)" end'
}

mj_answer_sha() { mj_sha256sum | cut -c1-64; }
# The answer digest is over the compact JSON of what was chosen and what was typed, so that a
# multi-select answer and a free-text answer have one definition.
mj_answer_digest_answer() { jq -cj '{selected: (.selected // []), free_text: (.free_text // "")}' | mj_answer_sha; }

mj_answer_require_jq() { mj_has jq || mj_die "$MJ_EX_MISSING" "question $1: jq is required to read and write owner-answer records"; }

# mj_answer_transcript <file> <locator> — prints a JSON object read from the provider's
# transcript (Anthropic message content blocks, one JSON object per line, as Claude Code
# writes its session log): the question, options, multi_select, the asking session, both
# times, the answer segment of the result and the sha256 of the raw lines. A locator
# `<tool-use id>#<n>` names question n of one question-tool call; any other locator names one
# chat message by its uuid, whose text is the free text. Refuses, never guesses.
mj_answer_transcript() {
  local file="$1" loc="$2" id idx n use_line res_line raw
  [ -f "$file" ] || mj_die "$MJ_EX_MISSING" "question record: no transcript at $file"
  case "$loc" in
    *'#'*) id="${loc%%#*}"; idx="${loc##*#}"
      case "$idx" in ''|*[!0-9]*) mj_die "$MJ_EX_USAGE" "question record: --locator $loc: the index after # must be a number" ;; esac
      use_line=""; res_line=""
      for n in $(grep -n -F -- "\"$id\"" "$file" | cut -d: -f1); do
        raw="$(sed -n "${n}p" "$file")"
        if [ -z "$use_line" ] && printf '%s' "$raw" | jq -e --arg id "$id" '[.message.content[]? | select(type=="object" and .type=="tool_use" and .id==$id)] | length > 0' >/dev/null 2>&1; then use_line="$n"; continue; fi
        if [ -z "$res_line" ] && printf '%s' "$raw" | jq -e --arg id "$id" '[.message.content[]? | select(type=="object" and .type=="tool_result" and .tool_use_id==$id)] | length > 0' >/dev/null 2>&1; then res_line="$n"; fi
      done
      [ -n "$use_line" ] || mj_die "$MJ_EX_MISSING" "question record: no tool use $id in $(basename "$file")"
      [ -n "$res_line" ] || mj_die "$MJ_EX_MISSING" "question record: tool use $id in $(basename "$file") has no result: the owner has not answered it"
      { sed -n "${use_line}p" "$file"; sed -n "${res_line}p" "$file"; } > "$MJ_ANSWER_TMP.raw"
      jq -cs --arg id "$id" --argjson i "$idx" --arg sha "$(mj_answer_sha < "$MJ_ANSWER_TMP.raw")" '
        (.[0]) as $u | (.[1]) as $r
        | ([$u.message.content[] | select(type=="object" and .type=="tool_use" and .id==$id)][0].input.questions[$i]) as $q
        | ([$r.message.content[] | select(type=="object" and .type=="tool_result" and .tool_use_id==$id)][0].content
           | if type=="string" then . elif type=="array" then (map(.text? // "") | join("")) else "" end) as $res
        | if $q == null then error("no question \($i) in tool use \($id)") else . end
        | { question: $q.question, multi_select: ($q.multiSelect // false),
            options: [($q.options // [])[] | {label} + (if .description then {description} else {} end)],
            session: ($u.sessionId // ""), asked_at: ($u.timestamp // ""), answered_at: ($r.timestamp // ""),
            result: $res, sha256: $sha, kind: "tool" }' "$MJ_ANSWER_TMP.raw" 2>"$MJ_ANSWER_TMP.err" \
        || mj_die "$MJ_EX_MISSING" "question record: the transcript does not carry the question: $(cat "$MJ_ANSWER_TMP.err")" ;;
    *)
      use_line=""
      for n in $(grep -n -F -- "\"$loc\"" "$file" | cut -d: -f1); do
        raw="$(sed -n "${n}p" "$file")"
        if printf '%s' "$raw" | jq -e --arg u "$loc" '.uuid == $u and .type == "user"' >/dev/null 2>&1; then use_line="$n"; break; fi
      done
      [ -n "$use_line" ] || mj_die "$MJ_EX_MISSING" "question record: no message $loc from the person in $(basename "$file")"
      sed -n "${use_line}p" "$file" > "$MJ_ANSWER_TMP.raw"
      jq -c --arg sha "$(mj_answer_sha < "$MJ_ANSWER_TMP.raw")" '
        (.message.content | if type=="string" then . else (map(select(.type=="text") | .text) | join("")) end) as $t
        | { free_text: $t, session: (.sessionId // ""), asked_at: (.timestamp // ""), answered_at: (.timestamp // ""),
            sha256: $sha, kind: "message" }' "$MJ_ANSWER_TMP.raw" ;;
  esac
}

mj_answer_record() {
  mj_answer_require_jq record
  local from="" transcript="" locator="" slug="" json args='{}' t
  MJ_ANSWER_TMP="$(mktemp "${TMPDIR:-/tmp}/mj.answer.XXXXXX")"
  # Every flag becomes one key of an input object; --from supplies the same keys as a JSON
  # document, so multi-line verbatim text never passes through shell quoting.
  local q="" ch="" pr="" ai="" aa="" wa="" ft="" sup="" tl="" tq="" ta="" ms="" hasq=0 hasft=0
  local opts="[]" selj="[]" setj="[]"
  while [ $# -gt 0 ]; do
    case "$1" in
      --from|--transcript|--locator|--slug|--question|--option|--selected|--free-text|--channel|--provider|--asked-in|--asked-at|--answered-at|--settles|--supersedes|--translation-language|--translation-question|--translation-answer)
        [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "question record: $1 needs a value" ;;
    esac
    case "$1" in
      --from) from="$2" ;;
      --transcript) transcript="$2" ;;
      --locator) locator="$2" ;;
      --slug) slug="$2" ;;
      --question) q="$2"; hasq=1 ;;
      --option) opts="$(printf '%s' "$opts" | jq -c --arg l "$2" '. + [{label: $l}]')" ;;
      --selected) selj="$(printf '%s' "$selj" | jq -c --arg l "$2" '. + [$l]')" ;;
      --free-text) ft="$2"; hasft=1 ;;
      --channel) ch="$2" ;;
      --provider) pr="$2" ;;
      --asked-in) ai="$2" ;;
      --asked-at) aa="$2" ;;
      --answered-at) wa="$2" ;;
      --settles) setj="$(printf '%s' "$setj" | jq -c --arg l "$2" '. + [$l]')" ;;
      --supersedes) sup="$2" ;;
      --translation-language) tl="$2" ;;
      --translation-question) tq="$2" ;;
      --translation-answer) ta="$2" ;;
      --multi-select) ms=true; shift; continue ;;
      *) mj_die "$MJ_EX_USAGE" "question record: unknown argument $1" ;;
    esac
    shift 2
  done
  mj_require_installed
  if [ -n "$from" ]; then
    if [ "$from" = - ]; then json="$(cat)"; else [ -f "$from" ] || mj_die "$MJ_EX_MISSING" "question record: no file $from"; json="$(cat "$from")"; fi
    printf '%s' "$json" | jq -e 'type == "object"' >/dev/null 2>&1 || mj_die "$MJ_EX_USAGE" "question record: --from must hold one JSON object"
    args="$json"
  fi
  # flags win over the document, one key at a time
  args="$(printf '%s' "$args" | jq -c --arg q "$q" --argjson hq "$hasq" --arg ft "$ft" --argjson hf "$hasft" \
    --arg ch "$ch" --arg pr "$pr" --arg ai "$ai" --arg aa "$aa" --arg wa "$wa" --arg sup "$sup" \
    --arg tl "$tl" --arg tq "$tq" --arg ta "$ta" --arg ms "$ms" --arg slug "$slug" \
    --argjson o "$opts" --argjson s "$selj" --argjson st "$setj" '
      (if $hq == 1 then .question = $q else . end)
      | (if $hf == 1 then .free_text = $ft else . end)
      | (if $ch != "" then .channel = $ch else . end) | (if $pr != "" then .provider = $pr else . end)
      | (if $ai != "" then .asked_session = $ai else . end) | (if $aa != "" then .asked_at = $aa else . end)
      | (if $wa != "" then .answered_at = $wa else . end) | (if $sup != "" then .supersedes = $sup else . end)
      | (if $slug != "" then .slug = $slug else . end) | (if $ms == "true" then .multi_select = true else . end)
      | (if ($o | length) > 0 then .options = $o else . end)
      | (if ($s | length) > 0 then .selected = $s else . end)
      | (if ($st | length) > 0 then .settles = ((.settles // []) + $st) else . end)
      | (if $tl != "" then .translation = ({language: $tl} + (if $tq != "" then {question: $tq} else {} end) + (if $ta != "" then {answer: $ta} else {} end)) else . end)')" \
    || mj_die "$MJ_EX_USAGE" "question record: the input does not parse"
  [ -z "$transcript" ] || [ -n "$locator" ] || mj_die "$MJ_EX_USAGE" "question record: --transcript needs --locator (the tool-use id and question index, or a message id)"
  [ -z "$locator" ] || [ -n "$transcript" ] || mj_die "$MJ_EX_USAGE" "question record: --locator needs --transcript"

  local source_kind="typed" session_source="declared" tr=""
  if [ -n "$transcript" ]; then
    tr="$(mj_answer_transcript "$transcript" "$locator")" || exit $?
    # The transcript is the authority for what it carries: a typed question that differs from
    # it is refused rather than either one being preferred in silence.
    args="$(jq -cn --argjson a "$args" --argjson t "$tr" --arg f "$(basename "$transcript")" --arg loc "$locator" '
      $a as $a | $t as $t
      | if $t.kind == "tool" then
          (if ($a.question // null) != null and $a.question != $t.question then error("the question differs from the transcript") else . end)
          | (if ($a.options // null) != null and ([$a.options[].label] != [$t.options[].label]) then error("the options differ from the transcript") else . end)
          | $a + {question: $t.question, options: $t.options, multi_select: $t.multi_select, channel: ($a.channel // "question-tool")}
          | ($t.result) as $res
          | ("\"" + $t.question + "\"=\"") as $key
          | ($res | split($key) | if length > 1 then .[1:] | join($key) else $res end) as $seg
          | (if ((.selected // []) | length) == 0 and ((.free_text // "") == "") then error("--selected or --free-text is required: the answer is checked against the result, never parsed out of it") else . end)
          | (reduce (.selected // [])[] as $l (.; if ($seg | contains($l)) then . else error("the selected label \"" + $l + "\" is not in the answer the transcript holds") end))
          | (.free_text // "") as $ft
          | (if $ft != "" and (($seg | contains($ft)) | not) then error("the free text is not in the answer the transcript holds byte for byte") else . end)
        else
          (if ($a.free_text // null) != null and $a.free_text != $t.free_text then error("the free text differs from the transcript message") else . end)
          | $a + {free_text: $t.free_text, channel: ($a.channel // "chat")}
        end
      | . + {asked_session: (if $t.session != "" then $t.session else .asked_session end),
             asked_at: (if $t.asked_at != "" then $t.asked_at else .asked_at end),
             answered_at: (if $t.answered_at != "" then $t.answered_at else .answered_at end),
             source: {transcript: $f, locator: $loc, sha256: $t.sha256}}' 2>"$MJ_ANSWER_TMP.err")" \
      || mj_die "$MJ_EX_REFUSED" "question record: refused: $(sed 's/^jq: error[^:]*: //' "$MJ_ANSWER_TMP.err")"
    source_kind="transcript-checked"; session_source="transcript"
    printf '%s' "$args" | jq -e '(.asked_session // "") != ""' >/dev/null || session_source="declared"
  fi

  # computed: the provider, the asking session when this process names its own, the recorder
  local env_session env_provider now by rec
  env_session="$(mj_provider_session_env)"; env_provider="$(mj_provider_of_session_env 2>/dev/null || true)"
  now="$(mj_now)"
  by="$(git -C "$MJ_ROOT" config user.name 2>/dev/null || true) <$(git -C "$MJ_ROOT" config user.email 2>/dev/null || true)>"
  mj_record_attribution
  if printf '%s' "$args" | jq -e '(.asked_session // "") == ""' >/dev/null; then
    [ -n "$env_session" ] || mj_die "$MJ_EX_USAGE" "question record: no asking session: this process names no provider session; give --asked-in <session> or --transcript"
    args="$(printf '%s' "$args" | jq -c --arg s "$env_session" '.asked_session = $s')"; session_source="environment"
  fi
  rec="$(printf '%s' "$args" | jq -c --arg src "$source_kind" --arg ss "$session_source" --arg now "$now" --arg by "$by" \
      --arg rs "${MJ_RECORD_SESSION:-}" --arg rt "${MJ_RECORD_TASK:-}" --arg ep "${env_provider:-unknown}" '
    (.channel // (if (.options // []) | length > 0 then "question-tool" else "chat" end)) as $ch
    | if ([ "question-tool", "terminal", "chat", "forge", "other" ] | index($ch)) == null then error("unknown channel " + $ch + " (question-tool|terminal|chat|forge|other)") else . end
    | if $ch == "question-tool" and ((.options // []) | length) == 0 then error("a question-tool answer carries the options the tool offered") else . end
    | if $ch != "chat" and ((.question // "") == "") then error("the question is required; only a chat turn may answer without one") else . end
    | if ((.selected // []) | length) == 0 and ((.free_text // "") == "") then error("an empty answer: give --selected or --free-text") else . end
    | (.options // []) as $o
    | (reduce (.selected // [])[] as $l (.; if ([$o[].label] | index($l)) == null then error("\"" + $l + "\" is not one of the offered options") else . end))
    | if (.multi_select // false) == false and ((.selected // []) | length) > 1 then error("several labels selected on a single-select question") else . end
    | if (.slug // "") | test("^[a-z0-9][a-z0-9-]*$") | not then error("--slug is required: lower-case letters, digits and hyphens") else . end
    | ((.answered_at // $now)) as $wa
    | {schema: "owner-answer/v1",
       id: ("A-" + ($wa | .[0:10] | gsub("-"; "")) + "-" + .slug),
       channel: $ch, provider: (.provider // $ep),
       asked: {session: .asked_session, at: (.asked_at // $wa), session_source: $ss},
       answered_at: $wa, text_source: $src}
      + (if .source then {source: .source} else {} end)
      + (if (.question // "") != "" then {question: .question} else {} end)
      + (if (.multi_select // null) != null then {multi_select: .multi_select} else {} end)
      + (if ($o | length) > 0 then {options: $o} else {} end)
      + (if ((.selected // []) | length) > 0 then {selected: .selected} else {} end)
      + (if (.free_text // "") != "" then {free_text: .free_text} else {} end)
      + {digest: {question: "", answer: ""}}
      + (if .translation then {translation: .translation} else {} end)
      + (if ((.settles // []) | length) > 0 then {settles: .settles} else {} end)
      + (if .supersedes then {supersedes: .supersedes} else {} end)
      + {recorded: ({session: $rs, at: $now, by: $by} + (if $rt != "" then {task: $rt} else {} end))}' 2>"$MJ_ANSWER_TMP.err")" \
    || mj_die "$MJ_EX_REFUSED" "question record: refused: $(sed 's/^jq: error[^:]*: //' "$MJ_ANSWER_TMP.err")"
  local s
  while IFS= read -r s; do
    case "$s" in ''|issue:?*|pr:?*|adr:?*|decision:?*|task:?*|rule:?*|intent:?*|other:?*) ;;
      *) mj_die "$MJ_EX_USAGE" "question record: --settles $s: <issue|pr|adr|decision|task|rule|intent|other>:<ref>" ;; esac
  done <<EOF_SETTLES
$(printf '%s' "$rec" | jq -r '(.settles // [])[]')
EOF_SETTLES
  local id dq da dir file
  id="$(printf '%s' "$rec" | jq -r .id)"
  dq="$(printf '%s' "$rec" | jq -j '.question // ""' | mj_answer_sha)"
  da="$(printf '%s' "$rec" | mj_answer_digest_answer)"
  rec="$(printf '%s' "$rec" | jq -c --arg q "$dq" --arg a "$da" '.digest = {question: $q, answer: $a}')"
  dir="$(mj_answer_dir)"; file="$dir/$id.yaml"
  [ ! -e "$file" ] || mj_die "$MJ_EX_REFUSED" "question record: $(mj_rel "$file") exists; a record is never rewritten: record a new one with --supersedes $id"
  if printf '%s' "$rec" | jq -e '.supersedes' >/dev/null; then
    t="$(printf '%s' "$rec" | jq -r .supersedes)"
    [ -f "$dir/$t.yaml" ] || mj_die "$MJ_EX_USAGE" "question record: --supersedes $t names no record"
  fi
  mkdir -p "$dir"
  # the directory says what it is for, as every directory of the layer does
  [ -f "$dir/README.md" ] || cp "$MJ_SKELETON_DIR/templates/answers-README.md" "$dir/README.md"
  {
    printf '# One owner'"'"'s answer, written by `majordomus question record`; never edited. Every value is\n'
    printf '# a JSON string or object, byte for byte, and both digests are recomputed by the validator.\n'
    printf '%s' "$rec" | mj_answer_yaml
  } > "$file.tmp" && mv "$file.tmp" "$file"
  rm -f "$MJ_ANSWER_TMP" "$MJ_ANSWER_TMP.raw" "$MJ_ANSWER_TMP.err"
  printf 'recorded %s (%s): %s\n' "$id" "$source_kind" "$(mj_rel "$file")"
  printf 'cite it as owner-answer:%s; commit it with the work it settles\n' "$id"
}

mj_answer_list() {
  local want=""
  while [ $# -gt 0 ]; do case "$1" in
    --settles) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--settles needs a ref"; want="$2"; shift 2 ;;
    *) mj_die "$MJ_EX_USAGE" "question answers: unknown option $1" ;;
  esac; done
  mj_require_installed; mj_answer_require_jq answers
  local f n=0 dir; dir="$(mj_answer_dir)"
  for f in "$dir"/A-*.yaml; do
    [ -f "$f" ] || continue
    if [ -n "$want" ]; then mj_answer_raw "$f" settles | jq -e --arg w "$want" 'index($w) != null' >/dev/null 2>&1 || continue; fi
    n=$((n+1))
    printf '%s  %s  %s  %s\n' "$(basename "$f" .yaml)" "$(mj_answer_get "$f" answered_at)" "$(mj_answer_get "$f" text_source)" \
      "$( { mj_answer_get "$f" question; [ -n "$(mj_answer_get "$f" question)" ] || mj_answer_get "$f" free_text; } | tr '\n' ' ' | cut -c1-80)"
  done
  [ "$n" -gt 0 ] || printf 'no owner answers%s\n' "${want:+ settle $want}"
  return 0
}

mj_answer_show() {
  local id="${1:-}"
  [ -n "$id" ] || mj_die "$MJ_EX_USAGE" "question answer: an id is required (question answers lists them)"
  mj_require_installed; mj_answer_require_jq answer
  local f; f="$(mj_answer_dir)/$id.yaml"
  [ -f "$f" ] || mj_die "$MJ_EX_MISSING" "question answer: no record $id"
  cat "$f"
  case "$(mj_answer_get "$f" text_source)" in
    transcript-checked) printf '\nprovenance: the writer checked the question byte for byte against the provider transcript %s (%s) and the answer against its result, at record time. This clone cannot re-run that check without the transcript; the digests prove only that the text has not changed since.\n' \
        "$(mj_answer_get "$f" source | jq -r .transcript)" "$(mj_answer_get "$f" source | jq -r .locator)" ;;
    provider-payload) printf '\nprovenance: written from the provider'"'"'s own hook payload.\n' ;;
    *) printf '\nprovenance: typed by the recorder, unverified. The digests prove only that the text has not changed since it was typed.\n' ;;
  esac
  printf 'asked by session %s (%s)\n' "$(mj_answer_get "$f" asked | jq -r .session)" "$(mj_answer_get "$f" asked | jq -r .session_source)"
}

# The doctrine behind majordomus.owner-answers-are-recorded. Every tracked record holds what
# its digests say and offers what it selects; every owner-answer:<id> cited under the layer's
# repository half resolves. It reads explicit citations only: a scan for quoted answers or for
# "the owner decided" in prose would flag ordinary text and still miss a paraphrase.
mj_validate_owner_answers() {
  local dir f id n=0 bad=0 why
  dir="$(mj_answer_dir)"
  if ! mj_has jq; then
    MJ_DOCTRINE_SKIPPED=1; mj_doctrine_skip records "owner answers" "not checked: jq is not installed"; return 0
  fi
  for f in "$dir"/*.yaml; do
    [ -f "$f" ] || continue
    n=$((n+1)); id="$(basename "$f" .yaml)"; why=""
    mj_answer_json "$f" | jq -e 'type == "object"' >/dev/null 2>&1 || why="it is not in the shape question record writes"
    [ -n "$why" ] || [ "$(mj_answer_get "$f" id)" = "$id" ] || why="its id is not its file name"
    [ -n "$why" ] || [ "$(mj_answer_get "$f" schema)" = "owner-answer/v1" ] || why="it is not an owner-answer/v1 record"
    if [ -z "$why" ]; then
      [ "$(mj_answer_get "$f" question | mj_answer_sha)" = "$(mj_answer_get "$f" digest | jq -r .question)" ] || why="the question is not the text its digest was taken of"
    fi
    if [ -z "$why" ]; then
      [ "$(mj_answer_json "$f" | mj_answer_digest_answer)" = "$(mj_answer_get "$f" digest | jq -r .answer)" ] \
        || why="the answer is not the text its digest was taken of"
    fi
    if [ -z "$why" ]; then
      mj_answer_json "$f" | jq -e '(.options // []) as $o | all((.selected // [])[]; . as $l | [$o[].label] | index($l) != null)' >/dev/null 2>&1 \
        || why="it selects a label that was not offered"
    fi
    if [ -z "$why" ] && [ "$(mj_answer_get "$f" text_source)" = transcript-checked ]; then
      mj_answer_raw "$f" source | jq -e '(.locator // "") != "" and ((.sha256 // "") | test("^[0-9a-f]{64}$"))' >/dev/null 2>&1 || why="it says transcript-checked without a locator and a digest"
    fi
    if [ -n "$why" ]; then
      bad=1; mj_doctrine_fail records "$(mj_rel "$f")" "$why; a record is never edited: restore it and record a new one with --supersedes" "majordomus question answer $id"
    fi
  done
  # citations: only explicit ones, only under the repository half of the layer
  local cite file cid
  while IFS= read -r cite; do
    [ -n "$cite" ] || continue
    file="${cite%%:*}"; cid="${cite#*:owner-answer:}"
    [ -f "$dir/$cid.yaml" ] && continue
    bad=1; mj_doctrine_fail records "$file" "cites owner-answer:$cid, which names no record under $(mj_rel "$dir")/" "majordomus question answers"
  done < <(git -C "$MJ_ROOT" grep -I -o -E 'owner-answer:A-[0-9]{8}-[a-z0-9][a-z0-9-]*' -- "$(mj_rel "$MJ_AI_REPO_DIR")" 2>/dev/null | sed 's/-*$//' | sort -u || true)
  [ "$bad" = 1 ] || mj_doctrine_ok records "owner answers" "$n record(s) hold the text their digests name, and every citation resolves"
  return 0
}

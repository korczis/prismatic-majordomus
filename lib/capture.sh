#!/usr/bin/env bash
# sourced by doctor as well as by the dispatcher; guard against re-sourcing
[ -n "${MJ_LIB_capture:-}" ] && return 0 || MJ_LIB_capture=1
# capture — raw local prompt history, written below the model rather than by it.
#
# A worker cannot be asked to record its own prompts. It never sees the bytes the person
# typed, only what the provider assembled from them, and a record that depends on a model
# choosing to write it is missing exactly the prompts that mattered. An instruction in a
# bootstrap file is a request, not a mechanism. So the only honest capture happens in the
# provider's own hook, before the model is invoked, and this command is what that hook runs.
#
# What follows from that:
#
#   * A provider without an observable prompt event is reported as unsupported. It is never
#     silently treated as empty, and Majordomus never claims history it cannot have.
#   * The archive holds the person's prompts and nothing the model said. The writer emits a
#     closed set of fields, so a transcript cannot arrive through it.
#   * The captured text is the raw span from the provider's payload, still JSON-escaped,
#     copied through unchanged: no decode and re-encode step that could lose an escape.
#   * `capture prompt` never exits 2. In a provider hook that exit code can reject the
#     person's prompt, and a broken archive must never cost someone their input.
#   * A capture that failed leaves .capture.log beside the records, and doctor treats a
#     non-empty log as a failure of the repository. This is the only thing that can catch
#     the failure that matters: the hook still runs, the self test still passes because it
#     sends a payload of the tool's own making, and the provider has quietly renamed the
#     field the real prompt arrives in. Without the log that loses every prompt in silence.
#   * Nothing here deletes a record, ever. The ledger, the checkpoints and the handovers
#     rotate under a policy cap because each is a restatement of state that is still
#     available elsewhere. A prompt is not: it exists once, it was never derived from
#     anything, and no other file in the repository can reconstruct it. So the archive
#     grows, `doctor` reports how much of it there is, and a person who wants it smaller
#     removes files themselves — deliberately, and not as a side effect of the hook that
#     was supposed to be keeping them.
#
# One prompt is one file: .ai/local/prompts/YYYYMMDDHHMMSS-<slug>.json, where the slug is
# the opening of the prompt itself. A record is therefore immutable once written, two
# hooks racing cannot interleave inside one file, the name sorts chronologically, and a
# person can find the prompt they are looking for by listing the directory. Retention and
# reading are both a directory listing.
#
# Beside each record is the same prompt as Markdown, under the same stem. The two are not
# alternatives, and which one a reader gets is not the writer's choice to make: the JSON is
# the record — the provider's own spans, still escaped, byte for byte what it sent — and the
# Markdown is that record rendered for a person, the prompt decoded into the body and the
# rest into front matter. A rendering can be rebuilt from a record and never the other way
# round, so the record is the thing capture must not lose and the rendering is the thing
# that makes the archive worth opening. Both are written, always: an archive that is half
# one format and half the other is a directory nobody can read end to end, so the pair is
# an invariant the doctrine decides rather than a habit the writer is trusted to keep.
#
# The archive is evidence, not knowledge: nothing loads it into a context, no command
# retrieves from it, and it lives under the ignored half of the AI layer.

MJ_CAPTURE_SCHEMA="majordomus.prompt/v1"
# The schema identifier is also the path to what describes it: `<namespace>.<name>/<version>`
# lives at `share/schemas/<namespace>/<name>/<version>`, and there are two files there because
# there are two projections and they are not the same kind of thing. The record is JSON and is
# described by JSON Schema, in `.schema.json`. The document is Markdown — a shape, an order,
# sections present or absent — and is described by protobuf, in `.proto`, where a message and
# its field numbers say "these parts, in this order" without pretending a document is a bag of
# keys. A record therefore carries the way to read both halves, and nothing has to publish a
# lookup table for the archive to be self-describing.
MJ_CAPTURE_SCHEMA_DIR="share/schemas"
MJ_CAPTURE_SCHEMA_EXT="schema.json proto"

# The fields the hook writes, in order. Everything observable before the model runs is here,
# and this is what a record always carries.
MJ_CAPTURE_FIELDS="schema started_at provider event id session source cwd repository branch head text"

# The fields the schema declares that the hook cannot fill, because at UserPromptSubmit the
# turn has not happened. They are absent from a record rather than null in it: an absent
# field says "not observed", and a null one would claim it was observed to be nothing. A
# renderer shows them when they are there, a reformat carries them through, and nothing here
# invents one. The order is the order they render in.
MJ_CAPTURE_OPTIONAL="finished_at duration_ms model effort tokens meta"

# `ts` was this field's name before it had a sibling called finished_at; mj_capture_raw takes
# candidates, so a record written under the old name still reformats and still renders.
MJ_CAPTURE_STARTED="started_at,ts"

# ---------------------------------------------------------------- provider adapters
# One line per provider Majordomus can capture from:
#   1 provider  2 config file  3 event  4 id keys  5 session keys  6 text keys  7 source keys
#   8 shim  9 the source values that mean a person typed it  10 openings that mark a message
#   the provider injected rather than a person writing one
# All paths are repository-relative. Fields 4 to 7 are comma-separated candidates, tried in
# order, because a payload field is a provider's private shape: it is versioned on their
# schedule, renaming one is not a breaking change to them, and a capture built on a single
# assumed name loses every prompt the day it moves. When none of the candidates is present
# the record is not written and the keys that were present are logged, so the next name is
# a fact from a payload rather than a guess.
#
# A provider absent from this table is unsupported: it has no documented event that hands
# the person's prompt to a command before the model runs. Adding a line here does not make
# capture real — doctor proves it by running it.
#
# Fields 9 and 10 exist because the event is not what its name suggests. Claude Code fires
# UserPromptSubmit for messages it injects into the turn as well — a completed background
# task, a system reminder — and those are not the person's prompts. They are filtered here
# and not by the doctrine, because a skip is normal operation rather than a failure: it
# writes no record and no log line. Both tests are declared as data so that a provider
# growing another kind of injected message is one string, not a change to the writer.
#
# The list is what has been observed or documented, not a closed set: the provider owns it
# and can add to it without telling anyone. A marker that arrives and is not listed becomes
# a record whose text opens with an angle bracket, which is visible in a directory listing;
# that is the intended way to find the next one.
MJ_CAPTURE_ADAPTERS='claude-code .claude/settings.json UserPromptSubmit prompt_id,id session_id,sessionId prompt,prompt_text,text prompt_source,source .claude/hooks/majordomus-capture user <task-notification>,<system-reminder>,<subagent-notification>,<cross-session-message>,<hook-message>,<user-prompt-submit-hook>,<command-message>,<command-name>,<command-args>,<local-command-stdout>,<local-command-stderr>,<bash-input>,<bash-stdout>,<bash-stderr>,<ide_selection>,<ide_opened_file>,<ide_diagnostics>'

mj_capture_adapter()   { printf '%s\n' "$MJ_CAPTURE_ADAPTERS" | awk -v p="$1" '$1 == p { print; f = 1 } END { exit !f }'; }
mj_capture_providers() { printf '%s\n' "$MJ_CAPTURE_ADAPTERS" | awk '{ print $1 }'; }
mj_capture_field()     { mj_capture_adapter "$1" 2>/dev/null | awk -v n="$2" '{ print $n }'; }
mj_capture_shim_rel()  { mj_capture_field "$1" 8; }
# the first candidate of a comma-separated adapter field
mj_capture_first()     { mj_capture_field "$1" "$2" | cut -d, -f1; }
mj_capture_shim()      { printf '%s/%s' "$MJ_ROOT" "$(mj_capture_field "$1" 8)"; }

# Where records are written. MJ_CAPTURE_DIR overrides it so that doctor can prove the shim
# works without writing into the archive it is checking.
mj_capture_dir() {
  if [ -n "${MJ_CAPTURE_DIR:-}" ]; then printf '%s' "$MJ_CAPTURE_DIR"
  else printf '%s/prompts' "$MJ_AI_LOCAL_DIR"; fi
}

# ---------------------------------------------------------------- command
mj_cmd_capture() {
  local sub="${1:-}"; [ $# -gt 0 ] && shift
  case "$sub" in
    prompt)  mj_capture_prompt "$@" ;;
    render)  mj_capture_render "$@" ;;
    install) mj_capture_install "$@" ;;
    status)  mj_capture_status "$@" ;;
    --help|-h|"") cat <<H
usage: majordomus capture prompt --provider <name>   < the provider's hook payload
       majordomus capture render [--force]
       majordomus capture install [--provider <name>]
       majordomus capture status [--json]
  providers with an adapter: $(mj_capture_providers | tr '\n' ' ' | sed 's/ $//')
  two files per prompt, same stem, both ignored and never loaded:
    .ai/local/prompts/YYYYMMDDHHMMSS-<slug>.json  the record, the provider's spans unchanged
    .ai/local/prompts/YYYYMMDDHHMMSS-<slug>.md    that record rendered for a person to read
  'capture render' rebuilds a missing rendering from its record; it never writes a record
  'capture prompt' reads one JSON object on stdin and never exits 2, because in a
  provider hook that exit code can reject the person's prompt
H
      [ "$sub" = "" ] && return "$MJ_EX_USAGE"; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "capture: unknown subcommand '$sub' (prompt|render|install|status)" ;;
  esac
}

# ---------------------------------------------------------------- capture prompt
# Reads one provider payload on stdin and appends one record. Every failure short of "the
# archive cannot be written at all" is recorded in the archive's own log and reported as
# success, because this runs in front of a person's prompt.
mj_capture_prompt() {
  local provider="" dir log
  while [ $# -gt 0 ]; do
    case "$1" in
      --provider) provider="${2:-}"; shift 2 ;;
      --provider=*) provider="${1#--provider=}"; shift ;;
      *) mj_err "capture prompt: unknown option $1"; return "$MJ_EX_MISSING" ;;
    esac
  done
  [ -n "$provider" ] || { mj_err "capture prompt: --provider is required"; return "$MJ_EX_MISSING"; }
  mj_capture_adapter "$provider" >/dev/null 2>&1 || {
    mj_err "capture prompt: no adapter for provider '$provider' (have: $(mj_capture_providers | tr '\n' ' '))"
    return "$MJ_EX_MISSING"; }

  # A hook must work in a repository whose policy does not parse, so nothing here needs
  # more than the local half of the layer.
  mj_require_repo 2>/dev/null || { mj_err "capture prompt: not in a repository"; return "$MJ_EX_MISSING"; }
  dir="$(mj_capture_dir)"
  mkdir -p "$dir" 2>/dev/null || { mj_err "capture prompt: cannot create $dir"; return "$MJ_EX_INTERNAL"; }
  log="$dir/.capture.log"

  local payload scan rc=0
  payload="$(mktemp "${TMPDIR:-/tmp}/mj.cap.XXXXXX")"; scan="$payload.f"
  cat > "$payload"
  if ! awk -f "$MJ_LIB_DIR/json_scan.awk" < "$payload" > "$scan" 2>"$scan.err"; then
    mj_capture_log "$log" "payload from '$provider' not understood: $(sed -n 's/^ERROR://p' "$scan.err" | head -n 1)"
    rm -f "$payload" "$scan" "$scan.err"; return 0
  fi
  mj_capture_write "$provider" "$scan" "$dir" || rc=$?
  rm -f "$payload" "$scan" "$scan.err"
  return "$rc"
}

mj_capture_log() { printf '%s %s\n' "$(mj_now)" "$2" >> "$1" 2>/dev/null || true; }

# one record from the scanned payload; the raw JSON spans are copied through unchanged
mj_capture_write() {
  local provider="$1" scan="$2" dir="$3" file event
  local k_id k_session k_text k_source v_text v_id v_session v_source v_cwd
  event="$(mj_capture_field "$provider" 3)"
  k_id="$(mj_capture_field "$provider" 4)";   k_session="$(mj_capture_field "$provider" 5)"
  k_text="$(mj_capture_field "$provider" 6)"; k_source="$(mj_capture_field "$provider" 7)"

  v_text="$(mj_capture_raw "$scan" "$k_text")"
  [ -n "$v_text" ] || {
    mj_capture_log "$dir/.capture.log" \
      "payload from '$provider' carries none of '$k_text'; nothing captured. The payload's own keys were: $(cut -f1 "$scan" | paste -sd, - 2>/dev/null || cut -f1 "$scan" | tr '\n' ',')"
    return 0; }
  v_id="$(mj_capture_raw "$scan" "$k_id")";           [ -n "$v_id" ]      || v_id=null
  v_session="$(mj_capture_raw "$scan" "$k_session")"; [ -n "$v_session" ] || v_session=null
  v_source="$(mj_capture_raw "$scan" "$k_source")";   [ -n "$v_source" ]  || v_source=null
  v_cwd="$(mj_capture_raw "$scan" cwd)";              [ -n "$v_cwd" ]     || v_cwd=null

  # Not everything this event delivers is a person writing something. A payload that names
  # its origin and does not name a person is the provider talking to itself; so is one whose
  # text opens with a marker the provider injects. Neither is a failure and neither is a
  # prompt, so both leave nothing behind — but a payload that says nothing about its origin
  # is captured, because losing a real prompt is the worse of the two mistakes.
  mj_capture_is_person "$provider" "$v_source" "$v_text" || return 0

  # Idempotence is the provider's own prompt identity, not the name: a hook delivered
  # twice, or a run retried, must not become a second record. The scan is the current
  # day's files, which is bounded and is the window a redelivery falls in; a redelivery
  # days later is not a case this claims to cover.
  local day id_plain
  day="$(date -u +%Y%m%d)"
  id_plain="$(mj_capture_ident "$v_id")"
  if [ -n "$id_plain" ] && [ -n "$(grep -lF "\"id\": $v_id," "$dir/$day"*.json 2>/dev/null | head -n 1)" ]; then return 0; fi

  # The name is the second it was captured in, then the opening of the prompt, so the
  # directory reads as a list of what was asked and when.
  local slug stamp file n=2
  slug="$(mj_capture_slug "$v_text")"
  [ -n "$slug" ] || slug="$id_plain"
  [ -n "$slug" ] || slug=prompt
  stamp="$(date -u +%Y%m%d%H%M%S)"
  file="$dir/$stamp-$slug.json"
  # two prompts in one second whose openings agree are still two prompts
  while [ -e "$file" ]; do file="$dir/$stamp-$slug-$n.json"; n=$((n + 1)); [ "$n" -gt 99 ] && return 0; done

  mj_capture_record \
    "\"$MJ_CAPTURE_SCHEMA\"" "\"$(mj_now)\"" "\"$provider\"" "\"$event\"" "$v_id" "$v_session" "$v_source" "$v_cwd" \
    "\"$(mj_json_esc "$MJ_ROOT")\"" "\"$(mj_json_esc "$(mj_git_branch)")\"" "\"$(mj_git_head)\"" "$v_text" > "$file" \
    || { mj_err "capture prompt: cannot write $file"; return "$MJ_EX_INTERNAL"; }

  # The record exists now, so the prompt is safe whatever happens next. A rendering that
  # cannot be written is therefore not a lost prompt and does not go in .capture.log, whose
  # every line means one prompt is gone; it is a missing half of a pair, which the doctrine
  # reports on its own terms and `capture render` repairs from the record still sitting here.
  mj_capture_render_one "$file" || true
  return 0
}

# The record itself: one member per line, in the order MJ_CAPTURE_FIELDS declares. Pretty
# printed because a record is read by people too, and a single 40 KB line is not something
# any editor or diff shows usefully; the shape stays strict enough to check by reading the
# first two lines and the last.
#
# Every value arrives here as a raw JSON span and is printed as it came. The indentation is
# outside the spans, so nothing inside a string is touched — the text of the prompt is the
# provider's own bytes on either side of this change.
mj_capture_record() {
  local i=1 k v
  for k in $MJ_CAPTURE_FIELDS; do
    eval "v=\${$i}"; i=$((i + 1))
    printf '%s\t%s\n' "$k" "$v"
  done | mj_capture_emit
}

# key<TAB>raw-json-value lines to a pretty object. One place decides the indentation and
# where the commas go, so the writer and the reformatter cannot disagree about the shape the
# doctrine then checks.
mj_capture_emit() {
  awk -F'\t' '{ k[NR] = $1; v[NR] = $2 }
    END { printf "{\n"
          for (i = 1; i <= NR; i++) printf "  \"%s\": %s%s\n", k[i], v[i], (i < NR ? "," : "")
          printf "}\n" }'
}

# Is this file already the shape above? Cheap enough to ask of every record on every render.
mj_capture_is_pretty() {
  [ "$(sed -n '1p' "$1" 2>/dev/null)" = '{' ] || return 1
  sed -n '2p' "$1" 2>/dev/null | grep -qF "\"schema\": \"$MJ_CAPTURE_SCHEMA\"," || return 1
  [ "$(tail -n 1 "$1" 2>/dev/null)" = '}' ]
}

# ---------------------------------------------------------------- the rendering
# The Markdown beside a record, in one shape every rendering has: the closed field set as
# YAML front matter, the same fields again as a table a person reads, and the prompt last,
# under `## PROMPT`, fenced.
#
# Front matter and table are the same fields twice on purpose. One is what a tool reads and
# the other is what a person reads, they are written by one function out of one record in
# one pass, and neither is edited by hand — so they cannot drift, which is the only thing
# wrong with saying something twice.
#
# The prompt goes last because it is the only field with no bound on its size, and it is
# fenced with a run of backticks longer than any run inside it. A fixed ``` fence would be
# a claim about the prompt's content that capture is in no position to make: prompts quote
# code, and a prompt that opens a fence of its own would otherwise end the block early and
# spill into the document. Sizing the fence to the content is the one way to keep the
# rendering faithful without touching a byte of the prompt.
#
# Written to a temporary name in the same directory and moved into place, so a reader never
# opens a half-written rendering and a second run cannot interleave with a first.
mj_capture_md() { printf '%s' "${1%.json}.md"; }

mj_capture_render_one() {
  local rec="$1" md scan body tmp k v rc=0 fence
  md="$(mj_capture_md "$rec")"
  scan="$(mktemp "${TMPDIR:-/tmp}/mj.render.XXXXXX")" || return 1
  body="$scan.body"
  if ! awk -f "$MJ_LIB_DIR/json_scan.awk" < "$rec" > "$scan" 2>/dev/null; then
    rm -f "$scan"; return 1
  fi
  # A record an older version wrote on one line is reformatted here rather than left as a
  # second valid shape in the archive. It is not a rewrite: every value is the raw span the
  # scan just read out of the file, printed back in the same order, so the bytes inside the
  # strings — the prompt above all — cross this untouched. Only the whitespace between them
  # is this tool's.
  if ! mj_capture_is_pretty "$rec"; then
    if mj_capture_reformat "$rec" "$scan" > "$rec.part" 2>/dev/null && [ -s "$rec.part" ]; then
      mv "$rec.part" "$rec" 2>/dev/null || rm -f "$rec.part"
    else rm -f "$rec.part"; fi
  fi

  v="$(mj_capture_raw "$scan" text)"
  if [ -n "$v" ] && [ "$v" != null ]; then mj_capture_decode "$v" > "$body" 2>/dev/null || rc=1
  else : > "$body"; fi
  [ "$rc" = 0 ] || { rm -f "$scan" "$body"; return 1; }
  fence="$(mj_capture_fence "$body")"

  tmp="$md.part"
  {
    printf -- '---\n'
    for k in $MJ_CAPTURE_FIELDS; do
      [ "$k" = text ] && continue
      if [ "$k" = started_at ]; then v="$(mj_capture_raw "$scan" "$MJ_CAPTURE_STARTED")"
      else v="$(mj_capture_raw "$scan" "$k")"; fi
      [ -n "$v" ] || v=null
      printf '%s: %s\n' "$k" "$(mj_capture_yaml "$v")"
    done
    # a field the schema declares and this record happens to carry; absent stays absent
    for k in $MJ_CAPTURE_OPTIONAL; do
      v="$(mj_capture_raw "$scan" "$k")"
      [ -n "$v" ] && [ "$v" != null ] && printf '%s: %s\n' "$k" "$(mj_capture_yaml "$v")"
    done
    printf 'record: %s\n' "$(mj_capture_yaml "\"$(mj_json_esc "$(basename "$rec")")\"")"
    printf -- '---\n\n'

    printf '# Prompt — %s\n\n' "$(mj_capture_when "$(mj_capture_plain "$scan" "$MJ_CAPTURE_STARTED")")"
    printf '| | |\n|---|---|\n'
    mj_capture_row Started    "$(mj_capture_plain "$scan" "$MJ_CAPTURE_STARTED")"
    # every row below is omitted when the record does not carry it, which for a record the
    # UserPromptSubmit hook wrote is all of them: the turn had not run when it was written
    mj_capture_row Finished   "$(mj_capture_plain "$scan" finished_at)"
    mj_capture_row Duration   "$(mj_capture_duration "$(mj_capture_plain "$scan" duration_ms)")"
    mj_capture_row Provider   "$(mj_capture_plain "$scan" provider)" "$(mj_capture_plain "$scan" event)"
    mj_capture_row Model      "$(mj_capture_plain "$scan" model)" "$(mj_capture_plain "$scan" effort)"
    mj_capture_row Session    "$(mj_capture_plain "$scan" session)"
    mj_capture_row Prompt     "$(mj_capture_plain "$scan" id)"
    mj_capture_row Source     "$(mj_capture_plain "$scan" source)"
    mj_capture_row Repository "$(mj_capture_plain "$scan" repository)"
    mj_capture_row Branch     "$(mj_capture_plain "$scan" branch)" "$(mj_capture_plain "$scan" head | cut -c1-7)"
    mj_capture_row Directory  "$(mj_capture_plain "$scan" cwd)"
    mj_capture_row Schema     "$(mj_capture_plain "$scan" schema)" "$(mj_capture_schema_stem "$(mj_capture_plain "$scan" schema)").{schema.json,proto}"
    mj_capture_row Record     "$(basename "$rec")"
    printf '\n## PROMPT\n\n%s\n' "$fence"
    cat "$body"
    # a prompt that does not end in a newline must not put the closing fence on its line
    [ -s "$body" ] && [ -n "$(tail -c 1 "$body")" ] && printf '\n'
    printf '%s\n' "$fence"
  } > "$tmp" 2>/dev/null || rc=1
  rm -f "$scan" "$body"
  [ "$rc" = 0 ] || { rm -f "$tmp"; return 1; }
  mv "$tmp" "$md" 2>/dev/null || { rm -f "$tmp"; return 1; }
  return 0
}

# The record's own fields, re-emitted in the declared order. A field the file does not carry
# is written as null rather than dropped: the set is closed, and a reader may rely on it.
mj_capture_reformat() {
  local scan="$2" k v
  # json_scan returns scalars and skips nested values, so a record carrying an object — the
  # tokens, context, meta or output the schema declares — cannot be rebuilt from a scan of
  # it. Reformatting one would silently drop the very fields that were expensive to observe,
  # so a record this cannot account for in full is left exactly as it is. The shape finding
  # still names it; a person decides.
  mj_capture_accounted "$scan" "$1" || return 1
  { for k in $MJ_CAPTURE_FIELDS; do
      if [ "$k" = started_at ]; then v="$(mj_capture_raw "$scan" "$MJ_CAPTURE_STARTED")"
      else v="$(mj_capture_raw "$scan" "$k")"; fi
      [ -n "$v" ] || v=null
      printf '%s\t%s\n' "$k" "$v"
    done
    for k in $MJ_CAPTURE_OPTIONAL; do
      v="$(mj_capture_raw "$scan" "$k")"
      [ -n "$v" ] && [ "$v" != null ] && printf '%s\t%s\n' "$k" "$v"
    done; } | mj_capture_emit
}

# Is every member the scan found one this knows how to write back? A key it does not know,
# or a member it could not read because the value was nested, means the answer is no.
mj_capture_accounted() {
  local scan="$1" rec="$2" known k
  known=" $MJ_CAPTURE_FIELDS ts $MJ_CAPTURE_OPTIONAL "
  for k in $(cut -f1 "$scan" 2>/dev/null); do
    case "$known" in *" $k "*) ;; *) return 1 ;; esac
  done
  # a nested member is skipped by the scanner rather than reported, so its absence from the
  # scan is the only trace of it: if the file names a key the scan does not carry, stop
  for k in $(grep -oE '"[a-z_]+" *:' "$rec" 2>/dev/null | tr -d '" :'); do
    grep -q "^$k	" "$scan" || return 1
  done
  return 0
}

# The stem the files describing a schema share, derived from the identifier and never looked
# up. An extension picks the projection: .schema.json for the record, .proto for the document.
mj_capture_schema_stem() {
  local id="${1:-$MJ_CAPTURE_SCHEMA}" ns ver
  ns="${id%%/*}"; ver="${id#*/}"
  [ "$ns" != "$id" ] || return 1
  printf '%s/%s/%s' "$MJ_CAPTURE_SCHEMA_DIR" "$(printf '%s' "$ns" | tr '.' '/')" "$ver"
}

mj_capture_schema_path() {
  local stem
  stem="$(mj_capture_schema_stem "${1:-$MJ_CAPTURE_SCHEMA}")" || return 1
  printf '%s.%s' "$stem" "${2:-schema.json}"
}

# Milliseconds as something a person reads. Empty in, empty out, so the row disappears.
mj_capture_duration() {
  local ms="$1"
  [ -n "$ms" ] || return 0
  case "$ms" in *[!0-9]*) printf '%s' "$ms"; return 0 ;; esac
  if [ "$ms" -lt 1000 ]; then printf '%s ms' "$ms"
  elif [ "$ms" -lt 60000 ]; then printf '%s.%s s' "$((ms / 1000))" "$(((ms % 1000) / 100))"
  else printf '%sm %ss' "$((ms / 60000))" "$(((ms % 60000) / 1000))"; fi
}

# A fence longer than the longest run of backticks the text contains, and never shorter
# than three. This is what lets the prompt be quoted verbatim: CommonMark closes a fenced
# block only on a run at least as long as the one that opened it, so nothing inside can
# end it early.
mj_capture_fence() {
  local n
  n="$(awk '{ line = $0
               while (match(line, /`+/)) {
                 if (RLENGTH > m) m = RLENGTH
                 line = substr(line, RSTART + RLENGTH)
               } }
             END { print m + 0 }' "$1" 2>/dev/null)"
  [ -n "$n" ] || n=0
  [ "$n" -lt 3 ] && n=3 || n=$((n + 1))
  printf '%*s' "$n" '' | tr ' ' '`'
}

# One row of the table, or nothing when the record does not carry the field: a row reading
# "null" tells a person less than an absent row does. A second value is shown beside the
# first, which is how provider/event and branch/head read as one fact rather than two.
mj_capture_row() {
  local label="$1" a="$2" b="${3:-}"
  [ -n "$a" ] || return 0
  if [ -n "$b" ]; then printf '| **%s** | `%s` · `%s` |\n' "$label" "$a" "$b"
  else printf '| **%s** | `%s` |\n' "$label" "$a"; fi
}

# A field of the record as plain text: empty when it is absent or null, so a caller can
# test it rather than compare against the word "null".
mj_capture_plain() {
  local v
  v="$(mj_capture_raw "$1" "$2")"
  [ -n "$v" ] && [ "$v" != null ] || return 0
  # a number or a boolean is already what it says; only a string has escapes to undo
  case "$v" in \"*) mj_capture_decode "$v" | tr -d '\n\r' ;; *) printf '%s' "$v" ;; esac
}

# The timestamp as something a person reads at a glance, and the timestamp itself when it
# is not the shape this expects — a rendering never invents a time it cannot derive.
mj_capture_when() {
  case "$1" in
    ????-??-??T??:??:??Z) printf '%s %s UTC' "${1%%T*}" "$(printf '%s' "${1#*T}" | tr -d 'Z')" ;;
    '') printf 'an unrecorded time' ;;
    *) printf '%s' "$1" ;;
  esac
}

# A raw JSON string span as the bytes it stands for. LC_ALL=C so that what \u becomes is
# decided by the decoder and not by the locale the hook happened to run in.
mj_capture_decode() { printf '%s' "$1" | LC_ALL=C awk -f "$MJ_LIB_DIR/json_unesc.awk" 2>/dev/null; }

# A raw JSON span as a YAML scalar. null and numbers stand as they are; a string is decoded
# and single-quoted, which needs no escape but the quote itself. Only the front matter goes
# through this, and every field in it is one line by construction, so a newline that somehow
# reached one is dropped rather than allowed to end the document early — the prompt, which
# is the thing that may contain anything, never comes this way.
mj_capture_yaml() {
  local v="$1" d
  [ "$v" = null ] && { printf 'null'; return 0; }
  case "$v" in
    \"*) d="$(mj_capture_decode "$v" | tr -d '\n\r')"
         printf "'%s'" "$(printf '%s' "$d" | sed "s/'/''/g")" ;;
    *)   printf '%s' "$v" ;;
  esac
}

# ---------------------------------------------------------------- capture render
# Rebuild the renderings the archive is missing. This exists because the pair is enforced:
# a rule that can fail needs a command that repairs it, and the repair is always possible
# because the record is the source and it is still there. It writes no record and reads no
# payload — an archive with nothing missing is left untouched, and --force is for a change
# to the rendering itself, not for anything a hook does.
mj_capture_render() {
  local force=0 dir rel f md n=0 bad=0
  while [ $# -gt 0 ]; do
    case "$1" in
      --force) force=1; shift ;;
      *) mj_die "$MJ_EX_USAGE" "capture render: unknown option $1" ;;
    esac
  done
  mj_require_repo
  dir="$(mj_capture_dir)"; rel="$(mj_rel "$dir")"
  [ -d "$dir" ] || { printf 'no archive in %s; nothing to render\n' "$rel"; return 0; }
  for f in "$dir"/*.json; do
    [ -e "$f" ] || break
    md="$(mj_capture_md "$f")"
    [ "$force" = 0 ] && [ -e "$md" ] && continue
    if mj_capture_render_one "$f"; then n=$((n + 1))
    else bad=$((bad + 1)); mj_err "capture render: cannot render $(basename "$f")"; fi
  done
  printf '%d rendering(s) written into %s\n' "$n" "$rel"
  [ "$bad" = 0 ] || return "$MJ_EX_INTERNAL"
  return 0
}

# Did a person write this? False for a payload whose declared origin is not one the adapter
# calls a person, and for one whose text opens with a marker the provider injects.
mj_capture_is_person() {
  local p="$1" src="$2" text="$3" who mark
  who="$(mj_capture_field "$p" 9)"
  if [ "$src" != null ] && [ -n "$who" ]; then
    src="${src#\"}"; src="${src%\"}"
    case ",$who," in *",$src,"*) ;; *) return 1 ;; esac
  fi
  text="${text#\"}"
  for mark in $(mj_capture_field "$p" 10 | tr ',' ' '); do
    case "$text" in "$mark"*) return 1 ;; esac
  done
  return 0
}

# The opening of the prompt as a file-name slug: the raw JSON string without its quotes,
# lowercased, every run of bytes outside [a-z0-9] collapsed to one hyphen, trimmed, and cut
# to a length that keeps a listing readable. JSON escapes and non-ASCII bytes fall into the
# same collapse, so a slug is a hint and never a faithful rendering — the prompt itself is
# inside the file, where nothing has touched it.
mj_capture_slug() {
  local v="$1"
  [ "$v" = null ] && return 0
  v="${v#\"}"; v="${v%\"}"
  printf '%s' "$v" | cut -c1-72 \
    | tr 'A-Z' 'a-z' | tr -c 'a-z0-9' '-' | tr -s '-' \
    | sed -e 's/^-//' -e 's/-$//' | cut -c1-48 | sed -e 's/-$//'
}

# The provider's identity in the same safe form, used as a fallback name and nowhere as a
# path: a payload can never name a directory.
mj_capture_ident() {
  local v="$1"
  [ "$v" = null ] && return 0
  v="${v#\"}"; v="${v%\"}"
  printf '%s' "$v" | tr -c 'A-Za-z0-9._-' '-' | tr -s '-' | cut -c1-64 | sed -e 's/^-//' -e 's/-$//'
}

# The raw value of the first candidate key the payload carries, or empty when it carries
# none. The candidates are tried in the order the adapter declares them, so a provider that
# adds a name keeps working and one that renames a field is a logged fact, not a silence.
mj_capture_raw() {
  local scan="$1" k
  for k in $(printf '%s' "$2" | tr ',' ' '); do
    awk -F'\t' -v k="$k" '$1 == k { sub(/^[^\t]*\t/, ""); print; found = 1; exit } END { exit !found }' "$scan" && return 0
  done
  return 0
}


# ---------------------------------------------------------------- capture state
# One word for what is true about a provider here, and a reason. The words are distinct on
# purpose: a provider with no observable event, one this repository does not wire, one that
# names a hook this tool did not write, one wired but silent, and one proven to capture are
# five different facts, and collapsing them into a generic pass is what makes a diagnostic
# worthless.
#
#   unsupported   no adapter: no documented event hands a command the prompt before the model
#   unconfigured  an adapter exists, but this repository does not wire it
#   named         the configuration declares a hook, but not the shim this tool wrote, so
#                 what it runs has not been proven
#   wired         the shim is in place and executable, but a payload through it produced
#                 no record
#   verified      the shim is in place and a synthetic payload through it produced a record
#
# Prints: <state><tab><reason>
mj_capture_state() {
  local p="$1" cfg shim rel event
  mj_capture_adapter "$p" >/dev/null 2>&1 || { printf 'unsupported\tno adapter: no documented event delivers a prompt to a command before the model runs\n'; return 0; }
  cfg="$MJ_ROOT/$(mj_capture_field "$p" 2)"; shim="$(mj_capture_shim "$p")"
  rel="$(mj_capture_shim_rel "$p")"; event="$(mj_capture_field "$p" 3)"
  [ -f "$cfg" ]                || { printf 'unconfigured\t%s does not exist (run: majordomus capture install)\n' "$(mj_capture_field "$p" 2)"; return 0; }
  grep -qF "$event" "$cfg"     || { printf 'unconfigured\t%s declares no %s hook (run: majordomus capture install)\n' "$(mj_capture_field "$p" 2)" "$event"; return 0; }
  grep -qF "$rel" "$cfg"       || { printf 'named\t%s declares %s but does not name %s\n' "$(mj_capture_field "$p" 2)" "$event" "$rel"; return 0; }
  [ -f "$shim" ]               || { printf 'named\t%s names %s, which does not exist\n' "$(mj_capture_field "$p" 2)" "$rel"; return 0; }
  [ -x "$shim" ]               || { printf 'named\t%s is not executable, so the provider cannot run it\n' "$rel"; return 0; }
  if mj_capture_selftest "$p"; then printf 'verified\t%s is wired, and a synthetic payload through it produced one record and its rendering\n' "$rel"
  else printf 'wired\t%s is in place but a synthetic payload through it produced no record and rendering\n' "$rel"; fi
}

# Run the shim the way the provider would, with a synthetic payload and an archive of its
# own, and read the record back. This is the difference between a hook that is declared and
# one that captures. The shim is a known path this tool wrote; nothing evaluates a command
# string taken from a configuration file.
mj_capture_selftest() {
  local p="$1" tmp rc=0
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.selftest.XXXXXX")"
  printf '{"%s":"majordomus self test","%s":"selftest-%s","%s":"user"}' \
    "$(mj_capture_first "$p" 6)" "$(mj_capture_first "$p" 4)" "$$" "$(mj_capture_first "$p" 7)" \
    | MJ_CAPTURE_DIR="$tmp" "$(mj_capture_shim "$p")" >/dev/null 2>&1 || rc=1
  # both halves, because both are what the hook is supposed to leave behind: a shim that
  # writes a record and no rendering is wired but not doing the whole of its job.
  [ "$rc" = 0 ] && { grep -qF 'majordomus self test' "$tmp"/*.json 2>/dev/null || rc=1; }
  [ "$rc" = 0 ] && { grep -qF 'majordomus self test' "$tmp"/*.md   2>/dev/null || rc=1; }
  rm -rf "$tmp"
  return "$rc"
}

# ---------------------------------------------------------------- capture install
# Writes the shim and the provider's configuration, and refuses rather than overwriting
# either: a hook someone else wrote is not this tool's to replace.
#
# The shim finds the repository from its own location rather than from the environment or
# the working directory. The provider substitutes its project-directory placeholder into
# the command string textually, so the shim is invoked by absolute path but inherits no
# variable that names the repository, and the working directory it runs in is not part of
# any contract this tool can rely on.
mj_capture_install() {
  local p="" providers one rc=0
  while [ $# -gt 0 ]; do
    case "$1" in
      --provider) p="${2:-}"; shift 2 ;;
      --provider=*) p="${1#--provider=}"; shift ;;
      *) mj_die "$MJ_EX_USAGE" "capture install: unknown option $1" ;;
    esac
  done
  mj_require_installed
  providers="$(mj_capture_providers)"
  [ -n "$p" ] && { mj_capture_adapter "$p" >/dev/null 2>&1 || mj_die "$MJ_EX_USAGE" "capture install: no adapter for '$p' (have: $(printf '%s' "$providers" | tr '\n' ' '))"; providers="$p"; }
  for one in $providers; do mj_capture_install_one "$one" || rc=$?; done
  return "$rc"
}

mj_capture_install_one() {
  local p="$1" cfg shim rel event
  cfg="$MJ_ROOT/$(mj_capture_field "$p" 2)"; shim="$(mj_capture_shim "$p")"
  rel="$(mj_capture_shim_rel "$p")"; event="$(mj_capture_field "$p" 3)"
  mkdir -p "$(dirname "$shim")" "$(dirname "$cfg")"

  if [ -f "$shim" ]; then
    mj_info capture "$rel" "already present; left as it is"
  else
    { printf '#!/bin/sh\n'
      printf '# The %s hook: it runs before the model is given the prompt, and appends that\n' "$event"
      printf '# prompt to .ai/local/prompts/. Written by `majordomus capture install`.\n#\n'
      printf '# The repository is derived from this file: the provider substitutes its project\n'
      printf '# directory into the command string textually, so nothing in the environment names\n'
      printf '# the repository here, and the working directory is not contracted.\n#\n'
      printf '# It must never reject a prompt, so every path out of it is exit 0 or the exit of a\n'
      printf '# command that is itself contracted never to return 2.\n'
      printf 'root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd) || exit 0\n'
      printf 'for mj in "$root/bin/majordomus" "$root/.majordomus/bin/majordomus"; do\n'
      printf '  [ -x "$mj" ] && break\n'
      printf '  mj=\n'
      printf 'done\n'
      printf '[ -n "$mj" ] || mj=$(command -v majordomus) || exit 0\n'
      printf 'exec "$mj" --repo "$root" capture prompt --provider %s\n' "$p"
    } > "$shim"
    chmod +x "$shim"
    mj_info capture "$rel" "written and made executable"
  fi

  if [ ! -f "$cfg" ]; then
    # The event takes no matcher and fires on every prompt, but the shape is the one every
    # event uses: a group, then the handlers inside it.
    { printf '{\n  "hooks": {\n    "%s": [\n      {\n        "matcher": "",\n        "hooks": [\n' "$event"
      printf '          {\n            "type": "command",\n            "command": "${CLAUDE_PROJECT_DIR}/%s"\n          }\n' "$rel"
      printf '        ]\n      }\n    ]\n  }\n}\n'
    } > "$cfg"
    mj_info capture "$(mj_capture_field "$p" 2)" "written with the $event hook"
    return 0
  fi
  if grep -qF "$rel" "$cfg"; then
    mj_info capture "$(mj_capture_field "$p" 2)" "already names the hook; left as it is"
    return 0
  fi
  mj_err "capture install: $(mj_capture_field "$p" 2) exists and does not name the hook; it is not this tool's to rewrite. Add to its \"hooks\" object:"
  mj_err "  \"$event\": [ { \"matcher\": \"\", \"hooks\": [ { \"type\": \"command\", \"command\": \"\${CLAUDE_PROJECT_DIR}/$rel\" } ] } ]"
  return "$MJ_EX_REFUSED"
}

# ---------------------------------------------------------------- capture status
mj_capture_status() {
  [ $# = 0 ] || mj_die "$MJ_EX_USAGE" "capture status: unknown option $1"
  mj_require_installed
  local p line state reason first=1
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":"%s","providers":[' "$MJ_CAPTURE_SCHEMA"
    for p in $(mj_capture_providers); do
      line="$(mj_capture_state "$p")"; state="${line%%	*}"; reason="${line#*	}"
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"provider":"%s","state":"%s","reason":"%s"}' "$p" "$state" "$(mj_json_esc "$reason")"
    done
    printf ']}\n'; return 0
  fi
  for p in $(mj_capture_providers); do
    line="$(mj_capture_state "$p")"; state="${line%%	*}"; reason="${line#*	}"
    printf '%-14s %-12s %s\n' "$p" "$state" "$reason"
  done
}

# ---------------------------------------------------------------- doctrine
# The archive invariants. What the wiring verifier proves is that the hook runs; this proves
# that what it wrote is what the contract allows, and that it never left the machine.
#
# The two are deliberately separate findings: a repository can capture correctly and still
# have committed an archive, and a repository can have a clean archive because nothing ever
# captured anything. One word for both would hide each behind the other.
mj_validate_prompt_capture() {
  local dir rel tracked
  dir="$(mj_capture_dir)"; rel="$(mj_rel "$dir")"

  mj_capture_schema "$rel"
  [ -d "$dir" ] || { mj_doctrine_ok capture "$rel" "no archive here; nothing has been captured into this checkout"; return 0; }

  tracked="$(mj_git ls-files -- "$rel" 2>/dev/null | head -n 3 | tr '\n' ' ')"
  if [ -n "$tracked" ]; then
    mj_doctrine_fail capture "$rel" "raw prompts are tracked by git: $tracked" "git rm --cached -r $rel"
  elif ! mj_git check-ignore -q "$dir" 2>/dev/null; then
    mj_doctrine_fail capture "$rel" "is not ignored, so a captured prompt can be committed" "grep -n '.ai/local/' .gitignore"
  else
    mj_doctrine_ok capture "$rel" "ignored and untracked"
  fi

  mj_capture_records "$dir" "$rel"
  mj_capture_pairs "$dir" "$rel"
  mj_capture_failures "$dir" "$rel"
  return 0
}

# A capture cannot fail loudly — it runs in front of a person's prompt — so it fails into a
# log, and the log is only worth writing if something reads it. A non-empty one means
# prompts were lost, which is the guarantee broken, so it is a failure and not a note. It
# clears by being deleted, deliberately, once a person has read it: it is a diagnostic, and
# unlike a record, nothing is lost by removing it.
mj_capture_failures() {
  local dir="$1" rel="$2" n log
  # a second 'local': $dir is not yet this function's own inside the first one, and the
  # path it built resolved to the caller's variable of the same name by luck
  log="$dir/.capture.log"
  [ -s "$log" ] || return 0
  n="$(grep -c . "$log" 2>/dev/null || true)"
  mj_doctrine_fail capture "$rel/.capture.log" \
    "$n capture(s) failed and their prompts were lost; the most recent: $(tail -n 1 "$log")" \
    "cat $rel/.capture.log   # then remove it once understood"
}

# Every record is written twice, under one stem: the JSON that is the record and the
# Markdown that renders it. Neither half alone is the archive this repository claims to
# keep — a directory of records nobody reads, or of renderings nothing can be rebuilt from
# — so a stem that carries one and not the other is a finding, and `capture render` closes
# the gap the only way it can be closed honestly, from the record.
#
# An .md with no .json is the opposite defect and is not repairable: nothing can reconstruct
# a record from a rendering, so it is reported for a person to remove rather than fixed.
# The identifier in every record is also the path to the file that describes it, so the
# archive is self-describing or it is not: a record naming a schema nothing defines is a
# record no one can be held to. The derivation is mechanical — `<ns>.<name>/<version>` is
# `.ai/repo/schemas/<ns>/<name>/<version>.proto` — which is the point: there is no registry
# to fall out of step with, and adding a version means adding a file at the path its own
# identifier already names.
mj_capture_schema() {
  local rel="$1" stem ext path missing="" n=0
  stem="$(mj_capture_schema_stem "$MJ_CAPTURE_SCHEMA")" || {
    mj_doctrine_fail capture "$rel" "the schema identifier '$MJ_CAPTURE_SCHEMA' is not <namespace>/<version> and names no file" "grep -n MJ_CAPTURE_SCHEMA= lib/capture.sh"
    return 0; }
  for ext in $MJ_CAPTURE_SCHEMA_EXT; do
    path="$stem.$ext"; n=$((n + 1))
    [ -f "$MJ_HOME/$path" ] || [ -f "$MJ_ROOT/$path" ] || missing="$missing $path"
  done
  if [ -n "$missing" ]; then
    mj_doctrine_fail capture "$stem" "$MJ_CAPTURE_SCHEMA names$missing and they do not exist; a record describes both its halves or it describes neither" "ls $(dirname "$stem")"
  else
    mj_doctrine_ok capture "$stem" "$n file(s) describe $MJ_CAPTURE_SCHEMA at the path the identifier derives: the record as JSON Schema, the document as protobuf"
  fi
}

mj_capture_pairs() {
  local dir="$1" rel="$2" f md missing="" orphan="" nm=0 no=0
  for f in "$dir"/*.json; do
    [ -e "$f" ] || break
    [ -e "${f%.json}.md" ] && continue
    nm=$((nm + 1)); [ "$nm" -le 3 ] && missing="$missing $(basename "$f")"
  done
  for md in "$dir"/*.md; do
    [ -e "$md" ] || break
    [ -e "${md%.md}.json" ] && continue
    no=$((no + 1)); [ "$no" -le 3 ] && orphan="$orphan $(basename "$md")"
  done
  if [ "$nm" != 0 ]; then
    mj_doctrine_fail capture "$rel" \
      "$nm record(s) have no Markdown rendering:$missing" "majordomus capture render"
  elif [ "$no" != 0 ]; then
    mj_doctrine_fail capture "$rel" \
      "$no rendering(s) have no record and cannot be rebuilt from one:$orphan" \
      "ls $rel/*.md   # remove the ones with no .json beside them"
  else
    mj_doctrine_ok capture "$rel" "every prompt is present as both a record and a rendering"
  fi
}

# Every record is one object, one member per line, opening with the schema identifier and
# carrying none of the model's half of the exchange. One awk pass over the archive, batched
# by find, so the cost is the archive's size and not a process per record.
mj_capture_records() {
  local dir="$1" rel="$2" n out model shape
  n="$(find "$dir" -maxdepth 1 -name '*.json' 2>/dev/null | wc -l | tr -d ' ')"
  [ "$n" -gt 0 ] || { mj_doctrine_ok capture "$rel" "no records yet; the archive is empty"; return 0; }
  out="$(find "$dir" -maxdepth 1 -name '*.json' -exec awk '
    FNR == 1 { seen[FILENAME] = 1; ok[FILENAME] = ($0 == "{") }
    FNR == 2 { if ($0 !~ /^  "schema": "majordomus\.prompt\/v1",$/) ok[FILENAME] = 0 }
    { last[FILENAME] = $0 }
    /"(response|completion|transcript|messages|reply|assistant)": / { if (!m[FILENAME]++) print "MODEL " FILENAME }
    END { for (f in seen) if (!ok[f] || last[f] != "}") print "SHAPE " f }
  ' {} + 2>/dev/null)"
  model="$(printf '%s' "$out" | grep -c '^MODEL ' || true)"
  shape="$(printf '%s' "$out" | grep -c '^SHAPE ' || true)"
  if [ "$model" != 0 ]; then
    mj_doctrine_fail capture "$rel" "$model record(s) carry the model's half of the exchange: $(printf '%s' "$out" | sed -n 's/^MODEL .*\///p' | head -n 3 | tr '\n' ' ')" "grep -lE '\"(response|completion|transcript|messages|reply|assistant)\":' $rel/*.json"
  elif [ "$shape" != 0 ]; then
    mj_doctrine_fail capture "$rel" "$shape record(s) are not a $MJ_CAPTURE_SCHEMA object: $(printf '%s' "$out" | sed -n 's/^SHAPE .*\///p' | head -n 3 | tr '\n' ' ')" "majordomus capture render   # reformats a record written by an older version"
  else
    mj_doctrine_ok capture "$rel" "$n record(s), $(du -sk "$dir" 2>/dev/null | awk '{ print $1 }') KiB, each an object of $MJ_CAPTURE_SCHEMA carrying the person's prompt only; nothing prunes them"
  fi
}

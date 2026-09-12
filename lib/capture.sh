#!/usr/bin/env bash
# shellcheck disable=SC2034  # MJ_DOCTRINE_SKIPPED is read by the dispatcher in doctrine.sh
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

MJ_CAPTURE_SCHEMA="majordomus.capture/v1"
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
#
# `episode`, `episode_link`, `repository_id` and `worktree_id` are the record's identity, and
# they are here rather than derivable because they stopped being derivable. A record used to
# carry the provider's own session UUID and nothing else, and the only thing that could turn
# that UUID into the canonical episode id was state/sessions-open/<uuid>.yaml — which the end
# of the episode deletes. Measured on 2026-09-11 in this repository: 974 records, 80 provider
# sessions, and no way left to say which episode 713 of those prompts belonged to. An identity
# that lives in a file somebody else will delete is not an identity, so it is written into the
# record at the moment it is still true.
#
# `episode_link` is how the episode was determined, and it is not decoration: a record whose
# episode was resolved while the episode was open is evidence, and one linked afterwards from
# a session context is an inference. A reader that cannot tell them apart cannot tell what the
# archive proves. The values are declared in MJ_CAPTURE_LINKS below.
MJ_CAPTURE_FIELDS="schema started_at provider event id session episode episode_link repository_id worktree_id source cwd repository branch head text"

# How a record came to name the episode it names, strongest evidence first:
#   open             the episode was open under that provider session when the prompt arrived
#   session-context  .ai/local/session-contexts/ records the provider session of that episode
#   ledger           the ledger's own timestamps left exactly one episode it could have been
#   orphan           no episode could be resolved, and none was invented
#   unlinked-legacy  written before a record carried its episode, and no evidence survives
# `orphan` and `unlinked-legacy` both carry a null episode. They are two words because they
# are two different facts: the first is a capture that happened outside any episode this
# checkout knew about, the second is a record that predates the field entirely. Collapsing
# them would make a migration look like a live defect for the rest of the archive's life.
MJ_CAPTURE_LINKS="open session-context ledger orphan unlinked-legacy"

# The fields the schema declares that the hook cannot fill, because at UserPromptSubmit the
# turn has not happened. They are absent from a record rather than null in it: an absent
# field says "not observed", and a null one would claim it was observed to be nothing. A
# renderer shows them when they are there, a reformat carries them through, and nothing here
# invents one. The order is the order they render in.
#
# `redacted` and the three `pruned_*` members are optional for the same reason and not the
# same cause: they say something happened to this record that does not happen to most of
# them. `redacted` names the credential shapes the writer replaced before the text was ever
# on disk, so a reader can tell "no secret was found" from "the secret is gone"; the
# `pruned_*` three are the tombstone retention leaves behind — when, how many bytes, and the
# digest of what they were — so that a pruned record still proves what it held without
# holding it. A record that was never redacted and never pruned carries none of them.
MJ_CAPTURE_OPTIONAL="finished_at duration_ms model effort tokens meta redacted pruned_at pruned_bytes pruned_sha256"

# `ts` was this field's name before it had a sibling called finished_at; mj_capture_raw takes
# candidates, so a record written under the old name still reformats and still renders.
MJ_CAPTURE_STARTED="started_at,ts"

# ---------------------------------------------------------------- secret hygiene
# A prompt is the one file in this repository whose content nobody vetted before it was
# written. People paste keys into prompts — to ask why a request 401s, to have a config file
# read back — and the archive then holds a live credential in a world-readable file for as
# long as the archive lives. Nothing else the tool writes has that shape.
#
# So the writer redacts before it persists, not after. The substitution happens on the raw
# JSON span on its way to the record, which means there is no moment at which the secret was
# on disk: not in the record, not in either rendering, not in the file name, and not in the
# diagnostics, because the slug is taken from the redacted text and the log never prints a
# value at all.
#
# The table is deliberately conservative and deliberately data. Each line is `name<TAB>ERE`,
# and only shapes that are credentials by construction are here — a token whose own prefix
# says what it is. A heuristic that redacted anything "secret-looking" would eat the prompts
# people most need to read back, and a prompt archive that mangles ordinary text is one
# nobody trusts. Two exceptions carry a name as well as a shape: an assignment of a long
# opaque value to something called a key, a token or a password, and a PEM private-key
# header, because both are unambiguous about what they are. The three casings are spelled
# out rather than matched case-insensitively: `sed -E` is what applies these, and BSD and GNU
# sed disagree about every way of asking for a case-insensitive match.
#
# What is replaced is the credential and nothing around it, so the prompt still reads as the
# sentence it was. The replacement names the pattern that fired, because "[redacted]" leaves
# a reader unable to tell a key from a password from a coincidence.
MJ_CAPTURE_SECRETS='anthropic-key	sk-ant-[A-Za-z0-9_-]{16,}
openai-key	sk-(proj-)?[A-Za-z0-9_-]{20,}
github-token	gh[pousr]_[A-Za-z0-9_]{20,}
github-pat	github_pat_[A-Za-z0-9_]{20,}
aws-access-key-id	AKIA[0-9A-Z]{16}
google-api-key	AIza[0-9A-Za-z_-]{35}
slack-token	xox[abprs]-[A-Za-z0-9-]{10,}
stripe-key	[rs]k_(live|test)_[A-Za-z0-9]{16,}
private-key-header	-----BEGIN ([A-Z]+ )?PRIVATE KEY-----
bearer-token	[Bb]earer [A-Za-z0-9._~+/=-]{20,}'

# The assignment form, kept apart because its replacement keeps the left-hand side. A value
# is redacted; the name of the thing it was assigned to is what makes the prompt readable
# afterwards, and is not itself a secret.
MJ_CAPTURE_SECRET_ASSIGN='([Aa][Pp][Ii][-_]?[Kk][Ee][Yy]|[Aa][Cc][Cc][Ee][Ss][Ss][-_]?[Tt][Oo][Kk][Ee][Nn]|[Ss][Ee][Cc][Rr][Ee][Tt][-_]?[Kk][Ee][Yy]|[Pp][Aa][Ss][Ss][Ww][Oo][Rr][Dd])([\"'\'']?[ ]*[:=][ ]*[\"'\'']?)[A-Za-z0-9._~+/-]{16,}'

# The sed script both of the above become. Built once per capture, from the table, so adding
# a pattern is a line of data and never a change to the writer.
mj_capture_secret_script() {
  local name pat
  while IFS="$(printf '\t')" read -r name pat; do
    [ -n "$pat" ] || continue
    printf 's%%%s%%[redacted:%s]%%g\n' "$pat" "$name"
  done <<EOF
$MJ_CAPTURE_SECRETS
EOF
  printf 's%%%s%%\\1\\2[redacted:assignment]%%g\n' "$MJ_CAPTURE_SECRET_ASSIGN"
}

# Redact stdin. The input and the output are both raw JSON spans: every pattern above matches
# only bytes that are their own literal in JSON — no quote, no backslash, no control byte — so
# a substitution cannot land inside an escape and cannot leave the span unparseable.
#
# The substitutions are delimited with `%` and not `/` or `|`: a credential shape contains
# slashes (a bearer token is base64) and alternations (`(live|test)`), and either byte used
# as the delimiter turns the pattern into a syntax error sed reports at run time — which, in
# a hook that must never fail, is a redaction that silently stops happening.
mj_capture_redact() {
  local script
  script="$(mktemp "${TMPDIR:-/tmp}/mj.redact.XXXXXX")" || { cat; return 0; }
  mj_capture_secret_script > "$script"
  sed -E -f "$script" 2>/dev/null || cat
  rm -f "$script"
}

# The names of the patterns that fired, comma-separated, or nothing. Read off the redacted
# text rather than from the substitution, so the record's own bytes are what it describes.
mj_capture_redacted_kinds() {
  printf '%s' "$1" | grep -oE '\[redacted:[a-z-]+\]' 2>/dev/null \
    | sed -e 's/^\[redacted://' -e 's/\]$//' | LC_ALL=C sort -u | paste -sd, - | sed 's/[[:space:]]*$//'
}

# ---------------------------------------------------------------- permissions
# 0600, because the archive is raw prompts and the session state beside it has always been
# 0600. It was 0644 here for the whole life of the archive: every prompt anyone in this
# repository ever typed was readable by every account on the machine, which is the one
# property this directory must not have. Both are set on every write, and the directory too,
# because a mode that is only set when the directory is created is a mode a single `mkdir`
# somewhere else undoes.
mj_capture_secure_dir()  { [ -d "$1" ] && chmod 700 "$1" 2>/dev/null; return 0; }
mj_capture_secure_file() { [ -e "$1" ] && chmod 600 "$1" 2>/dev/null; return 0; }

# ---------------------------------------------------------------- the episode
# Which episode this prompt belongs to, resolved through the one resolver the rest of the
# tool uses (`mj_open_session_id`, lib/common.sh) rather than a second reading of the same
# store. The provider session comes from the payload, which is stronger than the environment:
# the hook is handed the identity of the session the prompt was typed in, and a worker's
# exported variable is only the identity of whatever opened the terminal.
#
# MJ_SESSION_KEY makes the resolution strict — that episode or none — which is the whole
# point. The resolver's other two steps fall back to the environment and then to this
# checkout's pointer, and either would happily hand a prompt the episode of whichever window
# opened one last. A prompt attributed to the wrong episode is worse than a prompt attributed
# to none: the second is visibly missing, the first is quietly false.
#
# A subshell, so the strict key cannot leak into anything the caller does afterwards.
mj_capture_episode_of() {
  [ -n "${1:-}" ] || return 0
  ( MJ_SESSION_KEY="$1"; export MJ_SESSION_KEY; mj_open_session_id 2>/dev/null ) || true
}

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

# ---------------------------------------------------------------- lifecycle adapters
# The same argument, applied to the other thing a worker cannot be asked to do: mark where
# its own episode began and ended. A model that is told to open a session opens one when it
# remembers to, which is never the episode that mattered — the one that ended in a crash, a
# compaction or somebody closing the window. The provider knows, it fires an event, and the
# boundary is drawn there or it is fiction.
#
# One line per provider whose lifecycle Majordomus can follow:
#   1 provider  2 config file  3 start event  4 end event  5 start shim  6 end shim
#   7 session keys  8 start source keys  9 end reason keys  10 the end reasons that mean the
#   episode ended deliberately rather than being cut short
# Fields 7 to 9 are comma-separated candidates for the same reason the prompt adapter's
# are: a payload field is the provider's private shape, and a capture built on one assumed
# name loses everything the day it moves.
#
# Field 10 is what separates `closed` from `interrupted` in the record. Both are
# self-reported and the record says so; the difference is the one thing about an ended
# episode that changes what somebody does next, so it is read from the event rather than
# assumed. A reason outside the list — a crash, a name this table has not seen — closes the
# episode as interrupted, because the mistake of calling a cut-short episode complete is
# worse than the reverse.
#
# The third event is the one an episode boundary alone cannot see. A compaction is not the
# end of an episode — the worker keeps going — but it is the moment the conversation stops
# being a place anything is kept, and it is by far the most common way state is lost while
# work is still in progress. The provider announces it before it happens, which is the only
# moment at which anything can be written about a context that is about to be discarded.
#
# Field 13 is the one column read outside a hook. An episode belongs to the provider session
# that opened it, so a command a worker runs inside that session has to be able to say which
# open episode is its own — and only the provider can tell it, by exporting its session
# identity into the environment the worker's commands run in. The variable is declared here,
# beside the payload keys, because a provider is one thing and a second table naming the
# same providers elsewhere is how two lists come to disagree. A provider that exports
# nothing writes `-`, and its workers resolve through the pointer as everything did before.
MJ_CAPTURE_LIFECYCLE='claude-code .claude/settings.json SessionStart SessionEnd .claude/hooks/majordomus-session-start .claude/hooks/majordomus-session-end session_id,sessionId source,session_source reason,end_reason clear,logout,prompt_input_exit PreCompact .claude/hooks/majordomus-session-compact CLAUDE_CODE_SESSION_ID'

# The events an adapter line describes: the kind this tool calls it, the column holding the
# provider's own name for it, and the column holding the shim. Adding a fourth event is a
# row here and two columns there, rather than another pair of positional branches in four
# readers that can disagree about which column means what.
MJ_LIFECYCLE_COLUMNS='start 3 5
end 4 6
compact 11 12'

mj_lifecycle_adapter()   { printf '%s\n' "$MJ_CAPTURE_LIFECYCLE" | awk -v p="$1" '$1 == p { print; f = 1 } END { exit !f }'; }
mj_lifecycle_field()     { mj_lifecycle_adapter "$1" 2>/dev/null | awk -v n="$2" '{ print $n }'; }
mj_lifecycle_first()     { mj_lifecycle_field "$1" "$2" | cut -d, -f1; }
# every event kind, in the order a configuration writes them
mj_lifecycle_kinds()     { printf '%s\n' "$MJ_LIFECYCLE_COLUMNS" | awk '{ print $1 }'; }
# Which lifecycle events the model waits for. `start` injects the briefing it is about to
# read; `compact` must checkpoint before the transcript is folded. Nothing reads what `end`
# writes, and a session that is ending should not be held open to write it.
mj_lifecycle_latency()   { printf 'inline\n'; }
mj_lifecycle_column()    { printf '%s\n' "$MJ_LIFECYCLE_COLUMNS" | awk -v k="$1" -v n="$2" '$1 == k { print $n }'; }
mj_lifecycle_shim_rel()  { mj_lifecycle_field "$1" "$(mj_lifecycle_column "$2" 3)"; }
mj_lifecycle_shim()      { printf '%s/%s' "$MJ_ROOT" "$(mj_lifecycle_shim_rel "$1" "$2")"; }
mj_lifecycle_event()     { mj_lifecycle_field "$1" "$(mj_lifecycle_column "$2" 2)"; }
# The environment variables in which some provider names the session a command is running
# inside; `mj_provider_session_env` reads them to answer "which open episode is mine".
mj_lifecycle_session_vars() { printf '%s\n' "$MJ_CAPTURE_LIFECYCLE" | awk '$13 != "" && $13 != "-" { print $13 }'; }

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
    reconcile) mj_capture_reconcile "$@" ;;
    prune)   mj_capture_prune "$@" ;;
    session) mj_capture_session "$@" ;;
    install) mj_capture_install "$@" ;;
    status)  mj_capture_status "$@" ;;
    --help|-h|"") cat <<H
usage: majordomus capture prompt --provider <name>   < the provider's hook payload
       majordomus capture render [--force]
       majordomus capture reconcile [--dry-run]
       majordomus capture prune [--dry-run]
       majordomus capture session --provider <name> --event start|end  < the payload
       majordomus capture install [--provider <name>]
       majordomus capture status [--json]
  providers with an adapter: $(mj_capture_providers | tr '\n' ' ' | sed 's/ $//')
  three files per prompt, same stem, all ignored and never loaded:
    .ai/local/prompts/YYYYMMDDHHMMSS-<slug>.json  the record, the provider's spans unchanged
    .ai/local/prompts/YYYYMMDDHHMMSS-<slug>.md    that record rendered for a person to read
    .ai/local/prompts/YYYYMMDDHHMMSS-<slug>.yaml  that record rendered for a machine
  'capture render' rebuilds a missing rendering from its record; it never writes a record
  'capture session' opens and closes the execution episode from the provider's own
  lifecycle events, and writes nothing to stdout: on a start event the provider adds a
  hook's output to the model's context, and the local half of the layer is never loaded
  'capture prompt' reads one JSON object on stdin and never exits 2, because in a
  provider hook that exit code can reject the person's prompt
  'capture reconcile' links a record to its episode from evidence that still exists — the
  open-episode store, a session context, or an episode window only one provider session
  can have used — and marks the rest unlinked rather than guessing
  'capture prune' applies prompts.retention_max_days and prompts.retention_max_bytes: it
  takes the body and keeps the record, its identity and a tombstone of what was taken
H
      [ "$sub" = "" ] && return "$MJ_EX_USAGE"; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "capture: unknown subcommand '$sub' (prompt|render|reconcile|prune|session|install|status)" ;;
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
  # every capture, not only the one that created the directory: a mode set once is a mode
  # the next `mkdir -p` from somewhere else quietly replaces
  mj_capture_secure_dir "$dir"
  log="$dir/.capture.log"
  mj_capture_secure_file "$log"

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

  # Before anything else touches it, and before it is anywhere but a shell variable: the
  # credential shapes go. Everything downstream — the slug in the file name, the record, both
  # renderings — is built from the redacted span, so there is no path by which the original
  # bytes reach the disk. The log above prints the payload's key names and never a value, for
  # the same reason.
  local v_redacted kinds
  v_redacted="$(printf '%s' "$v_text" | mj_capture_redact)"
  kinds=""
  if [ "$v_redacted" != "$v_text" ]; then kinds="$(mj_capture_redacted_kinds "$v_redacted")"; fi
  v_text="$v_redacted"
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

  # The identity, resolved now because now is the only moment it is still resolvable. The
  # provider session the payload names is turned into the canonical episode id through the
  # open-episode store; when nothing there answers, the record says `orphan` and carries a
  # null episode. It is never attached to the episode the pointer happens to name: see
  # mj_capture_episode_of.
  local episode link
  episode="$(mj_capture_episode_of "$(mj_capture_ident "$v_session")")"
  if [ -n "$episode" ]; then link=open; else link=orphan; fi

  mj_capture_record \
    "\"$MJ_CAPTURE_SCHEMA\"" "\"$(mj_now)\"" "\"$provider\"" "\"$event\"" "$v_id" "$v_session" \
    "$([ -n "$episode" ] && printf '"%s"' "$(mj_json_esc "$episode")" || printf 'null')" "\"$link\"" \
    "\"$(mj_json_esc "$(mj_repository_id)")\"" "\"$(mj_worktree_id)\"" \
    "$v_source" "$v_cwd" \
    "\"$(mj_json_esc "$MJ_ROOT")\"" "\"$(mj_json_esc "$(mj_git_branch)")\"" "\"$(mj_git_head)\"" "$v_text" \
    ${kinds:+"$(printf 'redacted\t"%s"' "$kinds")"} > "$file" \
    || { mj_err "capture prompt: cannot write $file"; return "$MJ_EX_INTERNAL"; }
  mj_capture_secure_file "$file"

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
  local k v
  # The values arrive as positional parameters, one per declared field, and are consumed in
  # that order. `shift` rather than an indexed expansion, because the indexed form needs
  # eval and this repository forbids it (project.no-network-no-eval): nothing read from a
  # provider's payload may reach a shell that evaluates it.
  { for k in $MJ_CAPTURE_FIELDS; do
      if [ "$#" -gt 0 ]; then v="$1"; shift; else v=null; fi
      printf '%s\t%s\n' "$k" "$v"
    done
    # anything left is an optional member the caller already shaped as `key<TAB>raw-json`,
    # written after the closed set in the order it was passed. A member the writer had no
    # reason to emit is absent rather than null, which is what the schema means by optional.
    while [ "$#" -gt 0 ]; do printf '%s\n' "$1"; shift; done; } | mj_capture_emit
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
mj_capture_yml() { printf '%s' "${1%.json}.yaml"; }

mj_capture_render_one() {
  local rec="$1" md yml scan body tmp k v rc=0 fence
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
      # and the scan is taken again from what the record now says. The reformat can change a
      # value — the schema identifier is the one it exists to change — and the renderings
      # below are built from the scan, so rendering from the one taken before it would
      # publish the record's old answer beside its new one.
      if mv "$rec.part" "$rec" 2>/dev/null; then
        awk -f "$MJ_LIB_DIR/json_scan.awk" < "$rec" > "$scan.new" 2>/dev/null \
          && mv "$scan.new" "$scan" 2>/dev/null
        rm -f "$scan.new"
      fi
      rm -f "$rec.part"
    else rm -f "$rec.part"; fi
  fi

  v="$(mj_capture_raw "$scan" text)"
  if [ -n "$v" ] && [ "$v" != null ]; then mj_capture_decode "$v" > "$body" 2>/dev/null || rc=1
  elif [ -n "$(mj_capture_raw "$scan" pruned_at)" ]; then
    # A pruned record has no text and is not a broken record: retention took the body and
    # left the tombstone. The rendering says so in the body rather than showing an empty
    # block, because a reader who finds nothing under ## PROMPT cannot tell a prompt that
    # was pruned from a rendering that failed — and the document schema says this section is
    # never empty.
    mj_capture_tombstone "$scan" > "$body"
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
    if [ -s "$body" ] && [ -n "$(tail -c 1 "$body")" ]; then printf '\n'; fi
    printf '%s\n' "$fence"
  } > "$tmp" 2>/dev/null || rc=1
  [ "$rc" = 0 ] || { rm -f "$tmp" "$scan" "$body"; return 1; }
  mv "$tmp" "$md" 2>/dev/null || { rm -f "$tmp" "$scan" "$body"; return 1; }

  # The third rendering: the same closed field set, plus the prompt as a literal block
  # scalar. The Markdown is the person's copy and carries a table; this one is the machine's
  # and carries nothing the record does not, in a form a reader with a YAML parser and no
  # JSON parser can still take. Both are rebuilt from the record, never the other way round.
  yml="$(mj_capture_yml "$rec")"
  tmp="$yml.part"
  {
    for k in $MJ_CAPTURE_FIELDS; do
      [ "$k" = text ] && continue
      if [ "$k" = started_at ]; then v="$(mj_capture_raw "$scan" "$MJ_CAPTURE_STARTED")"
      else v="$(mj_capture_raw "$scan" "$k")"; fi
      [ -n "$v" ] || v=null
      printf '%s: %s\n' "$k" "$(mj_capture_yaml "$v")"
    done
    for k in $MJ_CAPTURE_OPTIONAL; do
      v="$(mj_capture_raw "$scan" "$k")"
      [ -n "$v" ] && [ "$v" != null ] && printf '%s: %s\n' "$k" "$(mj_capture_yaml "$v")"
    done
    printf 'record: %s\n' "$(mj_capture_yaml "\"$(mj_json_esc "$(basename "$rec")")\"")"
    # `|-` keeps every byte of the prompt and strips only the trailing newline the block
    # form adds; two spaces of indent are outside the scalar and never reach the text.
    printf 'text: |-\n'
    sed 's/^/  /' "$body"
    # an `if` and not an `&&` chain: this is the last command of the block, so its status is
    # the block's, and a prompt that already ends in a newline would report the rendering as
    # having failed after writing it correctly
    if [ -s "$body" ] && [ -n "$(tail -c 1 "$body")" ]; then printf '\n'; fi
  } > "$tmp" 2>/dev/null || rc=1
  rm -f "$scan" "$body"
  [ "$rc" = 0 ] || { rm -f "$tmp"; return 1; }
  mv "$tmp" "$yml" 2>/dev/null || { rm -f "$tmp"; return 1; }
  # The renderings hold the prompt in clear text, exactly as the record does, so they are
  # the record's mode and not the umask's. The record itself is set here too: the reformat
  # above moves a temporary over it, and a `mv` carries the temporary's mode with it.
  mj_capture_secure_file "$rec"; mj_capture_secure_file "$md"; mj_capture_secure_file "$yml"
  return 0
}

# What a rendering shows where the prompt was. Everything retention kept: when the body went,
# how many bytes it was, and the digest of those bytes — enough to recognise a prompt somebody
# still has a copy of, and not enough to reconstruct one nobody does.
mj_capture_tombstone() {
  local scan="$1" at bytes digest
  at="$(mj_capture_plain "$scan" pruned_at)"
  bytes="$(mj_capture_plain "$scan" pruned_bytes)"
  digest="$(mj_capture_plain "$scan" pruned_sha256)"
  printf '(the body was pruned by retention on %s' "${at:-an unrecorded date}"
  [ -n "$bytes" ] && printf '; it was %s byte(s)' "$bytes"
  [ -n "$digest" ] && printf '; sha256 %s' "$digest"
  printf ')\n'
  printf '\n'
  printf 'Every other field of this record is unchanged, including how it came to name its\n'
  printf 'episode. The record was never deleted and never will be; only the prompt itself is gone.\n'
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
      # The identifier written back is this version's, never the one the file arrived with.
      # `mj_capture_is_pretty` asks for the current identifier, so a record that kept an
      # older one would be reformatted on every run and never converge. Re-stamping is only
      # safe because `mj_capture_accounted` has already refused anything whose field set is
      # not this schema's: what is rewritten is the name, and the record was always this.
      if [ "$k" = schema ]; then v="\"$MJ_CAPTURE_SCHEMA\""
      elif [ "$k" = started_at ]; then v="$(mj_capture_raw "$scan" "$MJ_CAPTURE_STARTED")"
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
  local id="${1:-$MJ_CAPTURE_SCHEMA}" ns ver name
  ns="${id%%/*}"; ver="${id#*/}"
  [ "$ns" != "$id" ] || return 1
  # `<vendor>.<name>/v<n>` lives at share/schemas/<vendor>/<name>/<name>.v<n>.*, the one
  # convention the whole layer uses: the identity fixes the path, and a schema whose
  # declared identity and path disagree is refused rather than preferred one way or another.
  name="${ns##*.}"
  printf '%s/%s/%s.%s' "$MJ_CAPTURE_SCHEMA_DIR" "$(printf '%s' "$ns" | tr '.' '/')" "$name" "$ver"
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
  # The mode, on every file, on every run — before anything is decided about rendering.
  # `capture render` is the archive's repair command and the one the permission finding names,
  # and a repair that only reaches the records it happened to rewrite leaves most of an
  # archive loose. It is a chmod over a directory listing; it costs nothing to do it to all.
  mj_capture_secure_dir "$dir"
  for f in "$dir"/* ; do [ -f "$f" ] && mj_capture_secure_file "$f"; done
  for f in "$dir"/*.json; do
    [ -e "$f" ] || break
    md="$(mj_capture_md "$f")"
    # a record needs rendering when either rendering is absent, not only the Markdown:
    # skipping on the Markdown alone would leave a missing YAML half unrepairable
    # A record not in the current shape needs rendering too, renderings or not: the reformat
    # that migrates it happens inside `render_one`, so skipping on the renderings alone would
    # leave the finding standing with the command the finding names reporting success.
    [ "$force" = 0 ] && [ -e "$md" ] && [ -e "$(mj_capture_yml "$f")" ] \
      && mj_capture_is_pretty "$f" && continue
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


# ---------------------------------------------------------------- capture reconcile
# The join that used to die when the episode closed.
#
# A record carries the provider's own session UUID. Turning that into the canonical episode
# id needed state/sessions-open/<uuid>.yaml, and closing the episode deletes that file — so
# from the moment an episode ended, nothing could say which episode its prompts belonged to.
# The writer now stamps the episode into the record while it is still resolvable, which fixes
# every prompt from here on and no prompt already written. This command is for those, and for
# the live case the writer cannot cover: a prompt captured before its episode was open.
#
# It links only on evidence, in a fixed order, and says which kind of evidence it used:
#
#   1. the open-episode store             — the mapping is still there
#   2. .ai/local/session-contexts/        — the frozen context of an episode records the
#                                           provider session that opened it, and it survives
#                                           the close
#   3. the episode windows, where one and only one closed episode of this worktree contains
#      the whole span of that provider session's prompts, and no other provider session has
#      claimed it
#
# and nothing else. There is no fourth tier, and in particular no "nearest episode in time":
# a prompt attached to the wrong episode is worse than a prompt attached to none, because the
# second is visibly missing and the first is quietly false. What tier 3 will not do is exactly
# what makes it usable — it refuses the moment two episodes could both be the answer.
#
# A record no tier reaches is marked, not left blank: `unlinked-legacy` for one written before
# records carried an episode, `orphan` for one whose capture found no episode at the time. Both
# carry a null episode. Neither is repaired by guessing, and a validator that reported this
# archive as fully linked would be lying about 713 records in this repository alone.
mj_capture_reconcile() {
  local dry=0 dir rel links windows f scan ps ep lk new_ep new_link row ov changed=0 kept=0 stuck=0 refused=0 n=0
  while [ $# -gt 0 ]; do
    case "$1" in
      --dry-run) dry=1; shift ;;
      *) mj_die "$MJ_EX_USAGE" "capture reconcile: unknown option $1" ;;
    esac
  done
  mj_require_installed
  # not on the hook path, so the session machinery this needs is sourced here
  # shellcheck source=session.sh
  . "$MJ_LIB_DIR/session.sh"
  dir="$(mj_capture_dir)"; rel="$(mj_rel "$dir")"
  [ -d "$dir" ] || { printf 'no archive at %s; nothing to reconcile\n' "$rel"; return 0; }
  mj_capture_secure_dir "$dir"

  links="$(mktemp "${TMPDIR:-/tmp}/mj.links.XXXXXX")"
  windows="$(mktemp "${TMPDIR:-/tmp}/mj.win.XXXXXX")"
  mj_capture_link_table > "$links"
  mj_capture_episode_windows "$links" > "$windows"
  mj_capture_tier3_table "$dir" "$links" "$windows" >> "$links"

  ov="$(mktemp "${TMPDIR:-/tmp}/mj.ov.XXXXXX")"
  for f in "$dir"/*.json; do
    [ -e "$f" ] || break
    n=$((n + 1))
    scan="$(mktemp "${TMPDIR:-/tmp}/mj.rc.XXXXXX")"
    if ! awk -f "$MJ_LIB_DIR/json_scan.awk" < "$f" > "$scan" 2>/dev/null; then
      refused=$((refused + 1)); rm -f "$scan"; continue
    fi
    ep="$(mj_capture_raw "$scan" episode)"; lk="$(mj_capture_raw "$scan" episode_link)"
    # evidence already recorded is never recomputed: a link made while the episode was open
    # is stronger than anything this command can reconstruct afterwards
    case "$lk" in
      '"open"'|'"session-context"'|'"ledger"') kept=$((kept + 1)); rm -f "$scan"; continue ;;
    esac
    ps="$(mj_capture_ident "$(mj_capture_raw "$scan" session)")"
    new_ep=""; new_link=""
    if [ -n "$ps" ]; then
      row="$(awk -F'\t' -v k="$ps" '$1 == k { print; exit }' "$links")"
      if [ -n "$row" ]; then
        new_ep="$(printf '%s' "$row" | cut -f2)"; new_link="$(printf '%s' "$row" | cut -f3)"
      fi
    fi
    if [ -z "$new_ep" ]; then
      # no evidence. Which of the two unlinked words applies is a fact about the record, not
      # about this run: a record that already said `orphan` was written by a capture that
      # looked and found nothing, and relabelling it would erase that.
      if [ "$lk" = '"orphan"' ]; then new_link='orphan'; else new_link='unlinked-legacy'; fi
      stuck=$((stuck + 1))
    else
      changed=$((changed + 1))
    fi
    # a record already carrying exactly this answer is not rewritten
    if [ "$lk" = "\"$new_link\"" ] && { [ -z "$new_ep" ] && [ "$ep" = null ] || [ "$ep" = "\"$new_ep\"" ]; }; then
      rm -f "$scan"; continue
    fi
    if [ "$dry" = 1 ]; then rm -f "$scan"; continue; fi
    : > "$ov"
    if [ -n "$new_ep" ]; then printf 'episode\t"%s"\n' "$(mj_json_esc "$new_ep")" >> "$ov"
    else printf 'episode\tnull\n' >> "$ov"; fi
    printf 'episode_link\t"%s"\n' "$new_link" >> "$ov"
    # the two identity members a legacy record has no way to carry. They are facts about this
    # checkout, and a record reconciled somewhere else must not be stamped with them, so they
    # are written only when the record's own `repository` is this repository's root.
    if [ "$(mj_capture_raw "$scan" repository_id)" = "" ] \
       && [ "$(mj_capture_raw "$scan" repository)" = "\"$(mj_json_esc "$MJ_ROOT")\"" ]; then
      printf 'repository_id\t"%s"\n' "$(mj_json_esc "$(mj_repository_id)")" >> "$ov"
      printf 'worktree_id\t"%s"\n' "$(mj_worktree_id)" >> "$ov"
    fi
    if mj_capture_rewrite "$f" "$scan" "$ov" > "$f.part" 2>/dev/null && [ -s "$f.part" ]; then
      mv "$f.part" "$f" 2>/dev/null && mj_capture_render_one "$f" >/dev/null 2>&1
    else
      rm -f "$f.part"; refused=$((refused + 1))
      [ -n "$new_ep" ] && changed=$((changed - 1))
    fi
    rm -f "$scan"
  done
  rm -f "$links" "$windows" "$ov"
  if [ "$dry" = 1 ]; then
    printf 'reconcile --dry-run: %s record(s); %s would be linked, %s already linked, %s have no evidence and stay unlinked\n' \
      "$n" "$changed" "$kept" "$stuck"
  else
    printf 'reconcile: %s record(s); %s linked, %s already linked, %s unlinked for want of evidence' \
      "$n" "$changed" "$kept" "$stuck"
    [ "$refused" != 0 ] && printf ', %s left untouched (this writer cannot account for every member)' "$refused"
    printf '\n'
    mj_ledger_append prompts.reconciled \
      "\"records\":$n,\"linked\":$changed,\"unlinked\":$stuck,\"refused\":$refused" 2>/dev/null || true
  fi
  return 0
}

# `<provider session>\t<episode>\t<kind>` for every mapping that still exists, strongest
# first, one line per provider session — the first line for a key wins, which is what the
# lookup above relies on.
mj_capture_link_table() {
  local f ps sid
  for f in "$(mj_session_open_dir)"/*.yaml; do
    [ -f "$f" ] || break
    ps="$(sed -n 's/^provider_session: *"\{0,1\}\([^"]*\)"\{0,1\}$/\1/p' "$f" | head -n 1)"
    sid="$(sed -n 's/^session_id: //p' "$f" | head -n 1)"
    [ -n "$ps" ] && [ -n "$sid" ] && printf '%s\t%s\t%s\n' "$(mj_capture_ident "$ps")" "$sid" open
  done
  # The frozen working context of an episode names the provider session that opened it, and
  # unlike the open-episode file it is not deleted at the close. It was the only surviving
  # link for 261 of this repository's 974 records.
  for f in "$MJ_AI_LOCAL_DIR"/session-contexts/*.md; do
    [ -f "$f" ] || break
    ps="$(sed -n 's/^provider_session: *"\{0,1\}\([^"]*\)"\{0,1\}$/\1/p' "$f" | head -n 1)"
    sid="$(sed -n 's/^session_id: //p' "$f" | head -n 1)"
    [ -n "$ps" ] && [ -n "$sid" ] && printf '%s\t%s\t%s\n' "$(mj_capture_ident "$ps")" "$sid" session-context
  done
  return 0
}

# `<episode>\t<started_at>\t<closed_at>` for every closed episode of this worktree that no
# provider session already claims. An episode a mapping already names is not a candidate for
# an inference: it has an answer, and offering it as a second one would let a prompt be
# attributed to an episode that demonstrably belongs to a different provider session.
mj_capture_episode_windows() {
  local claimed="$1" f sid st cl wt
  for f in "$(mj_session_store)"/*.md; do
    [ -f "$f" ] || break
    sid="$(sed -n 's/^session_id: //p' "$f" | head -n 1)"
    st="$(sed -n 's/^started_at: //p' "$f" | head -n 1)"
    cl="$(sed -n 's/^closed_at: //p' "$f" | head -n 1)"
    wt="$(sed -n 's/^worktree_id: //p' "$f" | head -n 1)"
    [ -n "$sid" ] && [ -n "$st" ] && [ -n "$cl" ] || continue
    [ -n "$wt" ] && [ "$wt" != "$(mj_worktree_id)" ] && continue
    awk -F'\t' -v s="$sid" '$2 == s { f = 1 } END { exit f }' "$claimed" || continue
    printf '%s\t%s\t%s\n' "$sid" "$st" "$cl"
  done
  return 0
}

# Tier 3, and only where it is unambiguous: `<provider session>\t<episode>\tledger` for every
# provider session whose prompts all fall inside exactly one unclaimed episode window.
#
# The span of a provider session is the first and last prompt it produced — the ledger's own
# timestamps, read off the archive it wrote. A window that contains the whole span is a
# candidate; two candidates is no answer, and neither is none. Measured over this repository
# on 2026-09-11: 60 unresolved provider sessions, 6 unclaimed windows, and 0 sessions where
# exactly one window contained the span — the tier is implemented because the evidence might
# exist elsewhere, and it is honest here because it found nothing and claimed nothing.
mj_capture_tier3_table() {
  local dir="$1" links="$2" windows="$3" spans
  [ -s "$windows" ] || return 0
  spans="$(mktemp "${TMPDIR:-/tmp}/mj.span.XXXXXX")"
  # one pass over the archive: the session and the timestamp of every record, as the pretty
  # shape writes them. A record in some older shape contributes nothing here, which can only
  # narrow a span and therefore can only make a match less likely, never wrongly more.
  find "$dir" -maxdepth 1 -name '*.json' -exec awk '
    /^  "session": / { s = $0; sub(/^  "session": "?/, "", s); sub(/"?,?$/, "", s) }
    /^  "(started_at|ts)": / { t = $0; sub(/^  "[a-z_]*": "?/, "", t); sub(/"?,?$/, "", t) }
    FNR == 1 && NR > 1 { emit() }
    function emit() { if (s != "" && s != "null" && t != "") print s "\t" t; s = ""; t = "" }
    END { emit() }' {} + 2>/dev/null \
    | LC_ALL=C sort > "$spans"
  awk -F'\t' -v w="$windows" -v l="$links" '
    BEGIN {
      while ((getline line < w) > 0) { split(line, a, "\t"); nw++; ws[nw] = a[1]; wa[nw] = a[2]; wb[nw] = a[3] }
      close(w)
      while ((getline line < l) > 0) { split(line, a, "\t"); known[a[1]] = 1 }
      close(l)
    }
    { if (lo[$1] == "" || $2 < lo[$1]) lo[$1] = $2; if ($2 > hi[$1]) hi[$1] = $2 }
    END {
      for (s in lo) {
        if (s in known) continue
        hit = 0; which = ""
        for (i = 1; i <= nw; i++) if (wa[i] <= lo[s] && hi[s] <= wb[i]) { hit++; which = ws[i] }
        if (hit == 1) print s "\t" which "\tledger"
      }
    }' "$spans"
  rm -f "$spans"
  return 0
}

# One record, re-emitted with overrides. The closed field set in its declared order, then the
# optional members the record carries, exactly as `mj_capture_reformat` does — this is that
# function with a place to put a value the caller decided.
#
# OVERRIDES is a file of `key<TAB>raw-json` lines. A raw span, never a string to be quoted,
# because everything else in this writer is a raw span and a second convention here is how a
# quote ends up doubled inside somebody's prompt three months later.
#
# A record whose members this cannot account for in full is refused rather than rewritten,
# for the reason `mj_capture_accounted` exists: a rewrite that drops what it could not read
# is worse than no rewrite at all.
mj_capture_rewrite() {
  local rec="$1" scan="$2" ov="$3" k v
  mj_capture_accounted "$scan" "$rec" || return 1
  { for k in $MJ_CAPTURE_FIELDS; do
      v="$(mj_capture_override "$ov" "$k")"
      if [ -n "$v" ]; then :
      elif [ "$k" = schema ]; then v="\"$MJ_CAPTURE_SCHEMA\""
      elif [ "$k" = started_at ]; then v="$(mj_capture_raw "$scan" "$MJ_CAPTURE_STARTED")"
      else v="$(mj_capture_raw "$scan" "$k")"; fi
      [ -n "$v" ] || v=null
      printf '%s\t%s\n' "$k" "$v"
    done
    for k in $MJ_CAPTURE_OPTIONAL; do
      v="$(mj_capture_override "$ov" "$k")"
      [ -n "$v" ] || v="$(mj_capture_raw "$scan" "$k")"
      [ -n "$v" ] && [ "$v" != null ] && printf '%s\t%s\n' "$k" "$v"
    done; } | mj_capture_emit
}

mj_capture_override() {
  [ -n "${1:-}" ] && [ -f "$1" ] || return 0
  awk -F'\t' -v k="$2" '$1 == k { sub(/^[^\t]*\t/, ""); print; exit }' "$1"
}

# ---------------------------------------------------------------- capture prune
# Retention, for the one store that had none.
#
# The ledger, the handovers and the checkpoints rotate under a policy cap because each
# restates state that is still available elsewhere. A prompt restates nothing, which is why
# nothing pruned them and why the archive reached 20 MB here without anyone deciding it
# should. But the reason a prompt cannot be deleted is a reason about the *record* — that it
# happened, when, in which episode, under which head — and not about the bytes of the text.
# The text is the part that carries credentials and customer names, the part whose value
# decays and whose risk does not.
#
# So pruning takes the body and keeps the record. Every field survives, including how the
# record came to name its episode, and three members are added saying what was taken: when,
# how many bytes, and their digest. The archive stays a complete index of what was asked and
# when; what leaves is only what nobody can justify keeping.
#
# Two bounds, both from the policy, neither with a default written here: `prompts.retention_max_days`
# and `prompts.retention_max_bytes`. Age first, because that is the bound about exposure; then
# size, oldest body first, until the total fits.
#
# Atomic per record: the rewrite goes to a temporary beside the record and is moved over it,
# so a reader never opens a half-pruned record and an interrupted run leaves every record it
# had not reached exactly as it was.
mj_capture_prune() {
  local dry=0 dir rel days bytes cutoff total pruned=0 refused=0 n=0 f scan ov body sz
  while [ $# -gt 0 ]; do
    case "$1" in
      --dry-run) dry=1; shift ;;
      *) mj_die "$MJ_EX_USAGE" "capture prune: unknown option $1" ;;
    esac
  done
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"
  days="$(mj_pol_req prompts.retention_max_days)" || mj_die "$MJ_EX_CONTRACT" \
    "policy declares no prompts.retention_max_days; add a prompts: block (see share/skeleton/policy.yaml)"
  bytes="$(mj_pol_req prompts.retention_max_bytes)" || mj_die "$MJ_EX_CONTRACT" \
    "policy declares no prompts.retention_max_bytes; add a prompts: block (see share/skeleton/policy.yaml)"
  dir="$(mj_capture_dir)"; rel="$(mj_rel "$dir")"
  [ -d "$dir" ] || { printf 'no archive at %s; nothing to prune\n' "$rel"; return 0; }
  mj_capture_secure_dir "$dir"
  cutoff="$(mj_capture_cutoff "$days")"

  # the bodies, oldest first, with the size of each. The name is the second the prompt was
  # captured in, so the listing is already in age order and no timestamp has to be parsed
  # out of a record to decide which body goes first.
  local list; list="$(mktemp "${TMPDIR:-/tmp}/mj.prune.XXXXXX")"
  ov="$(mktemp "${TMPDIR:-/tmp}/mj.pov.XXXXXX")"
  total=0
  for f in "$dir"/*.json; do
    [ -e "$f" ] || break
    n=$((n + 1))
    sz="$(mj_capture_body_bytes "$f")"
    [ "$sz" = 0 ] && continue                       # already pruned, or empty
    total=$((total + sz))
    printf '%s\t%s\n' "$f" "$sz" >> "$list"
  done

  # 1. age. 2. size, oldest first, until what is left fits.
  local keep="$total" over line file
  while IFS="$(printf '\t')" read -r file sz; do
    [ -n "$file" ] || continue
    over=0
    [ "$(basename "$file" | cut -c1-14)" \< "$cutoff" ] && over=1
    [ "$over" = 0 ] && [ "$keep" -gt "$bytes" ] && over=1
    [ "$over" = 1 ] || continue
    if [ "$dry" = 0 ]; then
      mj_capture_prune_one "$file" "$ov" || { refused=$((refused + 1)); continue; }
    fi
    keep=$((keep - sz)); pruned=$((pruned + 1))
  done < "$list"
  rm -f "$list" "$ov"

  if [ "$dry" = 1 ]; then
    printf 'prune --dry-run: %s record(s), %s byte(s) of body; %s would be pruned, leaving %s byte(s) (cap %s, %s day(s))\n' \
      "$n" "$total" "$pruned" "$keep" "$bytes" "$days"
  else
    printf 'prune: %s record(s); %s body(ies) pruned, %s byte(s) left of cap %s, older than %s day(s) taken' \
      "$n" "$pruned" "$keep" "$bytes" "$days"
    [ "$refused" != 0 ] && printf ', %s refused (this writer cannot account for every member)' "$refused"
    printf '\n'
    [ "$pruned" = 0 ] || mj_ledger_append prompts.pruned \
      "\"records\":$n,\"pruned\":$pruned,\"bytes_left\":$keep" 2>/dev/null || true
  fi
  return 0
}

# The compact stamp a record's name must sort at or after to survive the age bound. `date -v`
# on BSD and `date -d` on GNU; a system that answers neither prunes on size alone rather than
# on a cutoff nobody computed, because an unbounded cutoff would take the whole archive.
mj_capture_cutoff() {
  local d="$1" s
  s="$(date -u -v-"${d}"d +%Y%m%d%H%M%S 2>/dev/null)" || s=""
  [ -n "$s" ] || s="$(date -u -d "$d days ago" +%Y%m%d%H%M%S 2>/dev/null)" || s=""
  [ -n "$s" ] || s="00000000000000"
  printf '%s' "$s"
}

# How many bytes of prompt this record holds: the length of the raw `text` span, which is what
# pruning removes and therefore what the size bound is about. A record with no text, or one
# already pruned, is 0.
mj_capture_body_bytes() {
  local v
  v="$(sed -n 's/^  "text": \(.*\),\{0,1\}$/\1/p' "$1" 2>/dev/null | head -n 1)"
  [ -n "$v" ] && [ "$v" != null ] || { printf '0'; return 0; }
  printf '%s' "${#v}"
}

# One record, pruned. The digest is of the raw span exactly as the record holds it, so a
# person holding a copy of the prompt can be told whether it is the one that was here.
mj_capture_prune_one() {
  local rec="$1" ov="$2" scan v sz digest
  scan="$(mktemp "${TMPDIR:-/tmp}/mj.pr.XXXXXX")"
  awk -f "$MJ_LIB_DIR/json_scan.awk" < "$rec" > "$scan" 2>/dev/null || { rm -f "$scan"; return 1; }
  v="$(mj_capture_raw "$scan" text)"
  [ -n "$v" ] && [ "$v" != null ] || { rm -f "$scan"; return 1; }
  sz="${#v}"
  digest="$(printf '%s' "$v" | mj_sha256 /dev/stdin | cut -c1-16)"
  : > "$ov"
  printf 'text\tnull\n' >> "$ov"
  printf 'pruned_at\t"%s"\n' "$(mj_now)" >> "$ov"
  printf 'pruned_bytes\t%s\n' "$sz" >> "$ov"
  printf 'pruned_sha256\t"%s"\n' "$digest" >> "$ov"
  if mj_capture_rewrite "$rec" "$scan" "$ov" > "$rec.part" 2>/dev/null && [ -s "$rec.part" ]; then
    mv "$rec.part" "$rec" 2>/dev/null || { rm -f "$rec.part" "$scan"; return 1; }
    mj_capture_render_one "$rec" >/dev/null 2>&1 || true
    rm -f "$scan"; return 0
  fi
  rm -f "$rec.part" "$scan"; return 1
}

# ---------------------------------------------------------------- capture session
# The episode boundary, drawn where the provider draws it. Reads the provider's lifecycle
# payload on stdin and opens or closes the session; both are idempotent, because the events
# are: a start fires again on a resume and on a compaction, and an end fires whether or not
# anything was opened.
#
# Three properties this shares with `capture prompt`, for the same reasons. It never exits
# 2, because it runs inside a provider hook. Every failure short of "nothing can be written
# at all" is logged rather than raised, because the person's session is not this command's
# to interrupt. And what it could not do is visible afterwards, in the log the doctrine
# reads, instead of vanishing.
#
# One property it does not share: it writes nothing to stdout, ever. On a start event this
# provider adds a hook's output to the model's context, and nothing under the local half of
# the layer may be loaded into a context implicitly. Diagnostics go to stderr.
mj_capture_session() {
  local provider="" event="" scan payload psession source reason
  while [ $# -gt 0 ]; do
    case "$1" in
      --provider) provider="${2:-}"; shift 2 ;;
      --provider=*) provider="${1#--provider=}"; shift ;;
      --event) event="${2:-}"; shift 2 ;;
      --event=*) event="${1#--event=}"; shift ;;
      *) mj_err "capture session: unknown option $1"; return "$MJ_EX_MISSING" ;;
    esac
  done
  [ -n "$provider" ] || { mj_err "capture session: --provider is required"; return "$MJ_EX_MISSING"; }
  case " $(mj_lifecycle_kinds | tr '\n' ' ')" in
    *" $event "*) ;;
    *) mj_err "capture session: --event must be one of $(mj_lifecycle_kinds | paste -sd' ' -)"; return "$MJ_EX_MISSING" ;;
  esac
  mj_lifecycle_adapter "$provider" >/dev/null 2>&1 || {
    mj_err "capture session: no lifecycle adapter for provider '$provider'"
    return "$MJ_EX_MISSING"; }
  mj_require_repo 2>/dev/null || { mj_err "capture session: not in a repository"; return "$MJ_EX_MISSING"; }
  # sourced here rather than at the top of this file: the prompt path is the one a person
  # waits behind, and it needs none of the session machinery.
  # shellcheck source=session.sh
  . "$MJ_LIB_DIR/session.sh"

  # The payload is optional: an event that carries nothing still marks the boundary, and
  # refusing to open an episode because a provider sent an empty object would lose the
  # episode over a detail none of the record depends on.
  psession=""; source=""; reason=""
  if [ ! -t 0 ]; then
    payload="$(mktemp "${TMPDIR:-/tmp}/mj.ses.XXXXXX")"; scan="$payload.f"
    cat > "$payload"
    if awk -f "$MJ_LIB_DIR/json_scan.awk" < "$payload" > "$scan" 2>/dev/null; then
      psession="$(mj_capture_safe "$(mj_capture_raw "$scan" "$(mj_lifecycle_field "$provider" 7)")")"
      source="$(mj_capture_safe "$(mj_capture_raw "$scan" "$(mj_lifecycle_field "$provider" 8)")")"
      reason="$(mj_capture_safe "$(mj_capture_raw "$scan" "$(mj_lifecycle_field "$provider" 9)")")"
    elif [ -s "$payload" ]; then
      mj_session_context_log "$provider $event payload not understood; the episode boundary was drawn without it"
    fi
    rm -f "$payload" "$scan"
  fi

  # A verifier proves this command runs by driving a synthetic payload through the shim the
  # provider would run. It must not cost somebody their open episode to do it, so the
  # mutation is the one thing the dry run leaves out — the payload is still read, the
  # adapter still resolves, and what would have happened is reported.
  if [ "${MJ_SESSION_DRY_RUN:-0}" = 1 ]; then
    mj_err "capture session: would $event the episode for $provider${psession:+ (provider session $psession)}${reason:+, reason $reason}"
    return 0
  fi

  # From here on this process is that provider session and no other. Everything the event
  # does — opening, closing, the checkpoint a compaction derives, the ledger line each of
  # them appends — resolves to that episode strictly: it is found or it is absent, and the
  # pointer is never consulted. An end event that fell through to the pointer would close
  # whichever episode was opened last in this checkout, which is the defect this whole
  # change is about.
  # shellcheck disable=SC2034  # read by mj_session_here_file in lib/common.sh
  [ -n "$psession" ] && MJ_SESSION_KEY="$psession"

  # The receipt, before anything decides what to do about the event. Whatever happens after
  # this line — a guard that skips the work, a record that cannot be written, a policy that
  # does not parse — the repository knows the event arrived, from whom, and when.
  #
  # This is the line whose absence made the 2026-09-05 outage unreadable. Every adapter
  # below reports its refusals on stderr and returns 0, which is what a provider hook must
  # do; stderr is not kept, so afterwards an event that never fired and an event that fired
  # and did nothing were the same observation. Not blocking the provider and pretending the
  # work happened are different instructions, and only the first is a contract (ADR 0052).
  mj_capture_session_receipt "$provider" "$event" "$psession"

  case "$event" in
    start)   mj_capture_session_start   "$provider" "$psession" "$source" ;;
    end)     mj_capture_session_end     "$provider" "$psession" "$reason" ;;
    compact) mj_capture_session_compact "$provider" "$psession" ;;
  esac
  return 0
}

# One typed line saying an event arrived. Best-effort by construction: a receipt that could
# fail the hook would be a new way to lose an episode, which is the failure it exists to
# make visible. A ledger that cannot be written is itself reported beside the working
# contexts.
mj_capture_session_receipt() {
  local provider="$1" event="$2" psession="$3" fields
  fields="\"provider\":\"$(mj_json_esc "$provider")\",\"event\":\"$(mj_json_esc "$event")\""
  [ -n "$psession" ] && fields="$fields,\"provider_session\":\"$(mj_json_esc "$psession")\""
  mj_ledger_append provider.event.received "$fields" 2>/dev/null \
    || mj_session_context_log "$provider $event event: the receipt could not be appended to the ledger"
  return 0
}

# The other half of the receipt: the event arrived and the work it should have done did not
# complete. A receipt with neither a resulting record nor one of these beside it is a
# finding in its own right, which is what lets health see a writer that has stopped.
mj_capture_session_failed() {
  local provider="$1" event="$2" reason="$3" fields
  fields="\"provider\":\"$(mj_json_esc "$provider")\",\"event\":\"$(mj_json_esc "$event")\""
  fields="$fields,\"reason\":\"$(mj_json_esc "$reason")\""
  mj_ledger_append provider.event.failed "$fields" 2>/dev/null || true
  mj_session_context_log "$provider $event event: $reason"
  return 0
}

# The strings the provider sends are its own; nothing here lets one name a path or reach a
# shell, so they are reduced to the same safe form the prompt archive uses for an identity.
# Named apart from `mj_capture_plain` deliberately: that one reads a field out of a scan and
# takes two arguments, this one takes a value, and a shell keeps only the last definition of
# a name — so sharing one turned every row of every rendering into the scan's own path.
mj_capture_safe() {
  local v="$1"
  [ -z "$v" ] && return 0
  [ "$v" = null ] && return 0
  v="${v#\"}"; v="${v%\"}"
  printf '%s' "$v" | tr -c 'A-Za-z0-9._:@/-' '-' | tr -s '-' | cut -c1-64 | sed -e 's/^-//' -e 's/-$//'
}

# Opening the episode is half of it. The other half is that the worker which just started
# is told what the last one left, and this is the only moment at which telling it costs
# nothing: the provider adds what this event writes to standard output to the context it is
# about to build, before the worker has read anything or decided anything.
#
# That is the opposite of the contract the prompt hook runs under, where standard output
# would inject text into somebody's turn and is therefore forbidden. The difference is not
# an inconsistency: one event happens inside a conversation and must not alter it, and the
# other happens before there is one and exists to furnish it.
#
# Without this, discovery is automatic and loading is not — the episode opens, the record
# resolves, and nothing reads it unless a worker remembers to ask. That is the same
# failure the whole design is against, moved one step later.
mj_capture_session_start() {
  local provider="$1" psession="$2" source="$3" out
  set -- --if-open keep --provider "$provider"
  [ -n "$psession" ] && set -- "$@" --provider-session "$psession"
  out="$( (mj_session_start "$@") 2>&1 )" || {
    mj_capture_session_failed "$provider" start "the episode did not open: $(printf '%s' "$out" | tail -n 1)"
    mj_err "capture session: the episode did not open; see the log beside the working contexts"
    return 0; }
  mj_err "capture session: $(printf '%s' "$out" | head -n 1)${source:+ (source $source)}"

  # The briefing is best-effort and never decides the event: an episode that opened and
  # could not be described is worth more than no episode, and a hook that failed because a
  # record it wanted to quote was malformed would lose the boundary over a detail.
  # shellcheck source=derive.sh
  . "$MJ_LIB_DIR/derive.sh"
  mj_load_policy || return 0
  # The server, before the briefing: an agent is arriving, which is the one moment a server
  # nobody has started yet is owed one, and the briefing says where it stands. It travels as
  # an argument rather than as a variable the briefing reaches for: two files sharing a name
  # is a data flow no reader — and no shellcheck — can follow.
  local server; server="$(mj_capture_ensure_server "$provider")"
  # The mesh, on the same line block as the server it lives in: a declaration that says
  # `enabled: true` and a server whose mesh is not active is the drift this repository
  # paid for on 2026-09-12 (ADR 0059), and the briefing is where a worker is told before
  # its first tool call. Absent declaration, absent line: a repository with no mesh must
  # not grow a line about not having one.
  local mesh; mesh="$(mj_capture_mesh_line "$server")"
  [ -n "$mesh" ] && server="$server
Mesh: $mesh"
  [ "$(mj_pol session.briefing_on_start)" = false ] && return 0
  mj_derive_briefing "$server" 2>/dev/null || mj_session_context_log "$provider start event: the briefing could not be assembled"
  return 0
}

# Make sure the repository's shared server serves this checkout, on the start event, and
# answer one line for the briefing: where it stands. Best-effort and never load-bearing —
# nothing here decides the event — and never a build: an executable that is not there, or
# is older than its sources, is named, not compiled, because a hook is not the place to
# start a compiler and a server from stale code would answer with yesterday's tree. The
# executable's own `serve ensure` does the rest: it starts a server as a process of its own
# when none answers, waits until it is ready, and starts nothing when one already is.
mj_capture_ensure_server() {
  local provider="$1" bin share line
  [ "$(mj_pol session.ensure_server_on_start)" = false ] && { printf 'not ensured: session.ensure_server_on_start is false\n'; return 0; }
  # shellcheck source=rust_bin.sh
  . "$MJ_LIB_DIR/rust_bin.sh"
  bin="$(mj_rust_bin "$MJ_ROOT")"
  if [ ! -x "$bin" ]; then
    mj_session_context_log "$provider start event: the executable is not built; no server ensured"
    printf 'not ensured: the executable is not built (run `just build`)\n'; return 0
  fi
  if mj_rust_stale "$MJ_ROOT" "$bin"; then
    mj_session_context_log "$provider start event: the executable is older than its sources; no server ensured"
    printf 'not ensured: the executable is older than its sources (run `just build`)\n'; return 0
  fi
  share="$(mj_rust_share "$MJ_ROOT")"
  [ -n "$share" ] && export MAJORDOMUS_SHARE="$share"
  mkdir -p "$MJ_STATE_DIR/mcp" 2>/dev/null || { printf 'not ensured: %s is not writable\n' "$MJ_STATE_DIR/mcp"; return 0; }
  line="$("$bin" serve ensure --repo "$MJ_ROOT" 2>>"$MJ_STATE_DIR/mcp/ensure.log")" \
    || mj_session_context_log "$provider start event: the server did not converge: $line"
  printf '%s\n' "${line:-not ensured: serve ensure said nothing}"
}

# One line about the mesh for the briefing, when the repository declares one: `active` with
# what the server sees, `off, as declared`, or — the line this exists for — DECLARED ENABLED
# BUT NOT ACTIVE with the server's reason. Empty when nothing is declared, when no server
# was ensured, or when the executable could not answer: this is a hook, and a hook that
# fails loudly at the wrong moment is a session nobody can open.
#
# The answer comes from the executable's own `mesh doctor`, which asks the running server
# (the mesh lives in its memory) and reads the declaration from the index: no request
# leaves this file (SECURITY.md's one declared request stays the one in context.sh).
mj_capture_mesh_line() {
  local server="$1" bin share out decl
  case "$server" in ready\ http://*) ;; *) return 0 ;; esac
  decl="$(mj_git ls-files -- .ai/repo/mesh 2>/dev/null | grep -E '^\.ai/repo/mesh/[^/]+\.ya?ml$' | head -n 1)"
  [ -n "$decl" ] || return 0
  # shellcheck source=rust_bin.sh
  . "$MJ_LIB_DIR/rust_bin.sh"
  bin="$(mj_rust_bin "$MJ_ROOT")"; [ -x "$bin" ] || return 0
  share="$(mj_rust_share "$MJ_ROOT")"; [ -n "$share" ] && export MAJORDOMUS_SHARE="$share"
  out="$("$bin" mesh doctor --repo "$MJ_ROOT" --format json 2>/dev/null)" || true
  [ -n "$out" ] || return 0
  printf '%s' "$out" | jq -r '
    (.checks[] | select(.check == "declaration") | .detail) as $decl
    | (.checks[] | select(.check == "runtime")) as $rt
    | (.checks[] | select(.check == "trust")) as $tr
    | if ($decl | test("enabled=true") | not) then "off, as declared"
      elif $rt.ok then "active — " + ($rt.detail | sub("^active as [0-9a-f]+ since [^:]+: "; ""))
        + (if $tr.ok then "" else "; TRUST: " + $tr.detail end)
      else "DECLARED ENABLED BUT NOT ACTIVE — " + $rt.detail end' 2>/dev/null
  return 0
}

# A compaction discards the conversation and keeps working. Nothing about the episode ends,
# so nothing here closes it; what is recorded is a checkpoint, because the state that is
# about to stop being reachable is exactly what a checkpoint is for. It is derived, since
# the worker is not asked anything at this moment and could not answer if it were.
#
# It is skipped when there is no active task. A checkpoint is a progress note inside a task,
# and inventing one outside a task in order to have written something would put a record in
# the ledger that belongs to no work.
mj_capture_session_compact() {
  local provider="$1" psession="$2" out
  # shellcheck source=checkpoint.sh
  . "$MJ_LIB_DIR/checkpoint.sh"
  # A policy that does not parse is a failure of the repository, and `doctor` says so. It is
  # not a reason to fail here: this runs in a provider hook, where the cost of dying is the
  # episode nobody can reopen. Nothing is recorded and the reason is logged beside the
  # working contexts, which is where every other failure on this path is reported.
  mj_load_policy || {
    mj_capture_session_failed "$provider" compact "the policy does not parse; nothing recorded"
    return 0; }
  if [ "$(mj_pol session.checkpoint_on_compact)" = false ]; then
    mj_err "capture session: compaction ahead; session.checkpoint_on_compact is false, so nothing is recorded"
    return 0
  fi
  # No task guard. A compaction discards the conversation whether or not anybody declared a
  # task, and what is about to stop being reachable is worth the same either way. This
  # asked for an `active` task until ADR 0052, which meant that finishing a task turned
  # compaction checkpoints off for every episode after it — silently, for six days.
  out="$( (mj_cmd_checkpoint --derive) 2>&1 )" || {
    mj_capture_session_failed "$provider" compact "the checkpoint was not written: $(printf '%s' "$out" | tail -n 1)"
    mj_err "capture session: the checkpoint was not written; see the log beside the working contexts"
    return 0; }
  mj_err "capture session: compaction ahead${psession:+ (provider session $psession)}; checkpointed into $(printf '%s' "$out" | tail -n 1)"
  return 0
}

# Closing the envelope says what the episode produced. It does not say what the next worker
# should do, and those are different documents: the session record is a list of references
# selected from the ledger, and a handover is the continuation package a worker resumes
# from. An episode that ends with its task still active has, until now, left the first and
# not the second — so the next worker inherited an accurate index of a task nobody told it
# how to continue.
#
# It is written only when the task is still active. A task already finished or already
# handed over has said what it had to say, and a second record restating it would be the
# accumulation this design refuses.
mj_capture_session_end() {
  local provider="$1" psession="$2" reason="$3" outcome=interrupted out
  case ",$(mj_lifecycle_field "$provider" 10)," in *",$reason,"*) outcome=closed ;; esac
  # shellcheck source=handover.sh
  . "$MJ_LIB_DIR/handover.sh"
  # The same reasoning as the compaction event, with one difference: an end that cannot read
  # the policy still closes the episode. Only the continuation record is skipped, because
  # leaving a session open for ever is the worse of the two failures.
  # shellcheck source=checkpoint.sh
  . "$MJ_LIB_DIR/checkpoint.sh"
  local policy_ok=1
  mj_load_policy || { policy_ok=0; mj_capture_session_failed "$provider" end "the policy does not parse; the episode is closed without a continuation record"; }

  # A final progress record, before the continuation one. An episode that opened and closed
  # without ever being compacted used to write no checkpoint at all, which left the newest
  # one dating from whenever the last compaction happened to be — so a health check that
  # asks whether progress records are still advancing would report a stopped writer for ever
  # in a repository that simply never compacts, and a finding that never clears teaches its
  # reader to skip the report. The close is the other moment at which what the episode knows
  # stops being reachable, so it records the same thing for the same reason.
  if [ "$policy_ok" = 1 ] && [ "$(mj_pol session.checkpoint_on_compact)" != false ]; then
    out="$( (mj_cmd_checkpoint --derive) 2>&1 )" \
      || mj_capture_session_failed "$provider" end "the closing checkpoint was not written: $(printf '%s' "$out" | tail -n 1)"
  fi

  if [ "$policy_ok" = 1 ] && [ "$(mj_pol session.handover_on_end)" != false ]; then
    # The continuation record is the episode's, and it is written whether or not a task is
    # open and whatever outcome the last one reached. `--close` marks an active task handed
    # over and does nothing when there is none; `--no-task` lets the record exist without
    # one. Until ADR 0052 this whole branch was conditional on an `active` task, so a
    # repository whose last task had been handed over stopped producing the one record a
    # future worker resumes from — which is exactly what happened here on 2026-09-05.
    set -- --derive --close
    mj_load_current || set -- --derive --no-task
    out="$( (mj_cmd_handover "$@") 2>&1 )" \
      && mj_err "capture session: continuation written to $(printf '%s' "$out" | tail -n 1)" \
      || mj_capture_session_failed "$provider" end "the continuation was not written: $(printf '%s' "$out" | tail -n 1)"
  fi
  # The episode this provider session opened, named rather than resolved: an end event
  # closes its own or nothing. Before this, an end event closed whatever was open in the
  # checkout, so the second window to be shut ended the first window's episode — and the
  # first window went on writing ledger lines into an episode that had already been closed
  # and published.
  set -- --if-none ignore --outcome "$outcome"
  [ -n "$psession" ] && set -- "$@" --provider-session "$psession"
  out="$( (mj_session_close "$@" < /dev/null) 2>&1 )" || {
    mj_capture_session_failed "$provider" end "the episode did not close: $(printf '%s' "$out" | tail -n 1)"
    mj_err "capture session: the episode did not close; see the log beside the working contexts"
    return 0; }
  if [ -n "$out" ]; then mj_err "capture session: episode closed ($outcome${reason:+, reason $reason}) into $out"
  else mj_err "capture session: no open episode here${psession:+ for provider session $psession}; nothing to close"; fi
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
# The same five words describe the other aspect a provider can be wired for — the episode
# boundary — because the states are the same states: no event, not wired, wired to something
# else, in place, proven by running it. A second vocabulary for the same five facts would
# only have to be learnt twice.
#
# Prints: <state><tab><reason>
# mj_capture_state <provider> [prompt|session]
mj_capture_state() {
  [ "${2:-prompt}" = session ] && { mj_lifecycle_state "$1"; return 0; }
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

# The lifecycle aspect: whether this provider's own episode events reach Majordomus. Each
# one fails differently — a start that is not wired loses the episode and the briefing with
# it, a compaction that is not wired loses the state a discarded conversation was holding,
# an end that is not wired leaves an episode open for ever and writes no continuation — so
# every one is checked and the reason names whichever is missing.
mj_lifecycle_state() {
  local p="$1" cfg one rel
  mj_lifecycle_adapter "$p" >/dev/null 2>&1 || { printf 'unsupported\tno adapter: this provider has no documented event marking the start and end of a session\n'; return 0; }
  cfg="$MJ_ROOT/$(mj_lifecycle_field "$p" 2)"
  [ -f "$cfg" ] || { printf 'unconfigured\t%s does not exist (run: majordomus capture install)\n' "$(mj_lifecycle_field "$p" 2)"; return 0; }
  for one in $(mj_lifecycle_kinds); do
    grep -qF "$(mj_lifecycle_event "$p" "$one")" "$cfg" || {
      printf 'unconfigured\t%s declares no %s hook (run: majordomus capture install)\n' "$(mj_lifecycle_field "$p" 2)" "$(mj_lifecycle_event "$p" "$one")"; return 0; }
  done
  for one in $(mj_lifecycle_kinds); do
    rel="$(mj_lifecycle_shim_rel "$p" "$one")"
    grep -qF "$rel" "$cfg" || { printf 'named\t%s declares %s but does not name %s\n' "$(mj_lifecycle_field "$p" 2)" "$(mj_lifecycle_event "$p" "$one")" "$rel"; return 0; }
    [ -f "$MJ_ROOT/$rel" ] || { printf 'named\t%s names %s, which does not exist\n' "$(mj_lifecycle_field "$p" 2)" "$rel"; return 0; }
    [ -x "$MJ_ROOT/$rel" ] || { printf 'named\t%s is not executable, so the provider cannot run it\n' "$rel"; return 0; }
  done
  local shims; shims="$(for one in $(mj_lifecycle_kinds); do mj_lifecycle_shim_rel "$p" "$one"; done | paste -sd, - | sed 's/,/, /g')"
  if mj_lifecycle_selftest "$p"; then printf 'verified\t%s are wired, and a synthetic payload through the end shim reached the command\n' "$shims"
  else printf 'wired\t%s are in place but a synthetic payload through the end shim did not reach the command\n' "$shims"; fi
}

# Drive the end shim the way the provider would, and read what it says it would have done.
# The mutation is the one thing left out: closing somebody's open episode is not a price a
# diagnostic may charge, and everything else on the path — the shim resolving the
# repository, finding the executable, the adapter matching, the payload parsing — is
# exercised exactly as it is in a real event.
mj_lifecycle_selftest() {
  local p="$1" out rc=0
  out="$(printf '{"%s":"selftest-%s","%s":"other"}' \
    "$(mj_lifecycle_first "$p" 7)" "$$" "$(mj_lifecycle_first "$p" 9)" \
    | MJ_SESSION_DRY_RUN=1 "$(mj_lifecycle_shim "$p" end)" 2>&1)" || rc=1
  [ "$rc" = 0 ] && { printf '%s' "$out" | grep -qF "selftest-$$" || rc=1; }
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
  local p="$1" cfg rel event missing="" one
  cfg="$MJ_ROOT/$(mj_capture_field "$p" 2)"
  mkdir -p "$(dirname "$cfg")"

  mj_capture_install_shim "$p" "$(mj_capture_shim_rel "$p")" "$(mj_capture_field "$p" 3)" "capture prompt --provider $p" inline
  if mj_lifecycle_adapter "$p" >/dev/null 2>&1; then
    for one in $(mj_lifecycle_kinds); do
      mj_capture_install_shim "$p" "$(mj_lifecycle_shim_rel "$p" "$one")" "$(mj_lifecycle_event "$p" "$one")" \
        "capture session --provider $p --event $one" "$(mj_lifecycle_latency "$one")"
    done
  fi

  if [ ! -f "$cfg" ]; then
    mj_capture_config "$p" > "$cfg"
    mj_info capture "$(mj_capture_field "$p" 2)" "written with the $(mj_capture_events "$p" | sed 's/=.*//' | paste -sd, - | sed 's/,/, /g') hook(s)"
    return 0
  fi
  # A configuration this tool did not write is not this tool's to rewrite, and one it wrote
  # before this tool knew about the lifecycle events is the same file: what is missing is
  # named, one event at a time, rather than the whole object being replaced.
  for one in $(mj_capture_events "$p"); do
    event="${one%%=*}"; rel="${one#*=}"
    grep -qF "$rel" "$cfg" || missing="$missing $event=$rel"
  done
  if [ -z "$missing" ]; then
    mj_info capture "$(mj_capture_field "$p" 2)" "already names the hooks; left as it is"
    return 0
  fi
  mj_err "capture install: $(mj_capture_field "$p" 2) exists and does not name $(printf '%s' "$missing" | wc -w | tr -d ' ') of the hooks; it is not this tool's to rewrite. Add to its \"hooks\" object:"
  for one in $missing; do
    event="${one%%=*}"; rel="${one#*=}"
    mj_err "  \"$event\": [ { \"matcher\": \"\", \"hooks\": [ { \"type\": \"command\", \"command\": \"\${CLAUDE_PROJECT_DIR}/$rel\" } ] } ]"
  done
  return "$MJ_EX_REFUSED"
}

# Every event this provider has a shim for, as <event>=<shim path>, in the order they are
# written into a configuration. One list, read by the writer and by the reconciliation, so
# the two cannot disagree about what "installed" means.
mj_capture_events() {
  local p="$1" one
  printf '%s=%s\n' "$(mj_capture_field "$p" 3)" "$(mj_capture_shim_rel "$p")"
  mj_lifecycle_adapter "$p" >/dev/null 2>&1 || return 0
  for one in $(mj_lifecycle_kinds); do
    printf '%s=%s\n' "$(mj_lifecycle_event "$p" "$one")" "$(mj_lifecycle_shim_rel "$p" "$one")"
  done
}

# One shim: the provider runs it, it finds the repository from its own location, and it
# never rejects what the person is doing. Two things differ between the events: the command
# it ends in, and whether the model waits for that command.
#
# An event is `inline` when the model needs the result before it goes on — SessionStart
# injects the briefing, PreCompact must checkpoint before the transcript is folded. It is
# `async` when nothing downstream reads the result: the payload is taken from stdin, the
# work is left running, and the hook returns.
#
# Every event is `inline` today, and the asynchronous form is deliberately not selected
# anywhere. It was measured on 2026-09-10: UserPromptSubmit costs about 4 s, all of it a
# cold start of the tool, paid before the model is handed the prompt. Backgrounding the
# work removes the wait and loses the record — the provider tears down the hook's process
# group when it returns, and the child dies there, with `nohup` and without it alike. The
# fix is not a background job: it is either a durable outbox the shim appends to and
# something else drains, or a request to the shared server that is already running for this
# repository. Until one of those exists, a prompt that is captured slowly beats a prompt
# that is captured never. A synchronous shim costs a full cold start of
# the tool on the person's critical path, which on UserPromptSubmit is paid before the model
# is handed a single word of the prompt.
mj_capture_install_shim() {
  local p="$1" rel="$2" event="$3" args="$4" latency="${5:-inline}" shim body
  shim="$MJ_ROOT/$rel"
  body="$(mj_capture_shim_body "$event" "$args" "$latency")"
  # A shim this tool did not write is not this tool's to rewrite. One it did write is
  # regenerated when its shape has changed, so that a repository installed before this tool
  # knew an event could be asynchronous is repaired by the same command that installed it.
  if [ -f "$shim" ]; then
    grep -qF 'Written by `majordomus capture install`' "$shim" || {
      mj_info capture "$rel" "already present and not this tool's; left as it is"; return 0; }
    if [ "$(cat "$shim")" = "$body" ]; then
      mj_info capture "$rel" "already present; left as it is"; return 0
    fi
    printf '%s\n' "$body" > "$shim"
    chmod +x "$shim"
    mj_info capture "$rel" "rewritten ($latency)"
    return 0
  fi
  mkdir -p "$(dirname "$shim")"
  printf '%s\n' "$body" > "$shim"
  chmod +x "$shim"
  mj_info capture "$rel" "written and made executable ($latency)"
}

# The text of one shim. Kept apart from writing it so that an installed shim can be compared
# with what this tool would write today, which is what makes the rewrite above safe.
mj_capture_shim_body() {
  local event="$1" args="$2" latency="$3"
  { printf '#!/bin/sh\n'
    printf '# The %s hook: it runs where the provider fires that event, and hands the payload\n' "$event"
    printf '# to `majordomus %s`. Written by `majordomus capture install`.\n#\n' "$args"
    printf '# The repository is derived from this file: the provider substitutes its project\n'
    printf '# directory into the command string textually, so nothing in the environment names\n'
    printf '# the repository here, and the working directory is not contracted.\n#\n'
    printf '# It must never reject what the person is doing, so every path out of it is exit 0\n'
    printf '# or the exit of a command that is itself contracted never to return 2.\n'
    printf 'root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd) || exit 0\n'
    printf 'for mj in "$root/bin/majordomus" "$root/.majordomus/bin/majordomus"; do\n'
    printf '  [ -x "$mj" ] && break\n'
    printf '  mj=\n'
    printf 'done\n'
    printf '[ -n "$mj" ] || mj=$(command -v majordomus) || exit 0\n'
    if [ "$latency" = async ]; then
      printf '# Nothing downstream reads what this writes, so the person does not wait for it.\n'
      printf '# The payload is taken here and the work is left running under nohup, detached\n'
      printf '# from this shell: the provider tears down the hook\x27s process group as soon as it\n'
      printf '# returns, and a plain background job dies there with the record unwritten.\n'
      printf 'payload=$(cat)\n'
      printf 'printf %%s "$payload" | nohup "$mj" --repo "$root" %s >/dev/null 2>&1 &\n' "$args"
      printf 'exit 0\n'
    else
      printf 'exec "$mj" --repo "$root" %s\n' "$args"
    fi
  }
}

# The configuration this tool writes when there is none: every event it has a shim for, in
# one hooks object. The event takes no matcher and fires every time, but the shape is the
# one every event uses — a group, then the handlers inside it.
mj_capture_config() {
  local p="$1" one event rel first=1
  printf '{\n  "hooks": {\n'
  for one in $(mj_capture_events "$p"); do
    event="${one%%=*}"; rel="${one#*=}"
    [ "$first" = 1 ] || printf ',\n'; first=0
    printf '    "%s": [\n      {\n        "matcher": "",\n        "hooks": [\n' "$event"
    printf '          {\n            "type": "command",\n            "command": "${CLAUDE_PROJECT_DIR}/%s"\n          }\n' "$rel"
    printf '        ]\n      }\n    ]'
  done
  printf '\n  }\n}\n'
}

# ---------------------------------------------------------------- capture status
mj_capture_status() {
  [ $# = 0 ] || mj_die "$MJ_EX_USAGE" "capture status: unknown option $1"
  mj_require_installed
  local p a line state reason first=1
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":"%s","providers":[' "$MJ_CAPTURE_SCHEMA"
    for p in $(mj_capture_providers); do
      for a in prompt session; do
        line="$(mj_capture_state "$p" "$a")"; state="${line%%	*}"; reason="${line#*	}"
        [ "$first" = 1 ] || printf ','; first=0
        printf '{"provider":"%s","aspect":"%s","state":"%s","reason":"%s"}' "$p" "$a" "$state" "$(mj_json_esc "$reason")"
      done
    done
    printf ']}\n'; return 0
  fi
  # The prompt aspect keeps the provider's own name in the first column and the lifecycle
  # aspect is suffixed, because they are two wirings of one provider and a reader looking
  # for "is claude-code capturing" must not have to know that there are now two answers.
  for p in $(mj_capture_providers); do
    for a in prompt session; do
      line="$(mj_capture_state "$p" "$a")"; state="${line%%	*}"; reason="${line#*	}"
      printf '%-22s %-12s %s\n' "$([ "$a" = prompt ] && printf '%s' "$p" || printf '%s:session' "$p")" "$state" "$reason"
    done
  done
}

# ---------------------------------------------------------------- prompt continuity
# The four things that were wrong with the archive on 2026-09-11, each now a finding.
#
# 1. The join died at close. A record named the provider's session and nothing else, and the
#    only mapping to the canonical episode was a file the close deletes. 974 records, 80
#    provider sessions, 713 prompts that nothing could attribute. Every record written from
#    now on carries its episode; a record that carries neither an episode nor a word saying
#    why is the defect, and `capture reconcile` is what closes it.
# 2. Nothing prunes. The archive was 20 MB and had no bound at all, while the ledger, the
#    handovers and the checkpoints each had one.
# 3. The records were 0644. The session state beside them has always been 0600, and a raw
#    prompt is the most private thing this tool writes.
# 4. Nothing redacted. A prompt containing a credential was persisted verbatim, and stayed.
#
# The permission finding is deliberately not "some file is 0644": it is a count, with the
# repair named, because an archive of a thousand records repaired one file at a time is an
# archive nobody repairs.
mj_validate_prompt_continuity() {
  local dir rel n
  dir="$(mj_capture_dir)"; rel="$(mj_rel "$dir")"
  if [ ! -d "$dir" ]; then
    mj_doctrine_skip prompts "$rel" "no archive here; nothing has been captured into this checkout"
    MJ_DOCTRINE_SKIPPED=1; return 0
  fi
  n="$(find "$dir" -maxdepth 1 -name '*.json' 2>/dev/null | wc -l | tr -d ' ')"
  mj_capture_permissions "$dir" "$rel"
  [ "$n" -gt 0 ] || { mj_doctrine_ok prompts "$rel" "no records yet; the archive is empty"; return 0; }
  mj_capture_identity "$dir" "$rel" "$n"
  mj_capture_retention "$dir" "$rel"
  return 0
}

# Nothing under the archive is readable by anyone but its owner. The directory as well as the
# files: a 0755 directory over 0600 records still tells every account on the machine what was
# asked and when, because the file names are the openings of the prompts.
mj_capture_permissions() {
  local dir="$1" rel="$2" loose dperm spec found rc unasked=""
  dperm="$(mj_capture_mode "$dir")" || dperm=""
  [ -n "$dperm" ] || unasked="the directory's own mode (no stat here answers 'stat -c %a' or 'stat -f %Lp' with octal digits)"
  # `-perm /077` is GNU and `-perm +077` is BSD, and each rejects the other's spelling — the
  # gate runs on Linux and the development machine is a Mac, so both are asked and the first
  # that the local find accepts is the answer. A find that accepts neither reports nothing
  # rather than zero, because "no loose files" and "the question could not be asked" are
  # different answers and only one of them is a pass.
  #
  # Both spellings are asked of the directory itself, where the answer does not matter and
  # only the exit does. Neither being accepted used to leave `spec` at the BSD spelling and
  # walk with it anyway, and the walk's own failure then reached `wc -l`, which turned it
  # into a clean zero — a check reporting "no loose files" because it never looked.
  spec=""
  if find "$dir" -maxdepth 0 -perm /077 >/dev/null 2>&1; then spec=/077
  elif find "$dir" -maxdepth 0 -perm +077 >/dev/null 2>&1; then spec=+077; fi
  loose=""
  if [ -n "$spec" ]; then
    rc=0; found="$(find "$dir" -type f -perm "$spec" 2>/dev/null)" || rc=$?
    # a find that errored mid-walk has not seen the whole archive, so it has not answered
    if [ "$rc" = 0 ]; then loose="$(printf '%s' "$found" | grep -c '[^[:space:]]' || true)"; fi
  fi
  [ -n "$loose" ] || unasked="${unasked:+$unasked, and }the records' modes (no find here accepts '-perm /077' or '-perm +077', or the walk failed)"
  if [ -n "$unasked" ]; then
    # Never silently green. This environment cannot be asked the question, which is a fact
    # about the environment and not a result about the archive — and an unverifiable
    # privacy guarantee is not a satisfied one (majordomus.never-reported-is-not-green).
    mj_doctrine_fail prompts "$rel" \
      "unverified — this environment cannot be asked $unasked; whether the archive is private is unknown here, and unknown is not a pass" \
      "ls -ld $rel && ls -l $rel"
  elif [ "$loose" != 0 ]; then
    mj_doctrine_fail prompts "$rel" \
      "$loose file(s) in the prompt archive are readable beyond their owner; raw prompts are the most private thing this tool writes" \
      "chmod 700 $rel && chmod 600 $rel/*   # or: majordomus capture render, which sets both"
  elif [ -n "$dperm" ] && [ "$dperm" != 700 ]; then
    mj_doctrine_fail prompts "$rel" \
      "the archive directory is mode $dperm; the file names are the openings of the prompts" \
      "chmod 700 $rel"
  else
    mj_doctrine_ok prompts "$rel" "the archive and every record in it are readable by their owner only"
  fi
}

# The mode of one path as octal digits, or nothing and a non-zero status when this system
# cannot be asked.
#
# GNU FIRST. The ORDER is load-bearing, and this is the same trap lib/recover.sh and
# lib/common.sh already name for mtime — this was the one site left facing the other way.
# `stat -f` means two different things: on BSD it is "this format string", on GNU it is
# "report the FILE SYSTEM, not the file". Asked BSD-first, a GNU stat read `%Lp` as a
# filename, failed on it, printed the *filesystem* block for the directory to STDOUT
# anyway, and exited non-zero — so `||` ran the GNU spelling too and appended the real
# mode to that paragraph. The caller compared the whole thing against `700`, so an archive
# that was mode 700 was reported as
#
#     the archive directory is mode   File: "…/.ai/local/prompts"
#         ID: … Type: ext4  Block size: 4096 …
#     700; the file names are the openings of the prompts
#
# `doctor` therefore failed with exit 10 and `watch` drifted with exit 11 inside every
# use-case scenario that runs `init` first, on Linux only — so twelve scenarios passed on
# the Mac that wrote them and failed on every CI runner, taking `generate-site-data
# --check` and the whole `core-check` gate with them.
#
# `stat -c` is unambiguous: GNU accepts it, BSD rejects the option and falls through. The
# answer is then shape-checked rather than trusted, which is what makes that class of
# failure impossible rather than merely unlikely: anything that is not octal digits is not
# a mode, whatever printed it, and this says so with its status instead of handing a caller
# a number that is not one.
mj_capture_mode() {
  local m
  m="$(stat -c '%a' "$1" 2>/dev/null || stat -f '%Lp' "$1" 2>/dev/null || true)"
  case "$m" in ''|*[!0-7]*) return 1 ;; esac
  printf '%s' "$m"
}

# Every record says which episode it belongs to, or says in so many words that it does not.
# A record with neither is the state this whole subsystem exists to end: an archive whose
# attribution nobody can reconstruct and nobody can see is missing.
#
# An unlinked record is reported, never counted as a pass and never counted as a failure of
# the repository: the evidence for those prompts does not exist and no command can make it.
# A validator that reported this archive as fully linked would be inventing the linkage the
# migration deliberately refused to invent.
mj_capture_identity() {
  local dir="$1" rel="$2" n="$3" out silent linked unlinked bad
  out="$(find "$dir" -maxdepth 1 -name '*.json' -exec awk '
    FNR == 1 { seen[FILENAME] = 1 }
    /^  "episode_link": "/ { v = $0; sub(/^  "episode_link": "/, "", v); sub(/",?$/, "", v); link[FILENAME] = v }
    END {
      for (f in seen) {
        l = link[f]
        if (l == "") print "SILENT " f
        else if (l == "orphan" || l == "unlinked-legacy") print "UNLINKED " f
        else if (l == "open" || l == "session-context" || l == "ledger") print "LINKED " f
        else print "BAD " f " " l
      }
    }' {} + 2>/dev/null)"
  silent="$(printf '%s' "$out" | grep -c '^SILENT ' || true)"
  unlinked="$(printf '%s' "$out" | grep -c '^UNLINKED ' || true)"
  linked="$(printf '%s' "$out" | grep -c '^LINKED ' || true)"
  bad="$(printf '%s' "$out" | grep -c '^BAD ' || true)"
  if [ "$bad" != 0 ]; then
    mj_doctrine_fail prompts "$rel" \
      "$bad record(s) name a provenance that is not one of: $MJ_CAPTURE_LINKS" \
      "grep -h '\"episode_link\"' $rel/*.json | sort -u"
  elif [ "$silent" != 0 ]; then
    mj_doctrine_fail prompts "$rel" \
      "$silent record(s) say nothing about which episode they belong to; the mapping from a provider session to an episode is deleted when the episode closes, so this is unrecoverable once it is lost" \
      "majordomus capture reconcile"
  else
    mj_doctrine_ok prompts "$rel" \
      "$n record(s) each name their episode or say why they cannot: $linked linked, $unlinked unlinked with their provenance recorded"
  fi
}

# The archive is within the bounds the policy declares. Reported the way the ledger's,
# the handovers' and the checkpoints' caps are reported, and repaired by a command a person
# runs: nothing prunes behind anyone's back, and a prompt archive that shrank as a side
# effect of a hook would be the worse defect.
mj_capture_retention() {
  local dir="$1" rel="$2" days bytes total over cutoff
  # shellcheck disable=SC2034  # set by the awk pass below, read by the findings
  if ! days="$(mj_pol_req prompts.retention_max_days)"; then
    mj_doctrine_fail prompts "retention" "policy declares no prompts.retention_max_days" \
      "add a prompts: block to $(mj_rel "$MJ_POLICY_FILE"); see share/skeleton/policy.yaml"
    return 0
  fi
  if ! bytes="$(mj_pol_req prompts.retention_max_bytes)"; then
    mj_doctrine_fail prompts "retention" "policy declares no prompts.retention_max_bytes" \
      "add a prompts: block to $(mj_rel "$MJ_POLICY_FILE"); see share/skeleton/policy.yaml"
    return 0
  fi
  cutoff="$(mj_capture_cutoff "$days")"
  # One awk pass over the archive, not a process per record. This runs on every `doctor`,
  # which has a wall-time budget of three seconds, and a `sed` per record over a thousand
  # records is most of that budget spent on a number.
  local counted
  counted="$(find "$dir" -maxdepth 1 -name '*.json' -exec awk -v cut="$cutoff" '
    /^  "text": / {
      v = $0; sub(/^  "text": /, "", v); sub(/,$/, "", v)
      if (v == "null") next
      total += length(v)
      n = split(FILENAME, p, "/"); stamp = substr(p[n], 1, 14)
      if (stamp < cut) over++
    }
    END { print total + 0 "\t" over + 0 }' {} + 2>/dev/null | awk -F'\t' '
    { t += $1; o += $2 } END { print t + 0 "\t" o + 0 }')"
  total="$(printf '%s' "$counted" | cut -f1)"; over="$(printf '%s' "$counted" | cut -f2)"
  [ -n "$total" ] || total=0
  [ -n "$over" ] || over=0
  if [ "$over" != 0 ]; then
    mj_doctrine_fail prompts "retention" \
      "$over prompt body(ies) are older than the $days day(s) the policy allows" "majordomus capture prune"
  elif [ "$total" -gt "$bytes" ]; then
    mj_doctrine_fail prompts "retention" \
      "$total byte(s) of prompt bodies over the cap of $bytes" "majordomus capture prune"
  else
    mj_doctrine_ok prompts "retention" \
      "$total byte(s) of bodies under the cap of $bytes, none older than $days day(s)"
  fi
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
  local dir="$1" rel="$2" n
  local log="$dir/.capture.log"
  [ -s "$log" ] || return 0
  n="$(grep -c . "$log" 2>/dev/null || true)"
  mj_doctrine_fail capture "$rel/.capture.log" \
    "$n capture(s) failed and their prompts were lost; the most recent: $(tail -n 1 "$log")" \
    "cat $rel/.capture.log   # then remove it once understood"
}

# Every record is written three times, under one stem: the JSON that is the record, the
# Markdown that renders it for a person and the YAML that renders it for a machine with no
# JSON parser. No one of them alone is the archive this repository claims to keep — a directory of records nobody reads, or of renderings nothing can be rebuilt from
# — so a stem that carries one and not the other is a finding, and `capture render` closes
# the gap the only way it can be closed honestly, from the record.
#
# An .md or .yaml with no .json is the opposite defect and is not repairable: nothing can reconstruct
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
    [ -e "${f%.json}.md" ] && [ -e "${f%.json}.yaml" ] && continue
    nm=$((nm + 1)); [ "$nm" -le 3 ] && missing="$missing $(basename "$f")"
  done
  for md in "$dir"/*.md; do
    [ -e "$md" ] || break
    [ -e "${md%.md}.json" ] && continue
    no=$((no + 1)); [ "$no" -le 3 ] && orphan="$orphan $(basename "$md")"
  done
  if [ "$nm" != 0 ]; then
    mj_doctrine_fail capture "$rel" \
      "$nm record(s) are missing a rendering:$missing" "majordomus capture render"
  elif [ "$no" != 0 ]; then
    mj_doctrine_fail capture "$rel" \
      "$no rendering(s) have no record and cannot be rebuilt from one:$orphan" \
      "ls $rel/*.md   # remove the ones with no .json beside them"
  else
    mj_doctrine_ok capture "$rel" "every prompt is present as all three: the record, the Markdown and the YAML"
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
    FNR == 2 { if ($0 !~ /^  "schema": "majordomus\.capture\/v1",$/) ok[FILENAME] = 0 }
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
    mj_doctrine_ok capture "$rel" "$n record(s), $(du -sk "$dir" 2>/dev/null | awk '{ print $1 }') KiB, each an object of $MJ_CAPTURE_SCHEMA carrying the person's prompt only"
  fi
}

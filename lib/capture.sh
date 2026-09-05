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
# The archive is evidence, not knowledge: nothing loads it into a context, no command
# retrieves from it, and it lives under the ignored half of the AI layer.

MJ_CAPTURE_SCHEMA="majordomus.prompt/v1"
# the closed set of fields a record carries, in order; a reader may rely on it, and a field
# outside it is a defect the prompt_capture doctrine reports
MJ_CAPTURE_FIELDS="schema ts provider event id session source cwd repository branch head text"

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
    install) mj_capture_install "$@" ;;
    status)  mj_capture_status "$@" ;;
    --help|-h|"") cat <<H
usage: majordomus capture prompt --provider <name>   < the provider's hook payload
       majordomus capture install [--provider <name>]
       majordomus capture status [--json]
  providers with an adapter: $(mj_capture_providers | tr '\n' ' ' | sed 's/ $//')
  one file per prompt: .ai/local/prompts/YYYYMMDDHHMMSS-<slug>.json, ignored and never loaded
  'capture prompt' reads one JSON object on stdin and never exits 2, because in a
  provider hook that exit code can reject the person's prompt
H
      [ "$sub" = "" ] && return "$MJ_EX_USAGE"; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "capture: unknown subcommand '$sub' (prompt|install|status)" ;;
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
  if [ -n "$id_plain" ] && [ -n "$(grep -lF "\"id\":$v_id," "$dir/$day"*.json 2>/dev/null | head -n 1)" ]; then return 0; fi

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

  printf '{"schema":"%s","ts":"%s","provider":"%s","event":"%s","id":%s,"session":%s,"source":%s,"cwd":%s,"repository":"%s","branch":"%s","head":"%s","text":%s}\n' \
    "$MJ_CAPTURE_SCHEMA" "$(mj_now)" "$provider" "$event" "$v_id" "$v_session" "$v_source" "$v_cwd" \
    "$(mj_json_esc "$MJ_ROOT")" "$(mj_json_esc "$(mj_git_branch)")" "$(mj_git_head)" "$v_text" > "$file" \
    || { mj_err "capture prompt: cannot write $file"; return "$MJ_EX_INTERNAL"; }
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
  if mj_capture_selftest "$p"; then printf 'verified\t%s is wired, and a synthetic payload through it produced one record\n' "$rel"
  else printf 'wired\t%s is in place but a synthetic payload through it produced no record\n' "$rel"; fi
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
  [ "$rc" = 0 ] && { grep -qF 'majordomus self test' "$tmp"/*.json 2>/dev/null || rc=1; }
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
  mj_capture_failures "$dir" "$rel"
  return 0
}

# A capture cannot fail loudly — it runs in front of a person's prompt — so it fails into a
# log, and the log is only worth writing if something reads it. A non-empty one means
# prompts were lost, which is the guarantee broken, so it is a failure and not a note. It
# clears by being deleted, deliberately, once a person has read it: it is a diagnostic, and
# unlike a record, nothing is lost by removing it.
mj_capture_failures() {
  local dir="$1" rel="$2" log="$dir/.capture.log" n
  [ -s "$log" ] || return 0
  n="$(grep -c . "$log" 2>/dev/null || true)"
  mj_doctrine_fail capture "$rel/.capture.log" \
    "$n capture(s) failed and their prompts were lost; the most recent: $(tail -n 1 "$log")" \
    "cat $rel/.capture.log   # then remove it once understood"
}

# Every record is one file of one line: the schema identifier, the closed field set, and
# none of the model's half of the exchange. One awk pass over the archive, batched by find,
# so the cost is the archive's size and not a process per record.
mj_capture_records() {
  local dir="$1" rel="$2" n out model shape
  n="$(find "$dir" -maxdepth 1 -name '*.json' 2>/dev/null | wc -l | tr -d ' ')"
  [ "$n" -gt 0 ] || { mj_doctrine_ok capture "$rel" "no records yet; the archive is empty"; return 0; }
  out="$(find "$dir" -maxdepth 1 -name '*.json' -exec awk '
    FNR == 1 { if ($0 !~ /^\{"schema":"majordomus\.prompt\/v1",/ || $0 !~ /\}$/) print "SHAPE " FILENAME }
    FNR > 1  { if (!s[FILENAME]++) print "SHAPE " FILENAME }
    /"(response|completion|transcript|messages|reply|assistant)":/ { if (!m[FILENAME]++) print "MODEL " FILENAME }
  ' {} + 2>/dev/null)"
  model="$(printf '%s' "$out" | grep -c '^MODEL ' || true)"
  shape="$(printf '%s' "$out" | grep -c '^SHAPE ' || true)"
  if [ "$model" != 0 ]; then
    mj_doctrine_fail capture "$rel" "$model record(s) carry the model's half of the exchange: $(printf '%s' "$out" | sed -n 's/^MODEL .*\///p' | head -n 3 | tr '\n' ' ')" "grep -lE '\"(response|completion|transcript|messages|reply|assistant)\":' $rel/*.json"
  elif [ "$shape" != 0 ]; then
    mj_doctrine_fail capture "$rel" "$shape record(s) are not one line of $MJ_CAPTURE_SCHEMA: $(printf '%s' "$out" | sed -n 's/^SHAPE .*\///p' | head -n 3 | tr '\n' ' ')" "head -n 2 $rel/*.json"
  else
    mj_doctrine_ok capture "$rel" "$n record(s), $(du -sk "$dir" 2>/dev/null | awk '{ print $1 }') KiB, each one line of $MJ_CAPTURE_SCHEMA and carrying the person's prompt only; nothing prunes them"
  fi
}

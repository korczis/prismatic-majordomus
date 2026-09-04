#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_prompt:-}" ] && return 0 || MJ_LIB_prompt=1
# prompt — small reusable repository-local prompt assets.
#
# A prompt asset is a Markdown file in .majordomus/prompts/ with two required front-matter
# keys and a body. Rendering substitutes a closed, enumerated set of tokens from durable
# state. There is no templating language: no conditionals, no loops, no shell, no include.
# An unknown token is an error, exactly like an unknown configuration key, because a prompt
# that silently renders {{TSAK}} as literal text is worse than one that refuses.

# shellcheck source=handover.sh
. "$MJ_LIB_DIR/handover.sh"
# shellcheck source=decision.sh
. "$MJ_LIB_DIR/decision.sh"
# shellcheck source=context.sh
. "$MJ_LIB_DIR/context.sh"

MJ_PROMPT_INLINE_TOKENS="TASK TASK_ID PROFILE SCOPE OWNER BRANCH HEAD WORKING_TREE REPOSITORY NOW"
MJ_PROMPT_BLOCK_TOKENS="OPEN_QUESTIONS DECISIONS CHECKPOINT HANDOVER CONTEXT"

mj_cmd_prompt() {
  local sub="${1:-}"; [ $# -gt 0 ] && shift
  case "$sub" in
    list) mj_prompt_list "$@" ;;
    show) [ $# -ge 1 ] || mj_die "$MJ_EX_USAGE" "prompt show: a name is required"; mj_prompt_show "$1" ;;
    render) [ $# -ge 1 ] || mj_die "$MJ_EX_USAGE" "prompt render: a name is required"
      mj_require_installed; mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse"
      mj_prompt_render "$1" ;;
    --help|-h|"") cat <<H
usage: majordomus prompt list
       majordomus prompt show <name>
       majordomus prompt render <name>
  assets live in .majordomus/prompts/<name>.md with front matter: name (= filename), description
  render substitutes these tokens and no others:
    inline: $(printf '%s' "$MJ_PROMPT_INLINE_TOKENS" | sed 's/ /, /g')
    on a line of their own: $(printf '%s' "$MJ_PROMPT_BLOCK_TOKENS" | sed 's/ /, /g')
  an unknown token, or a block token used inline, is an error
H
      [ "$sub" = "" ] && return "$MJ_EX_USAGE"; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "prompt: unknown subcommand '$sub' (list|show|render)" ;;
  esac
}

mj_prompt_dir() { printf '%s' "$MJ_DIR/prompts"; }
mj_prompt_path() {
  case "$1" in *[!A-Za-z0-9._-]*|""|.*) mj_die "$MJ_EX_USAGE" "prompt: '$1' is not a valid asset name" ;; esac
  printf '%s/%s.md' "$(mj_prompt_dir)" "$1"
}

# validate one asset; prints nothing on success, a reason on failure
mj_prompt_validate() {
  local f="$1" fm flat rc=0 name tok known
  name="$(basename "$f" .md)"
  fm="$(mktemp "${TMPDIR:-/tmp}/mj.pf.XXXXXX")"
  if ! mj_record_front "$f" > "$fm" 2>/dev/null; then rm -f "$fm"; printf 'no front matter\n'; return 1; fi
  flat="$(mktemp "${TMPDIR:-/tmp}/mj.pg.XXXXXX")"
  if ! mj_yaml_flatten "$fm" > "$flat" 2>/dev/null; then rm -f "$fm" "$flat"; printf 'malformed front matter\n'; return 1; fi
  local unk; unk="$(mj_yaml_unknown_keys "$flat" "$MJ_BIN_DIR/../share/allow/prompt.txt" || true)"
  [ -n "$unk" ] && { printf 'unknown front-matter key(s): %s\n' "$(printf '%s' "$unk" | tr '\n' ' ')"; rc=1; }
  [ "$(mj_yget "$flat" name)" = "$name" ] || { printf 'name field "%s" does not match filename %s.md\n' "$(mj_yget "$flat" name)" "$name"; rc=1; }
  [ -n "$(mj_yget "$flat" description)" ] || { printf 'description is empty\n'; rc=1; }
  rm -f "$fm" "$flat"
  known=" $MJ_PROMPT_INLINE_TOKENS $MJ_PROMPT_BLOCK_TOKENS "
  for tok in $(mj_record_body "$f" | grep -oE '\{\{[A-Z_]+\}\}' | sed -e 's/^{{//' -e 's/}}$//' | sort -u || true); do
    case "$known" in *" $tok "*) ;; *) printf 'unknown token {{%s}}\n' "$tok"; rc=1; continue ;; esac
    case " $MJ_PROMPT_BLOCK_TOKENS " in
      *" $tok "*)
        if mj_record_body "$f" | grep -qE "^.+\{\{$tok\}\}|\{\{$tok\}\}.+$"; then
          printf 'block token {{%s}} must be alone on its line\n' "$tok"; rc=1
        fi ;;
    esac
  done
  return $rc
}

mj_prompt_list() {
  [ $# = 0 ] || mj_die "$MJ_EX_USAGE" "prompt list: unknown option $1"
  mj_require_installed
  local dir f n=0 desc fm flat
  dir="$(mj_prompt_dir)"
  [ -d "$dir" ] || { printf 'no prompt assets (.majordomus/prompts/ does not exist; run: majordomus update)\n'; return 0; }
  for f in "$dir"/*.md; do
    [ -f "$f" ] || continue
    n=$((n + 1))
    fm="$(mktemp "${TMPDIR:-/tmp}/mj.pl.XXXXXX")"; flat="$(mktemp "${TMPDIR:-/tmp}/mj.pm.XXXXXX")"; desc="(unreadable)"
    if mj_record_front "$f" > "$fm" 2>/dev/null && mj_yaml_flatten "$fm" > "$flat" 2>/dev/null; then desc="$(mj_yget "$flat" description)"; fi
    rm -f "$fm" "$flat"
    printf '%-22s %s\n' "$(basename "$f" .md)" "$desc"
  done
  [ "$n" = 0 ] && printf 'no prompt assets in .majordomus/prompts/\n'
  return 0
}

mj_prompt_show() {
  mj_require_installed
  local f; f="$(mj_prompt_path "$1")"
  [ -f "$f" ] || mj_die "$MJ_EX_MISSING" "no prompt asset '$1' (.majordomus/prompts/$1.md); try: majordomus prompt list"
  cat "$f"
}

mj_prompt_render() {
  local name="$1" f reason
  f="$(mj_prompt_path "$name")"
  [ -f "$f" ] || mj_die "$MJ_EX_MISSING" "no prompt asset '$name' (.majordomus/prompts/$name.md); try: majordomus prompt list"
  reason="$(mj_prompt_validate "$f")" || mj_die "$MJ_EX_CONTRACT" "prompt $name: $(printf '%s' "$reason" | tr '\n' ';')"

  local have_task=0 id="(no active task)" task="(no active task)" profile="(none)" scope="(none)" owner="(none)"
  mj_load_current && have_task=1
  if [ "$have_task" = 1 ]; then
    id="$(mj_cur id)"; task="$(mj_cur task)"; profile="$(mj_cur profile)"; owner="$(mj_cur owner)"
    scope="$(mj_ylist "$MJ_CUR_FLAT" scope | paste -sd, - | sed 's/,/, /g')"
  fi

  local tmp; tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.pr.XXXXXX")"
  # block fragments, built only when the body asks for them
  local body; body="$(mktemp "$tmp/body.XXXXXX")"; mj_record_body "$f" > "$body"
  if grep -q '{{OPEN_QUESTIONS}}' "$body"; then
    if [ "$have_task" = 1 ] && [ -f "$MJ_DIR/state/open-questions.md" ]; then
      grep -E "^- \[unresolved\] $id " "$MJ_DIR/state/open-questions.md" | sed -e "s/^- \[unresolved\] $id — /- /" > "$tmp/OPEN_QUESTIONS" || true
    fi
    [ -s "$tmp/OPEN_QUESTIONS" ] || printf -- '- none\n' > "$tmp/OPEN_QUESTIONS"
  fi
  if grep -q '{{DECISIONS}}' "$body"; then
    local dmax; dmax="$(mj_pol_req context.recent_decisions)"
    if [ -f "$MJ_DIR/state/decisions.md" ]; then mj_decision_entries "$MJ_DIR/state/decisions.md" "$([ "$have_task" = 1 ] && printf '%s' "$id")" "$dmax" > "$tmp/DECISIONS"
    else printf '(none)\n' > "$tmp/DECISIONS"; fi
  fi
  if grep -q '{{CHECKPOINT}}' "$body"; then
    if [ "$have_task" = 1 ] && mj_resolve_latest "$MJ_DIR/state/checkpoints" "$id"; then mj_record_body "$MJ_RES_PATH" | sed '/^$/d' > "$tmp/CHECKPOINT"
    else printf '(none)\n' > "$tmp/CHECKPOINT"; fi
  fi
  if grep -q '{{HANDOVER}}' "$body"; then
    if mj_resolve_latest "$MJ_DIR/state/handovers" "$([ "$have_task" = 1 ] && printf '%s' "$id")" ||
       mj_resolve_latest "$MJ_DIR/state/handovers" ""; then mj_record_body "$MJ_RES_PATH" | sed '/^$/d' > "$tmp/HANDOVER"
    else printf '(none)\n' > "$tmp/HANDOVER"; fi
  fi
  if grep -q '{{CONTEXT}}' "$body"; then
    # a subshell so the exit in mj_cmd_context ends only the nested render, and so the
    # two settings that make it a plain nested body do not leak back to the caller
    # a subshell so the exit in mj_cmd_context ends only the nested render, and so the
    # two settings that make it a plain nested body do not leak back to the caller
    ( export MJ_NO_PROMPT=1 MJ_JSON=0; mj_cmd_context ) > "$tmp/CONTEXT" 2>/dev/null || true
    [ -s "$tmp/CONTEXT" ] || printf '(context unavailable)\n' > "$tmp/CONTEXT"
  fi

  awk -v d="$tmp" \
      -v v_TASK="$task" -v v_TASK_ID="$id" -v v_PROFILE="$profile" -v v_SCOPE="$scope" -v v_OWNER="$owner" \
      -v v_BRANCH="$(mj_git_branch)" -v v_HEAD="$(mj_git_head)" -v v_WORKING_TREE="$(mj_git_dirty)" \
      -v v_REPOSITORY="$MJ_ROOT" -v v_NOW="$(mj_now)" \
      -v inline="$MJ_PROMPT_INLINE_TOKENS" -v blocks="$MJ_PROMPT_BLOCK_TOKENS" '
    BEGIN{
      n = split(inline, ik, " ")
      val["TASK"]=v_TASK; val["TASK_ID"]=v_TASK_ID; val["PROFILE"]=v_PROFILE; val["SCOPE"]=v_SCOPE
      val["OWNER"]=v_OWNER; val["BRANCH"]=v_BRANCH; val["HEAD"]=v_HEAD; val["WORKING_TREE"]=v_WORKING_TREE
      val["REPOSITORY"]=v_REPOSITORY; val["NOW"]=v_NOW
      split(blocks, bk, " ")
    }
    {
      line = $0
      for (i in bk) {
        tok = "{{" bk[i] "}}"
        if (line == tok) {
          f = d "/" bk[i]
          while ((getline l < f) > 0) print l
          close(f); next
        }
      }
      for (i = 1; i <= n; i++) {
        tok = "{{" ik[i] "}}"
        while ((p = index(line, tok)) > 0)
          line = substr(line, 1, p - 1) val[ik[i]] substr(line, p + length(tok))
      }
      print line
    }' "$body"
  rm -rf "$tmp"
}

#!/usr/bin/env bash
# init — create the repository's AI layer, .ai/, from the tool's skeleton.
#
# It seeds the tracked context (the protocol, the manifest, the policy, the profiles, the
# reusable prompts, the vendored Majordomus rule baseline, the knowledge declarations, the
# workflows) and the ignore boundary for the checkout-local half. It installs nothing: the
# tool stays wherever it was run from, no shell file is touched, and .majordomus/ is never
# created, because that path is only ever an optional installation of the tool itself.
# Everything it writes under .ai/repo/ belongs to the repository from that moment; a newer
# tool does not rewrite it.
# shellcheck source=rules.sh
. "$MJ_LIB_DIR/rules.sh"

mj_cmd_init() {
  local extend=0 a
  for a in "$@"; do case "$a" in
    --extend) extend=1 ;;
    --help|-h) cat <<H
usage: majordomus init [--extend]
  creates .ai/ in this repository from the tool's skeleton: the protocol and manifest,
  the tracked context under .ai/repo/ (policy, profiles, prompts, the vendored rule
  baseline, knowledge declarations, workflows, skills, adrs) and the ignore boundary
  for .ai/local/, where this checkout's own state lives
  --extend   add what is missing to an existing .ai/ and overwrite nothing
  refuses (15) when .ai/ already exists without --extend, and when project data still
  lives under .majordomus/ (run: majordomus migrate)
  installs nothing: not the tool, not a hook, not a shell file; doctor names the hooks
H
      return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "init: unknown option $a" ;;
  esac; done
  mj_require_repo
  local skel="$MJ_SKELETON_DIR"
  [ -d "$skel/ai" ] || mj_die "$MJ_EX_INTERNAL" "skeleton missing at $skel/ai"
  [ -f "$MJ_STD_RULES_DIR/manifest.yaml" ] || mj_die "$MJ_EX_INTERNAL" "the distribution ships no standard rule package at $MJ_STD_RULES_DIR"
  case "$MJ_LAYOUT" in
    legacy) mj_die "$MJ_EX_REFUSED" "project data lives under .majordomus/ (the pre-.ai layout); run: majordomus migrate" ;;
    ai) [ "$extend" = 1 ] || mj_die "$MJ_EX_REFUSED" ".ai/ already exists in $MJ_ROOT (use --extend to add what is missing; nothing is overwritten)" ;;
    *) [ ! -e "$MJ_AI_DIR" ] || [ "$extend" = 1 ] || mj_die "$MJ_EX_REFUSED" "$(mj_rel "$MJ_AI_DIR")/ exists but carries no manifest; move it aside, or use --extend to seed the missing files" ;;
  esac

  MJ_INIT_CREATED=""
  # the protocol and the registry, then every tracked section
  mj_init_file "$skel/ai/README.md" "$MJ_AI_DIR/README.md"
  mj_init_file "$skel/ai/manifest.yaml" "$MJ_AI_MANIFEST"
  mj_init_file "$skel/ai/repo/README.md" "$MJ_AI_REPO_DIR/README.md"
  mj_init_file "$skel/policy.yaml" "$MJ_POLICY_FILE"
  # the scope: seeded beside the policy; an --extend on a layer whose manifest predates
  # the section seeds the file and says the manifest must name it
  if [ -n "$MJ_SCOPE_FILE" ]; then mj_init_file "$skel/ai/repo/scope.yaml" "$MJ_SCOPE_FILE"
  else
    mj_init_file "$skel/ai/repo/scope.yaml" "$MJ_AI_REPO_DIR/scope.yaml"
    printf 'note: %s names no scope section; add `scope: repo/scope.yaml` under sections: so that the file applies\n' "$(mj_rel "$MJ_AI_MANIFEST")" >&2
  fi
  mj_init_tree "$skel/profiles" "$MJ_PROFILES_DIR" '*.yaml'
  mj_init_tree "$skel/prompts" "$MJ_PROMPTS_DIR" '*.md'
  mj_init_file "$skel/ai/repo/rules/README.md" "$MJ_RULES_DIR/README.md"
  mkdir -p "$MJ_RULES_DIR/project"
  if [ ! -d "$MJ_RULES_DIR/vendor/$MJ_RULES_VENDOR_NS" ]; then
    mj_rules_vendor_install "$MJ_STD_RULES_DIR" "$MJ_RULES_DIR/vendor/$MJ_RULES_VENDOR_NS"
    MJ_INIT_CREATED="$MJ_INIT_CREATED $(mj_rel "$MJ_RULES_DIR")/vendor/$MJ_RULES_VENDOR_NS/"
  fi
  mj_init_tree "$skel/ai/repo/knowledge" "$MJ_KNOWLEDGE_DIR" '*'
  mkdir -p "$MJ_KNOWLEDGE_DIR/curated"
  mj_init_tree "$skel/ai/repo/workflows" "$MJ_WORKFLOWS_DIR" '*.md'
  mj_init_tree "$skel/ai/repo/skills" "$MJ_SKILLS_DIR" '*.md'
  mj_init_tree "$skel/ai/repo/adrs" "$MJ_ADRS_DIR" '*.md'
  mkdir -p "$MJ_PROJECT_DIR"
  # the checkout-local half: the state directories the durable commands write into, and
  # the two hand-editable stores, seeded from the tool's templates. Never tracked.
  mkdir -p "$MJ_STATE_DIR/handovers" "$MJ_STATE_DIR/checkpoints" "$MJ_AI_LOCAL_DIR/prompts" \
           "$MJ_AI_LOCAL_DIR/cache" "$MJ_AI_LOCAL_DIR/session-contexts"
  [ -f "$MJ_STATE_DIR/decisions.md" ]      || cp "$skel/templates/decisions.md" "$MJ_STATE_DIR/decisions.md"
  [ -f "$MJ_STATE_DIR/open-questions.md" ] || cp "$skel/templates/open-questions.md" "$MJ_STATE_DIR/open-questions.md"
  mj_init_gitignore

  local rel; rel="$(cd "$MJ_BIN_DIR" && pwd)"
  if [ -n "$MJ_INIT_CREATED" ]; then
    printf 'created%s\n' "$MJ_INIT_CREATED" | tr ' ' '\n' | sed '1!s/^/  /'
  else printf 'nothing to add; .ai/ already carries every file the skeleton seeds\n'; fi
  cat <<OUT
local state: $(mj_rel "$MJ_STATE_DIR")/ (ignored by git; this checkout's own)
next: majordomus update      # generate the provider instruction files named in the policy
next: majordomus doctor      # verify nothing is declared that is not wired
hooks are not installed by init; add these two lines yourself, doctor verifies them:
  pre-commit: $rel/majordomus doctor || exit \$?
  pre-push:   $rel/majordomus finish --check || exit \$?
OUT
}

# copy one skeleton file into place unless it already exists
mj_init_file() {
  local src="$1" dst="$2"
  [ -f "$src" ] || mj_die "$MJ_EX_INTERNAL" "skeleton file missing: $src"
  [ -e "$dst" ] && return 0
  mkdir -p "$(dirname "$dst")"
  cp "$src" "$dst"
  MJ_INIT_CREATED="$MJ_INIT_CREATED $(mj_rel "$dst")"
}
# copy every matching file of a skeleton directory into place, one by one, never overwriting
mj_init_tree() {
  local src="$1" dst="$2" pat="$3" f n=0
  mkdir -p "$dst"
  [ -d "$src" ] || return 0
  for f in "$src"/$pat; do
    [ -f "$f" ] || continue
    [ -e "$dst/$(basename "$f")" ] && continue
    cp "$f" "$dst/$(basename "$f")"; n=$((n+1))
  done
  [ "$n" -gt 0 ] && MJ_INIT_CREATED="$MJ_INIT_CREATED $(mj_rel "$dst")/"
  return 0
}
# the ignore boundary: one line, added once, to a file whose other content is left alone
mj_init_gitignore() {
  local gi="$MJ_ROOT/.gitignore" line
  line="$(mj_rel "$MJ_AI_LOCAL_DIR")/"
  grep -qx "$line" "$gi" 2>/dev/null && return 0
  { [ -f "$gi" ] && [ -n "$(tail -c1 "$gi")" ] && printf '\n'; printf '%s\n' "$line"; } >> "$gi"
  MJ_INIT_CREATED="$MJ_INIT_CREATED .gitignore:$line"
}

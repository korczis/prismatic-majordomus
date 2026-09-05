#!/usr/bin/env bash
# migrate — move a repository's project data from the pre-.ai layout (.majordomus/) into
# the portable AI layer (.ai/), once, explicitly, and with a recovery copy.
#
# The legacy marker is .majordomus/policy.yaml; the new marker is .ai/manifest.yaml; a
# .majordomus/bin/majordomus is a tool installation and never project data. Nothing here
# runs from an ordinary command: check, doctor and start refuse a legacy layout and name
# this command, so no move happens implicitly.
#
# Order: inventory and classify every file under .majordomus/, refuse on ambiguity or a
# destination that already exists, print the plan; then write the manifest, move the
# canonical files (git mv where tracked), copy the state byte for byte to an ignored
# backup before moving it, take the state out of the index, drop what is derived, seed
# what the layer needs from the skeleton, extend .gitignore once, re-stamp the projections,
# and run doctor. Unknown files are never deleted; .majordomus/ goes only when empty.
# shellcheck source=init.sh
. "$MJ_LIB_DIR/init.sh"

MJ_MIG_PLAN=""        # one line per file: <action>\t<source>\t<destination>
MJ_MIG_CONFLICTS=""
MJ_MIG_UNKNOWN=""
MJ_MIG_SRC=""         # the legacy directory
MJ_MIG_MANIFEST=""    # flattened skeleton manifest: the destinations come from it

mj_cmd_migrate() {
  local dry=0 a
  for a in "$@"; do case "$a" in
    --dry-run) dry=1 ;;
    --help|-h) cat <<H
usage: majordomus migrate [--dry-run]
  moves this repository's project data from the pre-.ai layout under .majordomus/ into
  the portable AI layer under .ai/, and creates the rest of that layer from the skeleton
  legacy marker   .majordomus/policy.yaml          new marker   .ai/manifest.yaml
  tool install    .majordomus/bin/majordomus       never project data; left alone
  --dry-run   print the full plan, every file and where it goes, and write nothing
  exit 0 when migrated, or when nothing is left to migrate (already on the .ai layout)
  exit 12 when there is nothing to migrate and no .ai layer (run: majordomus init)
  exit 15 when .majordomus/ holds both project data and a tool installation, or when a
          destination under .ai/ already exists; the refusal names the safe manual step
  the local state is copied byte for byte to tmp/majordomus-migrate-backup/<utc>/ before
  it moves; the path is printed. Unknown files under .majordomus/ are never deleted.
H
      return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "migrate: unknown option $a" ;;
  esac; done
  mj_require_repo
  local legacy="$MJ_ROOT/.majordomus" skel="$MJ_SKELETON_DIR"
  local has_legacy=0 has_new=0 has_tool=0
  [ -f "$legacy/policy.yaml" ] && has_legacy=1
  [ -f "$MJ_ROOT/.ai/manifest.yaml" ] && has_new=1
  [ -f "$legacy/bin/majordomus" ] && has_tool=1

  if [ "$has_legacy" = 0 ]; then
    if [ "$has_new" = 1 ]; then
      printf 'already migrated: .ai/manifest.yaml is present and .majordomus/policy.yaml is not\n'
      [ "$has_tool" = 1 ] && printf '.majordomus/ holds a tool installation (bin/majordomus), which is not project data; left alone\n'
      [ -d "$legacy" ] && [ "$has_tool" = 0 ] && printf '.majordomus/ remains with %s file(s) this command does not know; nothing there is project data\n' "$(find "$legacy" -type f | wc -l | tr -d ' ')"
      return 0
    fi
    mj_die "$MJ_EX_MISSING" "nothing to migrate: no .majordomus/policy.yaml and no .ai/manifest.yaml in $MJ_ROOT (run: majordomus init)"
  fi
  if [ "$has_tool" = 1 ]; then
    mj_die "$MJ_EX_REFUSED" ".majordomus/ holds both project data (policy.yaml) and a tool installation (bin/majordomus); neither is overwritten. Safe path: mv .majordomus/bin <somewhere outside .majordomus>, run majordomus migrate, then put the installation back under .majordomus/bin"
  fi
  [ -d "$skel/ai" ] || mj_die "$MJ_EX_INTERNAL" "skeleton missing at $skel/ai"
  [ -f "$MJ_STD_RULES_DIR/manifest.yaml" ] || mj_die "$MJ_EX_INTERNAL" "the distribution ships no standard rule package at $MJ_STD_RULES_DIR"

  # the source formats: a policy this version reads, before anything moves
  local flat; flat="$(mktemp "${TMPDIR:-/tmp}/mj.mg.XXXXXX")"
  mj_yaml_flatten "$legacy/policy.yaml" > "$flat" 2>/dev/null || { rm -f "$flat"; mj_die "$MJ_EX_CONTRACT" ".majordomus/policy.yaml does not parse; fix it in place, then migrate"; }
  local pver; pver="$(mj_yget "$flat" version)"; rm -f "$flat"
  [ "$pver" = 1 ] || mj_die "$MJ_EX_CONTRACT" ".majordomus/policy.yaml is version '$pver' (want 1); fix it in place, then migrate"

  MJ_MIG_SRC="$legacy"
  MJ_MIG_MANIFEST="$(mktemp "${TMPDIR:-/tmp}/mj.mm.XXXXXX")"
  mj_yaml_flatten "$skel/ai/manifest.yaml" > "$MJ_MIG_MANIFEST" 2>/dev/null \
    || mj_die "$MJ_EX_INTERNAL" "the skeleton manifest does not parse: $skel/ai/manifest.yaml"
  mj_migrate_inventory
  mj_migrate_print_plan
  if [ -n "$MJ_MIG_CONFLICTS" ]; then
    printf '%s\n' "$MJ_MIG_CONFLICTS" | sed 's/^/REFUSE conflict   /'
    rm -f "$MJ_MIG_MANIFEST"
    mj_die "$MJ_EX_REFUSED" "a destination under .ai/ already exists; move it aside or remove it, then migrate again (nothing was written)"
  fi
  if [ "$dry" = 1 ]; then
    printf 'dry run: nothing written\n'; rm -f "$MJ_MIG_MANIFEST"; return 0
  fi
  mj_migrate_apply
  rm -f "$MJ_MIG_MANIFEST"
}

# the destination of a section, from the manifest that is about to be installed
mj_mig_dest() { printf '%s/.ai/%s' "$MJ_ROOT" "$(mj_yget "$MJ_MIG_MANIFEST" "$1")"; }

# ---------------------------------------------------------------- inventory
# Every file under .majordomus/, classified by its first path segment. The plan is a
# table; the same table drives the dry run and the move, so what is printed is what runs.
#   move     canonical, to a tracked section        drop      derived, or a byte-identical template
#   state    local state, backed up then moved       keep      unknown, preserved and reported
#   saved    the provider body and templates: copied to the backup, then removed. The body
#            no longer exists anywhere, update renders none, and an old template still asks
#            for it; carrying either into .ai would leave a literal token in a generated file
mj_migrate_inventory() {
  local f rel top rest dest action tmpl
  local repo; repo="$MJ_ROOT/.ai/$(mj_yget "$MJ_MIG_MANIFEST" repo.path)"
  local state; state="$MJ_ROOT/.ai/$(mj_yget "$MJ_MIG_MANIFEST" local.path)/state"
  MJ_MIG_PLAN=""; MJ_MIG_CONFLICTS=""; MJ_MIG_UNKNOWN=""
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    rel="${f#"$MJ_MIG_SRC/"}"; top="${rel%%/*}"; rest="${rel#*/}"
    case "$top" in
      policy.yaml) action=move; dest="$(mj_mig_dest sections.policy)" ;;
      profiles)    action=move; dest="$(mj_mig_dest sections.profiles)/$rest" ;;
      prompts)     action=move; dest="$(mj_mig_dest sections.prompts)/$rest" ;;
      project)     action=move; dest="$(mj_mig_dest sections.project)/$rest" ;;
      providers)   action=saved; dest="not carried into .ai; copied to the backup, then removed" ;;
      templates)
        tmpl="$MJ_SKELETON_DIR/templates/$rest"
        if [ -f "$tmpl" ] && cmp -s "$f" "$tmpl"; then action=drop; dest="identical to the tool's template"
        else action=move; dest="$repo/templates/$rest"; fi ;;
      state)       action=state; dest="$state/$rest" ;;
      generated)   action=drop; dest="derived; every projection now carries its own stamp" ;;
      *)           action=keep; dest="not project data this version knows; left where it is"; MJ_MIG_UNKNOWN="$MJ_MIG_UNKNOWN$rel"$'\n' ;;
    esac
    case "$action" in move|state) [ -e "$dest" ] && MJ_MIG_CONFLICTS="$MJ_MIG_CONFLICTS$(mj_rel "$dest") exists; $rel would overwrite it"$'\n' ;; esac
    MJ_MIG_PLAN="$MJ_MIG_PLAN$action"$'\t'"$rel"$'\t'"$dest"$'\n'
  done < <(find "$MJ_MIG_SRC" -type f | LC_ALL=C sort)
  MJ_MIG_CONFLICTS="${MJ_MIG_CONFLICTS%$'\n'}"; MJ_MIG_UNKNOWN="${MJ_MIG_UNKNOWN%$'\n'}"
}

mj_migrate_print_plan() {
  local action rel dest
  printf 'migrate: .majordomus/ (pre-.ai layout) -> .ai/ (%s)\n' "$(mj_yget "$MJ_MIG_MANIFEST" schema)"
  while IFS="$MJ_TAB" read -r action rel dest; do
    [ -n "$action" ] || continue
    case "$action" in
      move|state) printf '  %-5s .majordomus/%s -> %s\n' "$action" "$rel" "$(mj_rel "$dest")" ;;
      *)          printf '  %-5s .majordomus/%s    (%s)\n' "$action" "$rel" "$dest" ;;
    esac
  done <<<"$MJ_MIG_PLAN"
  printf '  seed  the rest of .ai/ from the skeleton: protocol, manifest, vendored rules, knowledge, workflows\n'
  printf '  state backup first: tmp/majordomus-migrate-backup/<utc>/state/, byte for byte; then moved, and taken out of the index\n'
  case "$MJ_MIG_PLAN" in *"saved"$'\t'*) printf '  saved backup first: tmp/majordomus-migrate-backup/<utc>/providers/, byte for byte; then removed, and the bootstraps become the tool'"'"'s thin adapters\n' ;; esac
  printf '  then  .ai/local/ appended to .gitignore once; majordomus update --force; majordomus doctor\n'
  [ -z "$MJ_MIG_UNKNOWN" ] || printf 'WARN  unknown     .majordomus/ — %s file(s) this version does not know are preserved, and .majordomus/ stays until they are moved by hand  [reproduce: find .majordomus -type f]\n' "$(printf '%s\n' "$MJ_MIG_UNKNOWN" | wc -l | tr -d ' ')"
}

# ---------------------------------------------------------------- apply
mj_migrate_apply() {
  local action rel dest src stamp backup="" n_moved=0 n_state=0 n_dropped=0 n_saved=0 d
  local state_src="$MJ_MIG_SRC/state"
  local state_dst; state_dst="$MJ_ROOT/.ai/$(mj_yget "$MJ_MIG_MANIFEST" local.path)/state"

  # 1. the recovery copy, before anything under state/ or providers/ is touched
  for d in state providers; do
    [ -d "$MJ_MIG_SRC/$d" ] || continue
    if [ -z "$backup" ]; then
      stamp="$(mj_now_compact)"; backup="$MJ_ROOT/tmp/majordomus-migrate-backup/$stamp"
      mkdir -p "$backup"
    fi
    cp -R "$MJ_MIG_SRC/$d" "$backup/$d"
    mj_migrate_verify_copy "$MJ_MIG_SRC/$d" "$backup/$d" || mj_die "$MJ_EX_INTERNAL" "the backup under $(mj_rel "$backup") does not match .majordomus/$d byte for byte; nothing was moved"
    printf 'backup .majordomus/%s/ -> %s/%s/ (byte for byte; verified)\n' "$d" "$(mj_rel "$backup")" "$d"
  done
  if [ -n "$backup" ]; then
    mj_git check-ignore -q "$(mj_rel "$backup")/state" 2>/dev/null \
      || printf 'WARN  backup      %s — is not ignored by git; do not commit it  [reproduce: git check-ignore -v %s]\n' "$(mj_rel "$backup")" "$(mj_rel "$backup")"
  fi

  # 2. the protocol and the manifest, so that the layout resolves as .ai from here on
  mkdir -p "$MJ_ROOT/.ai"
  cp "$MJ_SKELETON_DIR/ai/README.md" "$MJ_ROOT/.ai/README.md"
  cp "$MJ_SKELETON_DIR/ai/manifest.yaml" "$MJ_ROOT/.ai/manifest.yaml"
  # shellcheck disable=SC2034  # the manifest cache is read by mj_load_manifest; cleared so it reloads
  MJ_MAN_FLAT=""; mj_resolve_layout
  [ "$MJ_LAYOUT" = ai ] || mj_die "$MJ_EX_INTERNAL" "the layout did not resolve as .ai after the manifest was written"

  # 3. the canonical files, one by one: git mv keeps history for what is tracked
  while IFS="$MJ_TAB" read -r action rel dest; do
    [ -n "$action" ] || continue
    src="$MJ_MIG_SRC/$rel"
    case "$action" in
      move)
        mkdir -p "$(dirname "$dest")"
        if mj_git ls-files --error-unmatch -- ".majordomus/$rel" >/dev/null 2>&1; then mj_git mv -- ".majordomus/$rel" "$(mj_rel "$dest")"
        else mv "$src" "$dest"; fi
        n_moved=$((n_moved+1)) ;;
      drop|saved)
        if mj_git ls-files --error-unmatch -- ".majordomus/$rel" >/dev/null 2>&1; then mj_git rm -q -- ".majordomus/$rel"
        else rm -f "$src"; fi
        [ "$action" = saved ] && n_saved=$((n_saved+1)) || n_dropped=$((n_dropped+1)) ;;
    esac
  done <<<"$MJ_MIG_PLAN"
  [ "$n_saved" = 0 ] || printf 'INFO  providers   .majordomus/providers/ — %s file(s) copied to %s/providers/ and removed, not carried into .ai: the bootstraps are now the tool'"'"'s thin adapters, a repository override goes under %s/<provider>.tmpl in the new format, and the body'"'"'s rules belong under %s/project/ as rule objects (docs/DOCTRINE.md)  [reproduce: ls %s/providers]\n' \
    "$n_saved" "$(mj_rel "$backup")" "$(mj_rel "$MJ_PROVIDERS_DIR")" "$(mj_rel "$MJ_RULES_DIR")" "$(mj_rel "$backup")"

  # 4. the state: out of the index (it is local from now on), then one rename
  if [ -d "$state_src" ]; then
    [ -z "$(mj_git ls-files -- .majordomus/state | head -n 1)" ] || mj_git rm -r -q --cached -- .majordomus/state
    mkdir -p "$(dirname "$state_dst")"
    mv "$state_src" "$state_dst"
    n_state="$(find "$state_dst" -type f | wc -l | tr -d ' ')"
  fi

  # 5. what the layer needs and the legacy tree never had, never overwriting what moved
  mj_migrate_seed
  # 6. the ignore boundary, then the empty shells of the old tree
  mj_init_gitignore
  # one rmdir per directory, deepest first: a parent is empty only once its children went
  find "$MJ_MIG_SRC" -depth -type d -empty -exec rmdir {} \; 2>/dev/null
  printf 'moved %s canonical file(s) into %s/, %s state file(s) into %s/, dropped %s derived file(s), backed up and removed %s provider file(s)\n' \
    "$n_moved" "$(mj_rel "$MJ_AI_REPO_DIR")" "$n_state" "$(mj_rel "$MJ_STATE_DIR")" "$n_dropped" "$n_saved"
  if [ -d "$MJ_MIG_SRC" ]; then
    printf 'WARN  unknown     .majordomus/ — remains; these files are not project data this version knows and were not touched:  [reproduce: find .majordomus -type f]\n'
    printf '%s\n' "$MJ_MIG_UNKNOWN" | sed 's/^/  .majordomus\//'
  else printf 'removed .majordomus/ (empty)\n'; fi
  mj_ledger_append layout.migrated "\"from\":\".majordomus\",\"to\":\"$(mj_json_esc "$(mj_rel "$MJ_AI_DIR")")\",\"backup\":\"$(mj_json_esc "${backup:+$(mj_rel "$backup")}")\""

  # 7. projections carry the policy path in their stamp; re-stamp them, then judge the result
  local rc=0
  printf -- '--- majordomus update --force\n'
  "$MJ_BIN_DIR/majordomus" --repo "$MJ_ROOT" update --force || rc=$?
  printf 'update: exit %s\n' "$rc"
  rc=0
  printf -- '--- majordomus doctor\n'
  "$MJ_BIN_DIR/majordomus" --repo "$MJ_ROOT" doctor || rc=$?
  printf 'doctor: exit %s\n' "$rc"
  printf 'migrated: .ai/ is the layout; review with git status, then commit the tracked half\n'
  return 0
}

# the skeleton files the legacy tree never had: seeded through init's own never-overwrite
# helpers, so a file that just moved in is left exactly as it arrived
mj_migrate_seed() {
  MJ_INIT_CREATED=""
  mj_init_file "$MJ_SKELETON_DIR/ai/repo/README.md" "$MJ_AI_REPO_DIR/README.md"
  mj_init_file "$MJ_SKELETON_DIR/policy.yaml" "$MJ_POLICY_FILE"
  mj_init_file "$MJ_SKELETON_DIR/ai/repo/scope.yaml" "$MJ_AI_REPO_DIR/scope.yaml"
  mj_init_tree "$MJ_SKELETON_DIR/profiles" "$MJ_PROFILES_DIR" '*.yaml'
  mj_init_tree "$MJ_SKELETON_DIR/prompts" "$MJ_PROMPTS_DIR" '*.md'
  mj_init_file "$MJ_SKELETON_DIR/ai/repo/rules/README.md" "$MJ_RULES_DIR/README.md"
  mkdir -p "$MJ_RULES_DIR/project"
  if [ ! -d "$MJ_RULES_DIR/vendor/$MJ_RULES_VENDOR_NS" ]; then
    mj_rules_vendor_install "$MJ_STD_RULES_DIR" "$MJ_RULES_DIR/vendor/$MJ_RULES_VENDOR_NS"
    MJ_INIT_CREATED="$MJ_INIT_CREATED $(mj_rel "$MJ_RULES_DIR")/vendor/$MJ_RULES_VENDOR_NS/"
  fi
  mj_init_tree "$MJ_SKELETON_DIR/ai/repo/knowledge" "$MJ_KNOWLEDGE_DIR" '*'
  mkdir -p "$MJ_KNOWLEDGE_DIR/curated"
  mj_init_tree "$MJ_SKELETON_DIR/ai/repo/workflows" "$MJ_WORKFLOWS_DIR" '*.md'
  mj_init_tree "$MJ_SKELETON_DIR/ai/repo/skills" "$MJ_SKILLS_DIR" '*.md'
  mj_init_tree "$MJ_SKELETON_DIR/ai/repo/adrs" "$MJ_ADRS_DIR" '*.md'
  mkdir -p "$MJ_PROJECT_DIR"
  mkdir -p "$MJ_STATE_DIR/handovers" "$MJ_STATE_DIR/checkpoints" "$MJ_AI_LOCAL_DIR/prompts" \
           "$MJ_AI_LOCAL_DIR/cache" "$MJ_AI_LOCAL_DIR/session-contexts"
  [ -f "$MJ_STATE_DIR/decisions.md" ]      || cp "$MJ_SKELETON_DIR/templates/decisions.md" "$MJ_STATE_DIR/decisions.md"
  [ -f "$MJ_STATE_DIR/open-questions.md" ] || cp "$MJ_SKELETON_DIR/templates/open-questions.md" "$MJ_STATE_DIR/open-questions.md"
  [ -z "$MJ_INIT_CREATED" ] || printf 'seeded%s\n' "$MJ_INIT_CREATED" | tr ' ' '\n' | sed '1!s/^/  /'
}

# every file under SRC exists under DST with the same hash, and DST has no others
mj_migrate_verify_copy() {
  local src="$1" dst="$2" f
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    [ -f "$dst/$f" ] || return 1
    [ "$(mj_sha256 "$src/$f")" = "$(mj_sha256 "$dst/$f")" ] || return 1
  done < <(cd "$src" && find . -type f | sed 's|^\./||' | LC_ALL=C sort)
  [ "$(find "$src" -type f | wc -l)" = "$(find "$dst" -type f | wc -l)" ]
}

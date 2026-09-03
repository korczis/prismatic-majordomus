#!/usr/bin/env bash
# init — set up .majordomus/ in the repository. Refuses to overwrite. Never touches state/.
mj_cmd_init() {
  local force=0 gitignore=0 a
  for a in "$@"; do case "$a" in
    --force) force=1 ;; --gitignore) gitignore=1 ;;
    --help|-h) cat <<H
usage: majordomus init [--force] [--gitignore]
  --force      rewrite policy, profiles, templates, providers (state/ is never touched)
  --gitignore  add .majordomus/state/ to .gitignore (default: state is tracked)
H
      return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "init: unknown option $a" ;;
  esac; done
  mj_require_repo
  local skel="$MJ_BIN_DIR/../share/skeleton"
  [ -d "$skel" ] || mj_die "$MJ_EX_INTERNAL" "skeleton missing at $skel"
  if [ -e "$MJ_DIR" ] && [ "$force" != 1 ]; then
    mj_die "$MJ_EX_REFUSED" ".majordomus/ already exists in $MJ_ROOT (use --force to rewrite everything except state/)"
  fi
  mkdir -p "$MJ_DIR/profiles" "$MJ_DIR/templates" "$MJ_DIR/providers" "$MJ_DIR/state/handovers" "$MJ_DIR/generated"
  cp "$skel/policy.yaml" "$MJ_DIR/policy.yaml"
  cp "$skel"/profiles/*.yaml "$MJ_DIR/profiles/"
  cp "$skel"/templates/*.md "$MJ_DIR/templates/"
  cp "$skel"/providers/* "$MJ_DIR/providers/"
  [ -f "$MJ_DIR/state/decisions.md" ]      || cp "$skel/templates/decisions.md" "$MJ_DIR/state/decisions.md"
  [ -f "$MJ_DIR/state/open-questions.md" ] || cp "$skel/templates/open-questions.md" "$MJ_DIR/state/open-questions.md"
  if [ "$gitignore" = 1 ]; then
    grep -qx '.majordomus/state/' "$MJ_ROOT/.gitignore" 2>/dev/null || printf '.majordomus/state/\n' >> "$MJ_ROOT/.gitignore"
  fi
  local rel; rel="$(cd "$MJ_BIN_DIR" && pwd)"
  cat <<OUT
created .majordomus/policy.yaml
created .majordomus/profiles/ (routine, implementation, debugging, deep-work)
created .majordomus/templates/, .majordomus/providers/, .majordomus/state/, .majordomus/generated/
next: majordomus update      # generate the provider instruction files named in the policy
next: majordomus doctor      # verify nothing is declared that is not wired
hooks are not installed by init; add these two lines yourself, doctor verifies them:
  pre-commit: $rel/majordomus doctor || exit \$?
  pre-push:   $rel/majordomus finish --check || exit \$?
OUT
}

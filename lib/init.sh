#!/usr/bin/env bash
# init — create the repository's AI layer, .ai/, from the tool's skeleton.
#
# It seeds the tracked context (the protocol, the manifest, the policy, the profiles, the
# reusable prompts, the vendored Majordomus rule baseline, the knowledge declarations, the
# workflows) and the ignore boundary for the checkout-local half — or, with --personal, the
# same layer kept as one person's own: covered in git's own exclude file, with a check that
# nothing the repository shares was changed; --shared later hands such a layer to the team
# (ADR 0145). It installs nothing: the
# tool stays wherever it was run from, no shell file is touched, and .majordomus/ is never
# created, because that path is only ever an optional installation of the tool itself.
# Everything it writes under .ai/repo/ belongs to the repository from that moment; a newer
# tool does not rewrite it.
# shellcheck source=rules.sh
. "$MJ_LIB_DIR/rules.sh"

mj_cmd_init() {
  local extend=0 personal=0 shared=0 a
  for a in "$@"; do case "$a" in
    --extend) extend=1 ;;
    --personal) personal=1 ;;
    --shared) shared=1; extend=1 ;;
    --help|-h) cat <<H
usage: majordomus init [--extend] [--personal | --shared]
  creates .ai/ in this repository from the tool's skeleton: the protocol and manifest,
  the tracked context under .ai/repo/ (policy, profiles, prompts, the vendored rule
  baseline, knowledge declarations, workflows, skills, adrs) and the ignore boundary
  for .ai/local/, where this checkout's own state lives
  --extend   add what is missing to an existing .ai/ and overwrite nothing
  --personal a layer of your own in a repository other people share: nothing shared is
             changed. No line is added to .gitignore; .ai/ is covered in git's own
             exclude file (info/exclude of the git directory, which git never commits,
             pushes or clones), and init fails (10) unless 'git status' reads the same
             after it as before. Later writers (update, capture install) cover what they
             write the same way and leave a file the project tracks alone. An --extend of
             a personal layer stays personal without the flag
  --shared   hand a personal layer to the team: remove the cover, add the two .gitignore
             lines a plain init adds, and print what is now the repository's to commit.
             Nothing is committed for you
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
  [ "$personal" = 1 ] && [ "$shared" = 1 ] && mj_die "$MJ_EX_USAGE" "init: --personal and --shared are two answers to one question"
  if [ "$shared" = 1 ]; then
    # only a personal layer can be shared: a layer the repository already tracks is shared
    mj_personal_active || mj_die "$MJ_EX_REFUSED" "this layer is not personal, so there is nothing to share ($(mj_rel "$MJ_AI_DIR")/ is not covered in $(mj_personal_exclude_file))"
    MJ_INIT_UNCOVERED="$(mj_personal_uncover)" || mj_die "$MJ_EX_INTERNAL" "cannot rewrite $(mj_personal_exclude_file)"
  elif mj_personal_active; then
    # what this layer already is: an --extend of a personal layer must not start writing
    # .gitignore because the flag was not repeated
    personal=1
  fi
  local ai_rel before=''; ai_rel="$(mj_rel "$MJ_AI_DIR")"
  if [ "$personal" = 1 ]; then
    # a layer the repository tracks is already shared: its history holds it, and covering
    # it now would hide nothing and confuse everything
    [ -z "$(mj_git ls-files -- "$ai_rel" 2>/dev/null | head -n 1)" ] \
      || mj_die "$MJ_EX_REFUSED" "$ai_rel/ is tracked in $MJ_ROOT, so it is already shared; a personal layer is one git has never been given (git ls-files $ai_rel)"
    # what the repository shows before anything is written: the measure the result is held to
    before="$(mj_git status --porcelain --untracked-files=all 2>/dev/null)"
    mj_personal_cover "$ai_rel/" || mj_die "$MJ_EX_INTERNAL" "cannot write $(mj_personal_exclude_file)"
  fi

  MJ_INIT_CREATED="" MJ_INIT_SCOPE_NOTE=""
  # the protocol and the registry, then every tracked section
  mj_init_file "$skel/ai/README.md" "$MJ_AI_DIR/README.md"
  mj_init_file "$skel/ai/manifest.yaml" "$MJ_AI_MANIFEST"
  mj_init_file "$skel/ai/repo/README.md" "$MJ_AI_REPO_DIR/README.md"
  # a policy this run seeds, as opposed to one an --extend found: only the former is fitted
  local seeds_policy=0; [ -e "$MJ_POLICY_FILE" ] || seeds_policy=1
  mj_init_file "$skel/policy.yaml" "$MJ_POLICY_FILE"
  if [ "$personal" = 1 ] && [ "$seeds_policy" = 1 ]; then mj_init_personal_projections; fi
  # the scope: seeded beside the policy; an --extend on a layer whose manifest predates
  # the section seeds the file and says the manifest must name it
  if [ -n "$MJ_SCOPE_FILE" ]; then mj_init_scope "$skel/ai/repo/scope.yaml" "$MJ_SCOPE_FILE"
  else
    mj_init_scope "$skel/ai/repo/scope.yaml" "$MJ_AI_REPO_DIR/scope.yaml"
    printf 'note: %s names no scope section; add `scope: repo/scope.yaml` under sections: so that the file applies\n' "$(mj_rel "$MJ_AI_MANIFEST")" >&2
  fi
  mj_init_tree "$skel/profiles" "$MJ_PROFILES_DIR" '*.yaml'
  mj_init_tree "$skel/prompts" "$MJ_PROMPTS_DIR" '*.md'
  # every directory the layer seeds carries its own contract: coverage is a validation
  # class, and a directory this command creates without one would fail the first doctor
  mj_init_file "$skel/ai/repo/profiles/README.md" "$MJ_PROFILES_DIR/README.md"
  mj_init_file "$skel/ai/repo/prompts/README.md" "$MJ_PROMPTS_DIR/README.md"
  mj_init_file "$skel/ai/repo/rules/README.md" "$MJ_RULES_DIR/README.md"
  mkdir -p "$MJ_RULES_DIR/project"
  mj_init_file "$skel/ai/repo/rules/project/README.md" "$MJ_RULES_DIR/project/README.md"
  if [ ! -d "$MJ_RULES_DIR/vendor/$MJ_RULES_VENDOR_NS" ]; then
    mj_rules_vendor_install "$MJ_STD_RULES_DIR" "$MJ_RULES_DIR/vendor/$MJ_RULES_VENDOR_NS"
    MJ_INIT_CREATED="$MJ_INIT_CREATED $(mj_rel "$MJ_RULES_DIR")/vendor/$MJ_RULES_VENDOR_NS/"
  fi
  mj_init_tree "$skel/ai/repo/knowledge" "$MJ_KNOWLEDGE_DIR" '*'
  mkdir -p "$MJ_KNOWLEDGE_DIR/curated" "$MJ_KNOWLEDGE_DIR/candidates"
  mj_init_file "$skel/ai/repo/knowledge/curated/README.md" "$MJ_KNOWLEDGE_DIR/curated/README.md"
  # the review queue the deriver writes into (ADR 0091), with its contract from the start:
  # the first episode boundary adds a record, never a directory the layer has to explain
  mj_init_file "$skel/ai/repo/knowledge/candidates/README.md" "$MJ_KNOWLEDGE_DIR/candidates/README.md"
  mj_init_tree "$skel/ai/repo/workflows" "$MJ_WORKFLOWS_DIR" '*.md'
  mj_init_tree "$skel/ai/repo/skills" "$MJ_SKILLS_DIR" '*.md'
  mj_init_tree "$skel/ai/repo/adrs" "$MJ_ADRS_DIR" '*.md'
  # the use cases and applications: the sections exist from the start, with the taxonomy
  # and the context documents, so that the first use case is one file
  mj_init_tree "$skel/ai/repo/use-cases" "$MJ_AI_REPO_DIR/use-cases" '*'
  mj_init_tree "$skel/ai/repo/applications" "$MJ_AI_REPO_DIR/applications" '*'
  # the why catalogue: the section and its contract exist from the start, so that the first
  # moment is one file and nothing has to be registered for it
  mj_init_tree "$skel/ai/repo/why" "$MJ_AI_REPO_DIR/why" '*'
  # the product features: the section and its contract exist from the start, so that the
  # first feature is one file and every surface that presents it is derived from it
  mj_init_tree "$skel/ai/repo/features" "$MJ_AI_REPO_DIR/features" '*'
  mkdir -p "$MJ_PROJECT_DIR"
  # The sessions section, at the path the manifest names — or, on the first init, at the
  # path the skeleton's manifest is about to name. The resolved variable is empty on that
  # first run, because the manifest that names the section is being seeded in the same pass,
  # so reading it alone would seed nothing here and seed it on the next --extend: a
  # repository that reports work to do forever. The variable is still preferred when it does
  # resolve, so a layer whose manifest names another path is honoured rather than overruled.
  local sessions_dir="${MJ_SESSIONS_DIR:-$MJ_AI_REPO_DIR/sessions}"
  mkdir -p "$sessions_dir"
  mj_init_file "$MJ_SKELETON_DIR/ai/repo/sessions/README.md" "$sessions_dir/README.md"
  # The deployments section, the same way: the contract is there from the start, so a
  # repository that later deploys something adds one file rather than a directory, a
  # context document, a manifest entry and a source class.
  local deployments_dir="${MJ_DEPLOYMENTS_DIR:-$MJ_AI_REPO_DIR/deployments}"
  mkdir -p "$deployments_dir"
  mj_init_file "$MJ_SKELETON_DIR/ai/repo/deployments/README.md" "$deployments_dir/README.md"
  mj_init_file "$skel/ai/repo/project/README.md" "$MJ_PROJECT_DIR/README.md"
  # the checkout-local half: the state directories the durable commands write into, and
  # the two hand-editable stores, seeded from the tool's templates. Never tracked.
  mkdir -p "$MJ_STATE_DIR/handovers" "$MJ_STATE_DIR/checkpoints" "$MJ_AI_LOCAL_DIR/prompts" \
           "$MJ_AI_LOCAL_DIR/cache" "$MJ_AI_LOCAL_DIR/session-contexts"
  # The prompt archive is the one store here whose *names* are private: a record is called
  # after the opening of the prompt it holds, so a world-readable directory discloses what
  # was asked even while it is empty. It is created 0700 by the command that creates it,
  # rather than by the hook that writes the first record — the hook would be repairing a
  # mode that had already been wrong for however long the repository went without a prompt.
  chmod 700 "$MJ_AI_LOCAL_DIR/prompts" 2>/dev/null || true
  [ -f "$MJ_STATE_DIR/decisions.md" ]      || cp "$skel/templates/decisions.md" "$MJ_STATE_DIR/decisions.md"
  [ -f "$MJ_STATE_DIR/open-questions.md" ] || cp "$skel/templates/open-questions.md" "$MJ_STATE_DIR/open-questions.md"
  if [ "$personal" = 1 ]; then
    MJ_INIT_CREATED="$MJ_INIT_CREATED $(mj_personal_exclude_file):/$ai_rel/"
  else mj_init_gitignore; fi

  local rel; rel="${MJ_SELF:-$(cd "$MJ_BIN_DIR" && pwd)/majordomus}"
  if [ -n "$MJ_INIT_CREATED" ]; then
    printf 'created%s\n' "$MJ_INIT_CREATED" | tr ' ' '\n' | sed '1!s/^/  /'
  else printf 'nothing to add; .ai/ already carries every file the skeleton seeds\n'; fi
  [ -z "$MJ_INIT_SCOPE_NOTE" ] || printf '%s\n' "$MJ_INIT_SCOPE_NOTE"
  if [ "$personal" = 1 ]; then
    # The claim is checked where it is made: an init that says "personal" and changed what
    # the repository shows has not done what it said, and that is a failure, not a note.
    local after; after="$(mj_git status --porcelain --untracked-files=all 2>/dev/null)"
    if [ "$before" != "$after" ]; then
      mj_fail personal "$ai_rel/" "git status changed; a personal init leaves what the repository shares as it found it" "git status --porcelain --untracked-files=all"
      printf '%s\n' "$after" | grep -vxF -f <(printf '%s\n' "$before") | sed 's/^/  now shown: /' >&2
      exit "$MJ_EX_CONTRACT"
    fi
    mj_personal_verify plain || exit "$MJ_EX_CONTRACT"
    printf 'personal: this layer is yours; nothing the repository shares was changed. The cover is %s\n' "$(mj_personal_exclude_file)"
    printf 'personal: it stays on this machine. To propose it to the team: majordomus init --extend --shared\n'
  fi
  if [ "$shared" = 1 ]; then
    printf "shared: the cover is removed; these paths are now the repository's to commit, when you choose to:\n"
    printf '%s\n' "$MJ_INIT_UNCOVERED" | sed -e '/^kept /d' -e '/\.mj-tmp$/d' -e 's|^/||' -e 's/^/  git add /' -e '/^  git add $/d'
    printf '  git add .gitignore\n'
    # a file that is one person's by its own name stays covered, and the policy still
    # projects into it: the team's projection is theirs to name before the first commit
    printf '%s\n' "$MJ_INIT_UNCOVERED" | sed -n 's|^kept /\(.*\)$|shared: \1 stays yours and stays covered; point that projection at the file the team reads before you commit the policy|p'
    printf "shared: %s stays this checkout's own, as in every layer\n" "$(mj_rel "$MJ_AI_LOCAL_DIR")/"
  fi
  cat <<OUT
local state: $(mj_rel "$MJ_STATE_DIR")/ (ignored by git; this checkout's own)
next: majordomus update      # generate the provider instruction files named in the policy
next: majordomus doctor      # verify nothing is declared that is not wired
OUT
  mj_init_hook_advice "$personal" "$rel"
}

# Where the two hook lines go. Git says where its hooks are, and in a personal layer that
# answer decides whose they are: under the git directory they are this clone's and the lines
# go straight in; under a directory the project tracks (core.hooksPath pointing into the work
# tree) the hook git calls is the project's, and a line added to it would change what
# everyone shares. A dispatcher that runs <hook>.d/ leaves room for a file of one's own,
# which is covered here before it exists, so that creating it shows nothing in git status;
# a tracked hook with no such directory has no place of one's own, and that is said.
mj_init_hook_advice() {
  local personal="$1" rel="$2" hookdir hookrel h line dropin tracked=0
  hookdir="$(mj_git rev-parse --git-path hooks 2>/dev/null || true)"; [ -n "$hookdir" ] || hookdir=".git/hooks"
  case "$hookdir" in "$MJ_ROOT"/*) hookrel="${hookdir#"$MJ_ROOT"/}" ;; /*) hookrel="" ;; *) hookrel="$hookdir" ;; esac
  if [ "$personal" = 1 ] && [ -n "$hookrel" ] \
     && [ -n "$(mj_git ls-files -- "$hookrel" 2>/dev/null | head -n 1)" ]; then tracked=1; fi
  if [ "$tracked" = 0 ]; then
    cat <<OUT
hooks are not installed by init; add these two lines yourself, doctor verifies them:
  pre-commit: $rel doctor || exit \$?
  pre-push:   $rel finish --check || exit \$?
OUT
    return 0
  fi
  printf "hooks are not installed by init, and this repository's hooks are the project's (%s/ is tracked):\n" "$hookrel"
  for h in pre-commit pre-push; do
    case "$h" in pre-commit) line="$rel doctor || exit \$?" ;; *) line="$rel finish --check || exit \$?" ;; esac
    if [ -d "$MJ_ROOT/$hookrel/$h.d" ]; then
      dropin="$hookrel/$h.d/majordomus-personal"
      mj_personal_cover "$dropin" || mj_die "$MJ_EX_INTERNAL" "cannot write $(mj_personal_exclude_file)"
      printf '  %s: an executable file of your own, %s (covered already), holding: %s\n' "$h" "$dropin" "$line"
    else
      printf "  %s: %s/%s is tracked and runs no %s.d/, so there is no place of your own for: %s\n" "$h" "$hookrel" "$h" "$h" "$line"
    fi
  done
  printf '  doctor verifies whichever you add; an entry you cannot wire is yours to remove from %s\n' "$(mj_rel "$MJ_POLICY_FILE")"
}

# Fit the seeded projections to a repository other people share (ADR 0145). The skeleton
# names AGENTS.md, CLAUDE.md and GEMINI.md, which is right where none of them exists. Where
# the project already tracks one, `update` refuses it, whole, and the three commands the
# manual gives — init --personal, update, doctor — stopped at the second in exactly the
# repositories personal mode is for: the ones somebody already set up. So a tracked target
# is moved to the file its provider reads for one person (share/providers.yaml,
# personal_target), or left out when the provider has none, and one projection stays
# always-loaded, which the policy requires. Nothing is fitted in a repository that tracks
# none of them: there the personal layer is the plain one, as before.
mj_init_personal_projections() {
  local pol="$MJ_POLICY_FILE" flat prov_flat i=0 prov tgt own plan="" kept=0 loaded="" first="" changed=0 tmp
  flat="$(mj_yaml_flatten "$pol" 2>/dev/null)" || return 0
  prov_flat="$(mj_yaml_flatten "$MJ_SHARE_DIR/providers.yaml" 2>/dev/null || true)"
  while :; do
    tgt="$(printf '%s\n' "$flat" | sed -n "s/^projections\\.$i\\.target=//p" | head -n 1)"
    [ -n "$tgt" ] || break
    prov="$(printf '%s\n' "$flat" | sed -n "s/^projections\\.$i\\.provider=//p" | head -n 1)"
    if mj_git ls-files --error-unmatch -- "$tgt" >/dev/null 2>&1; then
      changed=1
      own="$(printf '%s\n' "$prov_flat" | sed -n "s/^providers\\.$prov\\.personal_target=//p" | head -n 1)"
      if [ -n "$own" ] && ! mj_git ls-files --error-unmatch -- "$own" >/dev/null 2>&1; then
        plan="$plan$i	$own	move	$tgt|"; kept=$((kept+1)); [ -n "$first" ] || first="$i"
      else
        plan="$plan$i	-	drop	$tgt|"
      fi
    else
      plan="$plan$i	-	keep	$tgt|"; kept=$((kept+1)); [ -n "$first" ] || first="$i"
      [ "$(printf '%s\n' "$flat" | sed -n "s/^projections\\.$i\\.always_loaded=//p" | head -n 1)" = true ] && loaded="$i"
    fi
    i=$((i+1))
  done
  # nothing the project tracks, or nothing left to keep: the seeded block stands, and in the
  # second case `update` says what it always said
  [ "$changed" = 1 ] && [ "$kept" -gt 0 ] || return 0
  [ -n "$loaded" ] || loaded="$first"
  tmp="$pol.mj-tmp.$$"
  awk -v plan="$plan" -v loaded="$loaded" '
    BEGIN {
      n = split(plan, rows, "|")
      for (r = 1; r <= n; r++) { if (rows[r] == "") continue; split(rows[r], f, "\t"); own[f[1]] = f[2]; act[f[1]] = f[3]; was[f[1]] = f[4] }
      idx = -1
    }
    function flush(   j) {
      if (idx < 0) return
      if (act[idx] == "drop") {
        printf "  # %s is the project%ss and its provider reads no file of one person%ss: left out\n", was[idx], q, q
      } else {
        if (act[idx] == "move") printf "  # %s is the project%ss; this is the file its provider reads beside it\n", was[idx], q
        for (j = 1; j <= m; j++) {
          if (item[j] ~ /^    always_loaded:/) continue
          if (act[idx] == "move" && item[j] ~ /^    target:/) { printf "    target: %s\n", own[idx]; continue }
          print item[j]
        }
        if (idx == loaded) print "    always_loaded: true"
      }
      m = 0
    }
    BEGIN { q = sprintf("%c", 39) }
    !inb && /^projections:[ \t]*$/ { inb = 1; print; next }
    inb && /^  - / { flush(); idx++; m = 0; item[++m] = $0; next }
    inb && /^[^ \t#]/ { flush(); idx = -1; inb = 0; print; next }
    inb && idx >= 0 { item[++m] = $0; next }
    { print }
    END { if (inb) flush() }
  ' "$pol" > "$tmp" || { rm -f "$tmp"; mj_die "$MJ_EX_INTERNAL" "cannot fit $(mj_rel "$pol") to a personal layer"; }
  mv "$tmp" "$pol" || mj_die "$MJ_EX_INTERNAL" "cannot write $(mj_rel "$pol")"
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
# Seed the scope from the skeleton, keeping a `# stack: <name> (<marker>)` ... `# end stack:
# <name>` block only when <marker> is at the repository's root: an adopter's scope names
# what its tree holds, not this repository's Elixir and Rust paths (I1989). What was found
# is said in MJ_INIT_SCOPE_NOTE, which the caller prints. Never overwrites.
mj_init_scope() {
  local src="$1" dst="$2" name marker keep=' ' found='' known=''
  [ -f "$src" ] || mj_die "$MJ_EX_INTERNAL" "skeleton file missing: $src"
  [ -e "$dst" ] && return 0
  while read -r name marker; do
    marker="${marker#(}"; marker="${marker%)}"
    known="$known${known:+, }$marker"
    if [ -e "$MJ_ROOT/$marker" ]; then keep="$keep$name "; found="$found${found:+, }$name ($marker)"; fi
  done < <(sed -n 's/^[[:space:]]*# stack: \([a-z0-9_-]*\) (\(.*\))[[:space:]]*$/\1 (\2)/p' "$src")
  mkdir -p "$(dirname "$dst")"
  awk -v keep="$keep" '
    /^[ \t]*# stack: /     { skip = (index(keep, " " $3 " ") == 0); if (!skip) print; next }
    /^[ \t]*# end stack: / { if (!skip) print; skip = 0; next }
    !skip' "$src" > "$dst"
  MJ_INIT_CREATED="$MJ_INIT_CREATED $(mj_rel "$dst")"
  if [ -n "$found" ]; then
    MJ_INIT_SCOPE_NOTE="scope: seeded for the stack found at the root: $found; edit $(mj_rel "$dst") for what else the tree holds"
  else
    MJ_INIT_SCOPE_NOTE="scope: no stack recognised at the root (looked for ${known:-nothing}); seeded only the entries every repository shares; add your own to $(mj_rel "$dst")"
  fi
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
# the ignore boundary: each line added once, to a file whose other content is left alone.
#
# Two lines, for two different reasons. The local section is this checkout's own state and is
# never shared. The staging file of mj_publish_record is the opposite — it lives inside a
# tracked section, because the hard link that publishes a record cannot cross a filesystem —
# and this line is what keeps a leftover from one from ever reaching a commit.
mj_init_gitignore() {
  local gi="$MJ_ROOT/.gitignore" line
  for line in "$(mj_rel "$MJ_AI_LOCAL_DIR")/" "$(mj_rel "$MJ_AI_REPO_DIR")/**/.tmp.*"; do
    grep -qxF "$line" "$gi" 2>/dev/null && continue
    { [ -f "$gi" ] && [ -n "$(tail -c1 "$gi")" ] && printf '\n'; printf '%s\n' "$line"; } >> "$gi"
    MJ_INIT_CREATED="$MJ_INIT_CREATED .gitignore:$line"
  done
}

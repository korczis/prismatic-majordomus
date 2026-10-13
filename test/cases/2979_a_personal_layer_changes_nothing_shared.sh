# majordomus-covers: init
# majordomus-negative: init
# claims: personal-layer-changes-nothing-shared
# A personal layer is one person's own in a repository other people share (ADR 0145, rule
# project.a-personal-layer-changes-nothing-shared): `init --personal` and the installer's
# --personal create the same layer a plain init does and leave what the repository shares
# — `git status`, the index, .gitignore — exactly as they found it.
#
# What is proved, each against a repository that already has uncommitted work in it, because
# "nothing changed" measured on a clean tree only proves that a clean tree stayed clean:
#   - init --personal changes nothing git reports, touches no tracked file, and says where
#     the cover is; the layer it made is the one a plain init makes;
#   - the later writers cover what they write before they write it (update's projection
#     targets, capture install's hooks), and refuse a target git already tracks, whole;
#   - an episode's end writes its session record and commits and pushes nothing;
#   - doctor holds a personal layer to its claim on every run, and names a file of it that
#     was tracked anyway; it does not ask the project's README to name the bootstrap;
#   - an --extend of a personal layer stays personal without the flag; a tracked layer is
#     refused; a linked worktree is covered through the common git directory;
#   - --shared hands the layer to the team: the cover goes, .gitignore gets a plain init's
#     two lines, the paths to commit are printed and nothing is committed; a layer that is
#     not personal is refused, and so are the two flags together;
#   - the executable reads an untracked layer without being told to (discovery is derived);
#   - the installer verifies it by itself, and a directory that is no repository is told so
#     after the tool is installed, not before.
. "$ROOT/test/lib.sh"

G() { git -c user.name=case -c user.email=case@example.invalid "$@"; }
status_of() { git -C "$1" status --porcelain --untracked-files=all; }
block_of() { sed -n '/^# >>> majordomus personal >>>$/,/^# <<< majordomus personal <<<$/p' "$1"; }
# a repository with history, a tracked ignore file, a modified file and an untracked one
dirty_repo() {
  mkdir -p "$1" && G -C "$1" init -q . \
    && printf 'a project\n' > "$1/README.md" && printf 'node_modules/\n' > "$1/.gitignore" \
    && G -C "$1" add -A && G -C "$1" commit -qm first \
    && printf 'in progress\n' >> "$1/README.md" && printf 'draft\n' > "$1/notes.txt"
}

# ------------------------------------------------------------------ init --personal
A="$T/a"; dirty_repo "$A"
before="$(status_of "$A")"; ignore_before="$(cksum < "$A/.gitignore")"
[ -n "$before" ] || { echo "    the fixture is clean; nothing would be measured"; exit 1; }
out="$(cd "$A" && "$MJ" init --personal 2>&1)" || { echo "    init --personal failed: $out"; exit 1; }
[ "$(status_of "$A")" = "$before" ] || { echo "    git status changed:"; status_of "$A" | sed 's/^/      /'; exit 1; }
[ "$(cksum < "$A/.gitignore")" = "$ignore_before" ] || { echo "    the tracked .gitignore was written"; exit 1; }
[ -z "$(git -C "$A" ls-files .ai)" ] || { echo "    something under .ai/ is tracked"; exit 1; }
git -C "$A" check-ignore -q .ai/manifest.yaml || { echo "    the layer is not ignored"; exit 1; }
git -C "$A" check-ignore -q .ai/repo/policy.yaml || { echo "    the shared half is not ignored"; exit 1; }
[ -z "$(git -C "$A" add -A --dry-run | grep -F '.ai/' || true)" ] || { echo "    git add -A would stage the layer"; exit 1; }
block_of "$A/.git/info/exclude" | grep -qxF '/.ai/' || { echo "    the exclude file does not cover /.ai/"; exit 1; }
printf '%s\n' "$out" | grep -q '^OK   personal' || { echo "    init did not report the verification: $out"; exit 1; }
printf '%s\n' "$out" | grep -qF "/.git/info/exclude" || { echo "    init did not say where the cover is"; exit 1; }
# the layer is the one a plain init makes: the same files, nothing held back
P="$T/plain"; mkdir -p "$P" && G -C "$P" init -q . && ( cd "$P" && "$MJ" init >/dev/null 2>&1 )
[ "$(cd "$A/.ai" && find . -type f | sort)" = "$(cd "$P/.ai" && find . -type f | sort)" ] \
  || { echo "    a personal layer holds other files than a plain one"; exit 1; }

# ------------------------------------------------------------------ the later writers
out="$(cd "$A" && "$MJ" update 2>&1)" || { echo "    update failed under personal: $out"; exit 1; }
[ -f "$A/AGENTS.md" ] || { echo "    update wrote no AGENTS.md"; exit 1; }
[ "$(status_of "$A")" = "$before" ] || { echo "    update changed what the repository shows:"; status_of "$A" | sed 's/^/      /'; exit 1; }
# covered because the policy names them, not because this case does
for tgt in $(sed -n 's/^ *target: *//p' "$A/.ai/repo/policy.yaml"); do
  block_of "$A/.git/info/exclude" | grep -qxF "/$tgt" || { echo "    projection target $tgt is not covered"; exit 1; }
done
out="$(cd "$A" && "$MJ" capture install 2>&1)" || { echo "    capture install failed under personal: $out"; exit 1; }
[ -f "$A/.claude/settings.json" ] || { echo "    capture install wrote no configuration"; exit 1; }
[ "$(status_of "$A")" = "$before" ] || { echo "    capture install changed what the repository shows:"; status_of "$A" | sed 's/^/      /'; exit 1; }

# ------------------------------------------------------------------ an episode's end commits nothing
# The seeded policy commits and pushes the session record when an episode closes. Under a
# personal layer that would put a file of the layer on a branch other people share.
head_a="$(git -C "$A" rev-parse HEAD)"
( cd "$A" && PATH="$(dirname "$MJ"):$PATH" && export PATH && unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID \
  && printf '{"session_id":"p-personal","source":"startup"}' | ./.claude/hooks/majordomus-session-start >/dev/null 2>&1 \
  && printf '{"session_id":"p-personal","reason":"clear"}' | ./.claude/hooks/majordomus-session-end >/dev/null 2>&1 ) || true
[ "$(git -C "$A" rev-parse HEAD)" = "$head_a" ] || { echo "    an episode's end committed onto the shared branch: $(git -C "$A" show --name-only --format=%s HEAD | tr '\n' ' ')"; exit 1; }
[ -n "$(find "$A/.ai/repo/sessions" -type f ! -name README.md 2>/dev/null | head -1)" ] || { echo "    the episode wrote no session record; nothing was measured"; exit 1; }
[ "$(status_of "$A")" = "$before" ] || { echo "    an episode changed what the repository shows:"; status_of "$A" | sed 's/^/      /'; exit 1; }

# ------------------------------------------------------------------ doctor holds it
out="$(cd "$A" && "$MJ" doctor 2>&1 || true)"
printf '%s\n' "$out" | grep -q '^OK   personal' || { echo "    doctor did not verify the personal layer"; exit 1; }
printf '%s\n' "$out" | grep -q '^FAIL bootstrap *README.md' && { echo "    doctor asks the project's README to name the bootstrap"; exit 1; }
G -C "$A" add -f .ai/README.md
out="$(cd "$A" && "$MJ" doctor 2>&1 || true)"
printf '%s\n' "$out" | grep -q '^FAIL personal.*tracked' || { echo "    doctor did not name the tracked file of a personal layer"; exit 1; }
G -C "$A" rm -q --cached .ai/README.md

# ------------------------------------------------------------------ --extend stays personal
rm "$A/.ai/repo/workflows/task-lifecycle.md"
( cd "$A" && "$MJ" init --extend >/dev/null 2>&1 ) || { echo "    init --extend failed on a personal layer"; exit 1; }
[ -f "$A/.ai/repo/workflows/task-lifecycle.md" ] || { echo "    --extend did not restore the file"; exit 1; }
[ "$(cksum < "$A/.gitignore")" = "$ignore_before" ] || { echo "    --extend wrote .gitignore on a personal layer"; exit 1; }
[ "$(status_of "$A")" = "$before" ] || { echo "    --extend changed what the repository shows"; exit 1; }

# ------------------------------------------------------------------ a tracked target refuses, whole
B="$T/b"; dirty_repo "$B"; printf 'ours\n' > "$B/CLAUDE.md"; G -C "$B" add CLAUDE.md; G -C "$B" commit -qm claude
( cd "$B" && "$MJ" init --personal >/dev/null 2>&1 ) || { echo "    init --personal failed beside a tracked CLAUDE.md"; exit 1; }
bbefore="$(status_of "$B")"
# without --force first: the refusal must be the personal one, and must not advise forcing
# a file that is the project's (the hand-edit protection used to answer here, and did)
rc=0; out="$(cd "$B" && "$MJ" update 2>&1)" || rc=$?
[ "$rc" = 15 ] || { echo "    update over a tracked target exited $rc, not 15: $out"; exit 1; }
printf '%s\n' "$out" | grep -q -- '--force' && { echo "    the refusal advises --force for the project's file: $out"; exit 1; }
printf '%s\n' "$out" | grep -q 'CLAUDE.md is tracked' || { echo "    the plain refusal does not name the file: $out"; exit 1; }
rc=0; out="$(cd "$B" && "$MJ" update --dry-run 2>&1)" || rc=$?
[ "$rc" = 15 ] || { echo "    a dry run over a tracked target exited $rc, not 15"; exit 1; }
out="$(cd "$B" && "$MJ" doctor 2>&1 || true)"
printf '%s\n' "$out" | grep -qE '^FAIL (bootstrap|projection) +CLAUDE.md' && { echo "    doctor holds the project's CLAUDE.md to this layer's rules: $(printf '%s\n' "$out" | grep 'CLAUDE.md')"; exit 1; }
printf '%s\n' "$out" | grep -q '^INFO projection  *CLAUDE.md.*the project' || { echo "    doctor does not say the tracked target is the project's"; exit 1; }
rc=0; out="$(cd "$B" && "$MJ" update --force 2>&1)" || rc=$?
[ "$rc" = 15 ] || { echo "    update --force over a tracked target exited $rc, not 15: $out"; exit 1; }
printf '%s\n' "$out" | grep -q 'CLAUDE.md is tracked' && printf '%s\n' "$out" | grep -q 'CLAUDE.local.md' || { echo "    the refusal does not name the file: $out"; exit 1; }
[ "$(cat "$B/CLAUDE.md")" = ours ] || { echo "    the tracked file was rewritten"; exit 1; }
[ ! -e "$B/AGENTS.md" ] || { echo "    a refused update wrote part of itself"; exit 1; }
[ "$(status_of "$B")" = "$bbefore" ] || { echo "    a refused update changed what the repository shows"; exit 1; }

# ------------------------------------------------------------------ a tracked layer cannot become one
C="$T/c"; mkdir -p "$C" && G -C "$C" init -q . && ( cd "$C" && "$MJ" init >/dev/null 2>&1 )
G -C "$C" add -A && G -C "$C" commit -qm layer
rc=0; out="$(cd "$C" && "$MJ" init --extend --personal 2>&1)" || rc=$?
[ "$rc" = 15 ] || { echo "    a tracked layer was accepted as personal (exit $rc): $out"; exit 1; }
[ ! -s "$C/.git/info/exclude" ] || ! grep -q 'majordomus personal' "$C/.git/info/exclude" \
  || { echo "    a refused init wrote the cover anyway"; exit 1; }

# ------------------------------------------------------------------ a linked worktree
G -C "$A" worktree add -q "$T/a-wt" -b side >/dev/null 2>&1
wbefore="$(status_of "$T/a-wt")"
( cd "$T/a-wt" && "$MJ" init --personal >/dev/null 2>&1 ) || { echo "    init --personal failed in a linked worktree"; exit 1; }
[ -f "$T/a-wt/.ai/manifest.yaml" ] || { echo "    the worktree got no layer"; exit 1; }
[ "$(status_of "$T/a-wt")" = "$wbefore" ] || { echo "    the linked worktree shows the layer"; exit 1; }
[ ! -e "$T/a-wt/.git/info" ] || { echo "    a worktree-private exclude was written; git reads the common one"; exit 1; }

# ------------------------------------------------------------------ --shared hands it to the team
S="$T/s"; dirty_repo "$S"
( cd "$S" && "$MJ" init --personal >/dev/null 2>&1 && "$MJ" update >/dev/null 2>&1 ) || { echo "    the layer to share could not be made"; exit 1; }
head_before="$(git -C "$S" rev-parse HEAD)"
rc=0; out="$(cd "$S" && "$MJ" init --personal --shared 2>&1)" || rc=$?
[ "$rc" = 2 ] || { echo "    --personal with --shared exited $rc, not 2"; exit 1; }
out="$(cd "$S" && "$MJ" init --extend --shared 2>&1)" || { echo "    init --shared failed: $out"; exit 1; }
grep -q 'majordomus personal' "$S/.git/info/exclude" && { echo "    --shared left the cover in place"; exit 1; }
grep -qx '.ai/local/' "$S/.gitignore" || { echo "    --shared did not write the local half's ignore line"; exit 1; }
printf '%s\n' "$out" | grep -q '^  git add \.ai/$' || { echo "    --shared did not say what to commit: $out"; exit 1; }
printf '%s\n' "$out" | grep -q '^  git add AGENTS\.md$' || { echo "    --shared did not name the projection targets: $out"; exit 1; }
printf '%s\n' "$out" | grep -q 'mj-tmp' && { echo "    --shared offers a staging name to commit"; exit 1; }
[ "$(git -C "$S" rev-parse HEAD)" = "$head_before" ] || { echo "    --shared committed"; exit 1; }
[ -z "$(git -C "$S" diff --cached --name-only)" ] || { echo "    --shared staged something"; exit 1; }
status_of "$S" | grep -q '^?? \.ai/' || { echo "    a shared layer is not offered to git"; exit 1; }
git -C "$S" check-ignore -q .ai/local/state/decisions.md || { echo "    the local half of a shared layer is not ignored"; exit 1; }
# a file that is one person's by its own name is not handed over with the layer
L="$T/l"; dirty_repo "$L"
( cd "$L" && "$MJ" init --personal >/dev/null 2>&1 ) || { echo "    the layer with a local projection could not be made"; exit 1; }
sed -i.bak 's|^\( *target: *\)CLAUDE\.md$|\1CLAUDE.local.md|' "$L/.ai/repo/policy.yaml" && rm -f "$L/.ai/repo/policy.yaml.bak"
grep -q 'target: CLAUDE.local.md' "$L/.ai/repo/policy.yaml" || { echo "    the fixture policy names no CLAUDE.md projection to retarget"; exit 1; }
( cd "$L" && "$MJ" update >/dev/null 2>&1 ) || { echo "    update into CLAUDE.local.md failed"; exit 1; }
out="$(cd "$L" && "$MJ" init --extend --shared 2>&1)" || { echo "    sharing a layer with a local projection failed: $out"; exit 1; }
printf '%s\n' "$out" | grep -q 'git add CLAUDE.local.md' && { echo "    --shared offers a personal file to the team: $out"; exit 1; }
printf '%s\n' "$out" | grep -q 'CLAUDE.local.md stays yours' || { echo "    --shared does not say the local file stays: $out"; exit 1; }
git -C "$L" check-ignore -q CLAUDE.local.md || { echo "    the local file lost its cover when the layer was shared"; exit 1; }
status_of "$L" | grep -q 'CLAUDE.local.md' && { echo "    a shared layer shows the personal file to git"; exit 1; }
# and the executable goes back to the index the moment the layer is committed: nothing to set
rc=0; out="$(cd "$S" && "$MJ" init --extend --shared 2>&1)" || rc=$?
[ "$rc" = 15 ] || { echo "    sharing a layer that is not personal exited $rc, not 15: $out"; exit 1; }

# ------------------------------------------------------------------ outside a repository
# the fixture may itself be a repository: git is told not to look above this directory
mkdir -p "$T/nowhere"; TP="$(cd "$T" && pwd -P)"
rc=0; ( cd "$T/nowhere" && GIT_CEILING_DIRECTORIES="$TP" "$MJ" init --personal >/dev/null 2>&1 ) || rc=$?
[ "$rc" = 2 ] || { echo "    init --personal outside a repository exited $rc, not 2"; exit 1; }

# ------------------------------------------------------------------ the executable reads it
# Discovery is derived: nobody passes --discovery filesystem for a layer git was never given.
if MJB="$(rust_bin)"; then
  n="$(MAJORDOMUS_SHARE="$ROOT/share" "$MJB" rules report --repo "$A" --format json 2>/dev/null | grep -c '"id"' || true)"
  [ "${n:-0}" -gt 0 ] || { echo "    the executable sees no rule in an untracked layer (discovery was not derived)"; exit 1; }
else
  echo "    note: no executable here; the derived discovery is proved by the crate's own tests"
fi

# ------------------------------------------------------------------ the installer, by itself
# The dry run needs no release; the install needs a fixture one, as case 85 builds it.
if MJB="$(rust_bin)" && command -v curl >/dev/null 2>&1 && mkdir -p "$T/fixture/releases" && start_http "$T/fixture"; then
  trap 'stop_http' EXIT INT TERM HUP
  MAJORDOMUS_DIST_BIN="$MJB" "$ROOT/scripts/release-fixture" --out "$T/fixture" --base-url "$HTTP_BASE" > "$T/fixture.log" 2>&1 \
    || { echo "    the fixture release could not be built"; tail -5 "$T/fixture.log" | sed 's/^/      /'; exit 1; }
  HOMEDIR="$T/home"; mkdir -p "$HOMEDIR"
  install_run() {
    env HOME="$HOMEDIR" MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE" MAJORDOMUS_INSECURE_BASE_URL=1 \
      sh "$ROOT/site/static/install.sh" "$@" 2>&1
  }
  out="$(install_run --dry-run --personal)" || { echo "    --dry-run --personal failed: $out"; exit 1; }
  printf '%s\n' "$out" | grep -q 'would run init *yes (personal)' || { echo "    the dry run does not say personal: $out"; exit 1; }
  D="$T/d"; dirty_repo "$D"; dbefore="$(status_of "$D")"
  out="$(cd "$D" && install_run --personal)" || { echo "    install --personal failed: $out"; exit 1; }
  printf '%s\n' "$out" | grep -q 'Verified (personal)' || { echo "    the installer did not verify the personal install: $out"; exit 1; }
  [ -f "$D/.ai/manifest.yaml" ] || { echo "    install --personal made no layer"; exit 1; }
  [ "$(status_of "$D")" = "$dbefore" ] || { echo "    install --personal changed what the repository shows"; exit 1; }
  # no repository: the tool stays installed, and the refusal says which half failed
  rc=0; out="$(cd "$T/nowhere" && GIT_CEILING_DIRECTORIES="$TP" install_run --personal)" || rc=$?
  [ "$rc" != 0 ] || { echo "    install --personal outside a repository reported success"; exit 1; }
  printf '%s\n' "$out" | grep -q 'needs a git repository' || { echo "    the refusal does not say why: $out"; exit 1; }
  [ -x "$HOMEDIR/.local/bin/majordomus" ] || { echo "    the refusal removed the installed tool"; exit 1; }
else
  echo "    note: no executable, curl or HTTP server here; the installer's half ran in no shell"
fi

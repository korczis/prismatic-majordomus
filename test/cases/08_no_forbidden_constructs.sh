# The tool's own source must contain none of the constructs SECURITY.md forbids.
# This is a source scan, not a behavioural test; it exists because there is no behavioural
# way to prove the absence of a network call.
. "$ROOT/test/lib.sh"
files="$ROOT/bin/majordomus $ROOT/lib/*.sh $ROOT/share/providers/*"
bad=0
chk() { # pattern description
  if grep -nE -- "$1" $files 2>/dev/null | grep -vE '^[^:]+:[0-9]+:\s*#'; then printf '    forbidden: %s\n' "$2"; bad=1; fi
}
chk '(^|[^a-zA-Z_])eval[[:space:]]'                       'eval'
chk '(^|[^a-zA-Z_./-])(wget|nc|ssh|scp)[[:space:]]'       'network client'
# `curl` is forbidden in the same breath, with one exception that is declared rather than
# tolerated: `majordomus context` reads the peer board of the shared MCP server this
# repository itself started, from the loopback URL in that server's own lease. SECURITY.md
# names it and so does the rule; here it is held to its shape, in both directions, so that
# it can neither spread nor quietly disappear while the promise still describes it.
net="$(grep -nE -- '(^|[^a-zA-Z_./-])curl[[:space:]]' $files 2>/dev/null | grep -vE '^[^:]+:[0-9]+:\s*#' || true)"
while IFS= read -r line; do
  [ -n "$line" ] || continue
  case "$line" in
    "$ROOT/lib/context.sh:"*"mj_has curl "*) ;;                              # a presence probe, not a call
    "$ROOT/lib/context.sh:"*'curl -fsS --max-time '*'/api/v1/peers"'*) ;;    # the one declared request
    *) printf '%s\n    forbidden: network client\n' "$line"; bad=1 ;;
  esac
done <<EOF
$net
EOF
# `return 0` or `return 1`: what is guarded is that the request is bounded and that its
# failure escapes as a status and never as output. `mj_peer_board` returns 1 so that a caller
# can tell "there is no board" from "the board is empty" — the distinction
# project.empty-is-not-failure exists for — and its callers turn that status into silence.
grep -qE 'curl -fsS --max-time [0-9]+ "[$]url/api/v1/peers".*\|\| return [01]' "$ROOT/lib/context.sh" \
  || { printf '    the declared peer-board exception is not one bounded request whose failure is silence\n'; bad=1; }
grep -qF 'http://127.0.0.1:*|http://localhost:*' "$ROOT/lib/context.sh" \
  || { printf '    the declared exception does not refuse a lease outside loopback\n'; bad=1; }
chk '/dev/(tcp|udp)/'                                       'bash network redirection'
chk 'rm[[:space:]]+-[a-zA-Z]*r[a-zA-Z]*f?[[:space:]]+"?\$MJ_ROOT'  'recursive delete of the repository'
chk 'rm[[:space:]]+-[a-zA-Z]*r[a-zA-Z]*f?[[:space:]]+"?\$MJ_(AI_DIR|AI_REPO_DIR|AI_LOCAL_DIR|STATE_DIR|HOME|SHARE_DIR)'   'recursive delete of the AI layer or the distribution'
chk 'rm[[:space:]]+-rf[[:space:]]+/[^t]'                    'recursive delete of an absolute path outside tmp'
[ "$bad" = 0 ]
# every rm -rf that does exist targets a mktemp path
grep -nE 'rm -rf' $files | grep -vE 'mktemp|\$tmp\b|\$TMP\b|"\$tmp"|"\$T"|\$MJ_CTX_TMP\b|\$MJ_PJ\b|\$MJ_ARCHIVE_TMPD\b' | grep -vE '^[^:]+:[0-9]+:\s*#' && exit 1
# ... and every variable the scan trusts by name is only ever assigned from mktemp
for v in MJ_CTX_TMP MJ_PJ MJ_BENCH_ARGV MJ_ARCHIVE_TMPD MJ_REC_TMP; do
  if grep -nE "(^|[;{][[:space:]]*)(local )?$v=" $files | grep -vE 'mktemp|'"$v"'=""'; then
    printf '    %s is assigned from something other than mktemp\n' "$v"; exit 1
  fi
done
# the tool writes only under the AI layer, to projection targets, or the one ignore line in
# .gitignore: every redirect into a path variable names a layout path (MJ_STATE_DIR and the
# other MJ_*_DIR/FILE variables), MJ_ROOT/<projection>, the ignore file, or a temp file
grep -nE '> *"?\$[A-Z_]+' $files | grep -vE 'MJ_STATE_DIR|MJ_POLICY_FILE|MJ_PROFILES_DIR|MJ_PROMPTS_DIR|MJ_PROJECT_DIR|MJ_RULES_DIR|MJ_KNOWLEDGE_DIR|MJ_AI_DIR|MJ_AI_REPO_DIR|MJ_AI_LOCAL_DIR|MJ_CUR|MJ_RULES_FLAT|MJ_KSRC_FLAT|MJ_DOC_FLAT|\$graph|\$fl\b|\$mf\b|\$gi\b|MJ_ROOT/\$tgt|MJ_ROOT/\$always|\$tmp|\$body|\$fm|\$flat|\$oflat|\$out|\$fp|\$fpflat|\$COPY|/dev/null|\$d/|\$MJ_POL_FLAT|\$MJ_PRO_FLAT|\$MJ_CUR_FLAT|\$final|\$MJ_CTX_TMP|\$MJ_CTXD_|\$MJ_TIMING_FILE|\$MJ_BENCH_ARGV|\$MJ_Q|\$rec|\$archive|\$led|\$tmpf|\$MJ_PJ/|\$MJ_ARCHIVE_TMPD|\$MJ_ARCHIVE_DROPPED|\$MJ_REC_TMP' | grep -vE '^[^:]+:[0-9]+:\s*#' && { echo "    write outside allowed paths"; exit 1; }

# ---------------------------------------------------------------- project.commands-run-non-interactively
# The mechanical half ADR 0039 put here: no automated run of this repository starts a command
# that can block for input. Scanned over everything that runs unattended — scripts, the
# shell library, the suite, the entry points and the CI definitions — for the constructs
# that wait for a person: a pager at the end of a pipe, an editor, a full-screen monitor, a
# prompt, an interactive git mode, npx without --yes (it asks before installing), an
# interactive login, sudo that may ask for a password, a container given a terminal.
#
# Only command position is matched, so the words in a message ("and more", "top of the
# list") are not findings. A scan that matches nothing proves nothing about itself, so each
# construct is first planted in a fixture and the scan must find every one of them.
interactive_scan() { # <files...>: prints "<construct>\t<file:line:text>" per finding
  local p
  while IFS='|' read -r name p; do
    [ -n "$name" ] || continue
    grep -nE -- "$p" "$@" 2>/dev/null | grep -vE '^([^:]+:)?[0-9]+:[[:space:]]*#' | sed "s|^|$name	|"
  done <<'PATTERNS'
pager|\|[[:space:]]*(less|more)([[:space:]]|$|\))
editor|(^|[;&(]|then|do|else)[[:space:]]*(vi|vim|nano|emacs|"?\$\{?(EDITOR|VISUAL)\}?"?)([[:space:];&|)]|$)
monitor|(^|[;&(]|then|do)[[:space:]]*(top|htop)([[:space:]]*(;|$)|[[:space:]]+-)
prompt|(^|[^a-zA-Z_])read[[:space:]]+-[a-zA-Z]*p
git-interactive|git[[:space:]]+(rebase|add)[[:space:]]+(-i|--interactive|-p|--patch)([[:space:]]|$)
npx-without-yes|(^|[;&|(]|then|do|else)[[:space:]]*npx[[:space:]]+([a-z@]|-[^-y]|--[^y])
interactive-login|(^|[;&|]|then|do|else)[[:space:]]*gh[[:space:]]+auth[[:space:]]+login
sudo-may-prompt|(^|[;&(]|then|do)[[:space:]]*sudo[[:space:]]+[^-]
container-tty|docker[[:space:]]+(run|exec)[[:space:]].*[[:space:]]-[a-z]*t[a-z]*i|docker[[:space:]]+(run|exec)[[:space:]].*[[:space:]]-[a-z]*i[a-z]*t
PATTERNS
}

planted="$PWD/interactive-fixture.sh"
cat > "$planted" <<'FIXTURE'
git log --oneline | less
  vim notes.txt
if true; then top; fi
read -rp "continue? " answer
git rebase -i HEAD~3
npx playwright install
gh auth login
sudo make install
docker run --rm -it alpine sh
FIXTURE
found="$(interactive_scan "$planted" | cut -f1 | LC_ALL=C sort -u | tr '\n' ' ')"
want="container-tty editor git-interactive interactive-login monitor npx-without-yes pager prompt sudo-may-prompt "
[ "$found" = "$want" ] || {
  printf '    the interactive scan does not find what it was planted with\n    want: %s\n    got:  %s\n' "$want" "$found"; exit 1; }
# ... and does not mistake prose, a comment or the non-interactive forms for the construct
cat > "$planted" <<'FIXTURE'
echo "and more of the same" # less is more
# vim is mentioned here only
printf 'top of the list\n'
while read -r line; do :; done < file
git rebase --onto main a b
npx --yes playwright install
sudo -n true
docker run --rm alpine sh
echo "skip: no browser (or: npx playwright install chromium)"
die "gh has no token; run: gh auth login"
echo "gh cannot see the repository (gh auth login)" >&2
    top && /^    - path:$/ { n = 0 }
FIXTURE
fp="$(interactive_scan "$planted")"
[ -z "$fp" ] || { printf '    the interactive scan reports non-interactive forms as findings:\n%s\n' "$fp"; exit 1; }

# The globs are walked rather than listed: a name is a path, not a line of `ls` output, and
# this case's own fixtures are the one thing the scan must not read — it plants the very
# constructs it refuses.
unattended=""
for f in "$ROOT"/scripts/* "$ROOT"/scripts/ci/* "$ROOT"/lib/*.sh "$ROOT"/test/*.sh "$ROOT"/test/cases/*.sh \
         "$ROOT"/bin/* "$ROOT"/.github/workflows/*.yml "$ROOT"/.github/actions/*/action.yml; do
  [ -f "$f" ] || continue
  case "$f" in */test/cases/08_no_forbidden_constructs.sh) continue ;; esac
  unattended="$unattended $f"
done
[ -n "$unattended" ] || { echo "    the interactive scan found no files to read; it would report clean over nothing"; exit 1; }
# shellcheck disable=SC2086  # the list is intentionally word-split into arguments
# A hosted CI runner's sudo has no password to ask for, so sudo in a workflow cannot block;
# on a person's machine it can, which is why every other file is held to `sudo -n`.
hits="$(interactive_scan $unattended | grep -vE '^sudo-may-prompt	[^:]*/\.github/workflows/[^/]+\.yml:' || true)"
[ -z "$hits" ] || { printf '    an unattended run starts a command that can block for input (project.commands-run-non-interactively):\n%s\n' "$hits"; exit 1; }
exit 0

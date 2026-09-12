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
grep -nE 'rm -rf' $files | grep -vE 'mktemp|\$tmp\b|\$TMP\b|"\$tmp"|"\$T"|\$MJ_CTX_TMP\b|\$MJ_PJ\b' | grep -vE '^[^:]+:[0-9]+:\s*#' && exit 1
# ... and every variable the scan trusts by name is only ever assigned from mktemp
for v in MJ_CTX_TMP MJ_PJ MJ_BENCH_ARGV; do
  if grep -nE "(^|[;{][[:space:]]*)(local )?$v=" $files | grep -vE 'mktemp|'"$v"'=""'; then
    printf '    %s is assigned from something other than mktemp\n' "$v"; exit 1
  fi
done
# the tool writes only under the AI layer, to projection targets, or the one ignore line in
# .gitignore: every redirect into a path variable names a layout path (MJ_STATE_DIR and the
# other MJ_*_DIR/FILE variables), MJ_ROOT/<projection>, the ignore file, or a temp file
grep -nE '> *"?\$[A-Z_]+' $files | grep -vE 'MJ_STATE_DIR|MJ_POLICY_FILE|MJ_PROFILES_DIR|MJ_PROMPTS_DIR|MJ_PROJECT_DIR|MJ_RULES_DIR|MJ_KNOWLEDGE_DIR|MJ_AI_DIR|MJ_AI_REPO_DIR|MJ_AI_LOCAL_DIR|MJ_CUR|MJ_RULES_FLAT|MJ_KSRC_FLAT|MJ_DOC_FLAT|\$graph|\$fl\b|\$mf\b|\$gi\b|MJ_ROOT/\$tgt|MJ_ROOT/\$always|\$tmp|\$body|\$fm|\$flat|\$oflat|\$out|\$fp|\$fpflat|\$COPY|/dev/null|\$d/|\$MJ_POL_FLAT|\$MJ_PRO_FLAT|\$MJ_CUR_FLAT|\$final|\$MJ_CTX_TMP|\$MJ_CTXD_|\$MJ_TIMING_FILE|\$MJ_BENCH_ARGV|\$MJ_Q|\$rec|\$archive|\$led|\$tmpf|\$MJ_PJ/' | grep -vE '^[^:]+:[0-9]+:\s*#' && { echo "    write outside allowed paths"; exit 1; }
exit 0

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
chk '(^|[^a-zA-Z_./-])(curl|wget|nc|ssh|scp)[[:space:]]'  'network client'
chk '/dev/(tcp|udp)/'                                       'bash network redirection'
chk 'rm[[:space:]]+-[a-zA-Z]*r[a-zA-Z]*f?[[:space:]]+"?\$MJ_ROOT'  'recursive delete of the repository'
chk 'rm[[:space:]]+-[a-zA-Z]*r[a-zA-Z]*f?[[:space:]]+"?\$MJ_(AI_DIR|AI_REPO_DIR|AI_LOCAL_DIR|STATE_DIR|HOME|SHARE_DIR)'   'recursive delete of the AI layer or the distribution'
chk 'rm[[:space:]]+-rf[[:space:]]+/[^t]'                    'recursive delete of an absolute path outside tmp'
[ "$bad" = 0 ]
# every rm -rf that does exist targets a mktemp path
grep -nE 'rm -rf' $files | grep -vE 'mktemp|\$tmp\b|\$TMP\b|"\$tmp"|"\$T"|\$MJ_CTX_TMP\b|\$MJ_PJ\b' | grep -vE '^[^:]+:[0-9]+:\s*#' && exit 1
# ... and every variable the scan trusts by name is only ever assigned from mktemp
for v in MJ_CTX_TMP MJ_PJ; do
  if grep -nE "(^|[;{][[:space:]]*)(local )?$v=" $files | grep -vE 'mktemp|'"$v"'=""'; then
    printf '    %s is assigned from something other than mktemp\n' "$v"; exit 1
  fi
done
# the tool writes only under the AI layer, to projection targets, or the one ignore line in
# .gitignore: every redirect into a path variable names a layout path (MJ_STATE_DIR and the
# other MJ_*_DIR/FILE variables), MJ_ROOT/<projection>, the ignore file, or a temp file
grep -nE '> *"?\$[A-Z_]+' $files | grep -vE 'MJ_STATE_DIR|MJ_POLICY_FILE|MJ_PROFILES_DIR|MJ_PROMPTS_DIR|MJ_PROJECT_DIR|MJ_RULES_DIR|MJ_KNOWLEDGE_DIR|MJ_AI_DIR|MJ_AI_REPO_DIR|MJ_AI_LOCAL_DIR|MJ_CUR|MJ_RULES_FLAT|MJ_KSRC_FLAT|MJ_DOC_FLAT|\$graph|\$fl\b|\$mf\b|\$gi\b|MJ_ROOT/\$tgt|MJ_ROOT/\$always|\$tmp|\$body|\$fm|\$flat|\$oflat|\$out|\$fp|\$fpflat|\$COPY|/dev/null|\$d/|\$MJ_POL_FLAT|\$MJ_PRO_FLAT|\$MJ_CUR_FLAT|\$final|\$MJ_CTX_TMP|\$MJ_CTXD_|\$MJ_Q|\$rec|\$archive|\$led|\$tmpf|\$MJ_PJ/' | grep -vE '^[^:]+:[0-9]+:\s*#' && { echo "    write outside allowed paths"; exit 1; }
exit 0

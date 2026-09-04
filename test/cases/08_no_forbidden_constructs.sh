# The tool's own source must contain none of the constructs SECURITY.md forbids.
# This is a source scan, not a behavioural test; it exists because there is no behavioural
# way to prove the absence of a network call.
. "$ROOT/test/lib.sh"
files="$ROOT/bin/majordomus $ROOT/lib/*.sh $ROOT/share/skeleton/providers/*"
bad=0
chk() { # pattern description
  if grep -nE -- "$1" $files 2>/dev/null | grep -vE '^[^:]+:[0-9]+:\s*#'; then printf '    forbidden: %s\n' "$2"; bad=1; fi
}
chk '(^|[^a-zA-Z_])eval[[:space:]]'                       'eval'
chk '(^|[^a-zA-Z_./-])(curl|wget|nc|ssh|scp)[[:space:]]'  'network client'
chk '/dev/(tcp|udp)/'                                       'bash network redirection'
chk 'rm[[:space:]]+-[a-zA-Z]*r[a-zA-Z]*f?[[:space:]]+"?\$MJ_ROOT'  'recursive delete of the repository'
chk 'rm[[:space:]]+-[a-zA-Z]*r[a-zA-Z]*f?[[:space:]]+"?\$MJ_DIR'   'recursive delete of .majordomus'
chk 'rm[[:space:]]+-rf[[:space:]]+/[^t]'                    'recursive delete of an absolute path outside tmp'
[ "$bad" = 0 ]
# every rm -rf that does exist targets a mktemp path
grep -nE 'rm -rf' $files | grep -vE 'mktemp|\$tmp\b|\$TMP\b|"\$tmp"|"\$T"|\$MJ_CTX_TMP\b' | grep -vE '^[^:]+:[0-9]+:\s*#' && exit 1
# ... and every variable the scan trusts by name is only ever assigned from mktemp
# shellcheck disable=SC2043  # one name today; the list is what makes adding another cheap
for v in MJ_CTX_TMP; do
  if grep -nE "^[[:space:]]*(local )?$v=" $files | grep -vE 'mktemp'; then
    printf '    %s is assigned from something other than mktemp\n' "$v"; exit 1
  fi
done
# the tool writes only under .majordomus/ or to projection targets: every redirect into a
# path variable names MJ_DIR, MJ_ROOT/<projection>, or a temp file
grep -nE '> *"?\$[A-Z_]+' $files | grep -vE 'MJ_DIR|MJ_CUR|MJ_ROOT/\$tgt|MJ_ROOT/\$always|\$tmp|\$body|\$fm|\$flat|\$oflat|\$out|\$fp|\$fpflat|\$COPY|/dev/null|\$d/|\$MJ_POL_FLAT|\$MJ_PRO_FLAT|\$MJ_CUR_FLAT|\$final|\$MJ_CTX_TMP|\$MJ_Q|\$rec|\$archive|\$led|\$tmpf' | grep -vE '^[^:]+:[0-9]+:\s*#' && { echo "    write outside allowed paths"; exit 1; }
exit 0

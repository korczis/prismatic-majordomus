# majordomus-covers: session capture
# majordomus-negative: session
# claims: session-records
# A session record is committed at episode end only when the policy says `true`.
#
# The lifecycle commits the record with plumbing — commit-tree and update-ref — because a
# provider's end hook has seconds and a repository's commit hooks can take minutes. So that
# commit runs none of the repository's hooks. Until 2026-10-09 it was made whenever the
# policy did not say `false`, and the skeleton said `true`: every adopter got commits that
# skipped its own commit-msg and pre-commit rules (prismatic-platform's issue references and
# doctrine checks among them) without anyone choosing that. Found by prismatic-platform-43
# (recovery finding F15).
#
# Three shapes, each driving the shim a provider runs:
#   1. the skeleton as `init` writes it: declares false — nothing committed, nothing pushed
#   2. the keys removed: absent is not consent — nothing committed
#   3. explicitly true: the record commit, as case 63 proves in full
. "$ROOT/test/lib.sh"
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T/pol" \
  && cp "$T/pol" .ai/repo/policy.yaml && rm -f "$T/pol"
mkdir -p lib && echo a > lib/a
git add -A >/dev/null; git commit -qm base >/dev/null
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH
git add -A >/dev/null; git commit -qm "hooks installed" >/dev/null 2>&1 || true

episode() {  # one provider episode, start to end; prints HEAD before and after
  local before after
  printf '{"session_id":"%s","source":"startup"}' "$1" | ./.claude/hooks/majordomus-session-start >/dev/null 2>&1
  before="$(git rev-parse HEAD)"
  printf '{"session_id":"%s","reason":"clear"}' "$1" | ./.claude/hooks/majordomus-session-end >/dev/null 2>&1
  after="$(git rev-parse HEAD)"
  printf '%s %s\n' "$before" "$after"
}
record_written() {
  local f
  for f in .ai/repo/sessions/*.md; do [ -f "$f" ] && [ "${f##*/}" != README.md ] && return 0; done
  return 1
}

# ---------------------------------------------------------------- 1. the skeleton: false
grep -q '^  commit_record_on_end: false' .ai/repo/policy.yaml \
  || { echo "    the skeleton does not declare commit_record_on_end: false"; exit 1; }
grep -q '^  push_record_on_end: false' .ai/repo/policy.yaml \
  || { echo "    the skeleton does not declare push_record_on_end: false"; exit 1; }
read -r before after <<EOF2
$(episode p-default)
EOF2
[ "$before" = "$after" ] || { echo "    a skeleton policy committed the record past the repository's hooks ($after)"; exit 1; }
record_written || { echo "    the record was not written at all; this case is about committing it"; exit 1; }

# ---------------------------------------------------------------- 2. absent: not consent
git add -A >/dev/null; git commit -qm "the first record, by the worker" >/dev/null
grep -v '^  commit_record_on_end:\|^  push_record_on_end:' .ai/repo/policy.yaml > "$T/pol" \
  && cp "$T/pol" .ai/repo/policy.yaml && rm -f "$T/pol"
if grep -q 'record_on_end' .ai/repo/policy.yaml; then
  echo "    the keys are still in the policy; the absent shape was not built"; exit 1
fi
git add -A >/dev/null; git commit -qm "keys removed" >/dev/null
read -r before after <<EOF2
$(episode p-absent)
EOF2
[ "$before" = "$after" ] || { echo "    an absent key committed the record ($after)"; exit 1; }

# ---------------------------------------------------------------- 3. explicitly true
git add -A >/dev/null; git commit -qm "the second record, by the worker" >/dev/null
awk '{print} /^session:/{print "  commit_record_on_end: true"; print "  push_record_on_end: true"}' \
  .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml && rm -f "$T/pol"
git add -A >/dev/null; git commit -qm "opted in" >/dev/null
read -r before after <<EOF2
$(episode p-opted-in)
EOF2
if [ "$before" = "$after" ]; then
  echo "    an explicit true did not commit the record"; exit 1
fi
files="$(git show --name-only --format= "$after" | sed '/^$/d')"
case "$files" in .ai/repo/sessions/*) ;; *) echo "    the record commit carried $files"; exit 1 ;; esac

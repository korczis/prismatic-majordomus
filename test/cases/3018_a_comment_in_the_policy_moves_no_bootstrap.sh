# majordomus-covers: none
# A comment-only edit of the policy regenerates no bootstrap (I1991). The stamp each
# generated bootstrap carries hashes the normal form of the policy and the profiles —
# comments, blank lines and trailing spaces dropped — not their bytes, so a whole-line
# comment, a changed trailing comment and blank lines added to .ai/repo/policy.yaml and to
# a profile leave `majordomus update --dry-run` reporting every target unchanged and
# `majordomus update` writing nothing, and the Rust executable's `generate providers
# --check`, which computes the same form (policy::normal_form), agrees with the shell's
# stamp. A changed value still moves the stamp in both.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
expect_exit 0 "$MJ" update
git add -A >/dev/null && git commit -qm install
stamp() { head -n 1 AGENTS.md; }
before="$(stamp)"
targets() { git ls-files -s -- AGENTS.md CLAUDE.md GEMINI.md; git status --porcelain -- AGENTS.md CLAUDE.md GEMINI.md; }
files_before="$(targets)"
[ -n "$files_before" ] || { echo "    update wrote no bootstrap to compare"; exit 1; }
# every plan line of a dry run that is not `unchanged`
moved() { "$MJ" update --dry-run 2>&1 | grep -v '^unchanged ' || true; }

# --- comments and blank lines only, in the policy and in a profile
profile="$(ls .ai/repo/profiles/*.yaml | head -n 1)"
{ printf '# a note on why the interval is what it is\n\n'; cat .ai/repo/policy.yaml; printf '\n# a trailing note\n'; } > "$T/policy.new"
sed 's/^\(  checkpoint_interval_default: [^ #]*\).*$/\1   # a changed comment/' "$T/policy.new" > .ai/repo/policy.yaml
grep -q '^  checkpoint_interval_default: [^ ]*   # a changed comment$' .ai/repo/policy.yaml \
  || { echo "    the fixture edit did not apply"; grep -n checkpoint_interval_default .ai/repo/policy.yaml; exit 1; }
printf '\n  # a comment in a profile\n' >> "$profile"
git diff --quiet -- .ai/repo/policy.yaml "$profile" && { echo "    the policy and the profile were not edited"; exit 1; }

expect_exit 0 "$MJ" update --dry-run
expect_grep '^unchanged AGENTS\.md$'
[ -z "$(moved)" ] || { echo "    a comment-only edit would rewrite a bootstrap:"; moved; exit 1; }
expect_exit 0 "$MJ" update
[ "$(targets)" = "$files_before" ] || { echo "    a comment-only edit rewrote a bootstrap:"; git status --porcelain; exit 1; }
[ "$(stamp)" = "$before" ] || { echo "    a comment moved the stamp: $before -> $(stamp)"; exit 1; }
doctor_out="$("$MJ" doctor 2>&1 || true)"
if grep -qE '^(FAIL|DRIFT) .*(AGENTS|CLAUDE|GEMINI)\.md' <<<"$doctor_out"; then
  echo "    doctor calls a bootstrap stale after a comment-only edit:"; grep -E '(AGENTS|CLAUDE|GEMINI)\.md' <<<"$doctor_out"; exit 1
fi
# the executable computes the same normal form, so the shell's stamp is current to it
expect_exit 0 "$RB" generate providers --check
expect_grep 'generate --check: in sync'

# --- a changed value still moves the stamp, in both tools
sed -i.bak 's/^\(  checkpoint_interval_default: \)[^ ]*/\145m/' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
[ -n "$(moved)" ] || { echo "    a changed value moves no bootstrap"; exit 1; }
expect_exit 10 "$RB" generate providers --check
expect_exit 0 "$MJ" update
[ "$(stamp)" != "$before" ] || { echo "    a changed value did not move the stamp"; exit 1; }
expect_exit 0 "$RB" generate providers --check
exit 0

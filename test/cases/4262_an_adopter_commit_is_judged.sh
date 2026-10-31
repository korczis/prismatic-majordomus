# majordomus-covers: init
# An adopter's commit message is judged by the policy it declares (I4262). A private
# repository whose plan bound every change to an issue, and which projected that plan onto a
# forge, wrote its own commit-msg hook: nothing shipped asked for the issue id, and nothing
# refused `Closes #12`, which closes the forge's issue the moment the commit lands.
#
#   init prints the commit-msg line beside the two doctor verifies, and the skeleton
#   policy declares both judgements off;
#   with reference_required: error a subject naming no issue of the layer is refused, and
#   one naming an issue it holds passes;
#   with closing_keyword: error a closing phrase is refused, naming the phrase, while a
#   reference and the keyword quoted in a code span pass.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init > "$T/init.out" 2>&1
grep -q 'commit-msg: .* commit validate "\$1" || exit \$?' "$T/init.out" \
  || { echo "    init printed no commit-msg line:"; cat "$T/init.out"; exit 1; }
grep -q '^  reference_required: off$' .ai/repo/policy.yaml \
  && grep -q '^  closing_keyword: off$' .ai/repo/policy.yaml \
  || { echo "    the skeleton policy does not declare both judgements off"; exit 1; }

pj_init
pj_milestone m0 0
pj_issue I0001 m0
git add . && git commit -qm plan

msg() { printf '%b' "$1" > "$T/msg"; }

# the skeleton's defaults judge neither
msg 'feat(plan): a thing\n\nCloses #12\n'
expect_exit 0 "$MJ" commit validate "$T/msg"

sed -i.bak -e 's/^  reference_required: off$/  reference_required: error/' \
  -e 's/^  closing_keyword: off$/  closing_keyword: error/' .ai/repo/policy.yaml
rm -f .ai/repo/policy.yaml.bak

msg 'feat(plan): a thing\n'
expect_exit 10 "$MJ" commit validate "$T/msg"
expect_grep 'commit.reference_missing'

msg 'feat(plan): a thing (I0001)\n'
expect_exit 0 "$MJ" commit validate "$T/msg"

msg 'feat(plan): a thing (I0001)\n\nCloses #12\n'
expect_exit 10 "$MJ" commit validate "$T/msg"
expect_grep 'commit.closing_keyword'
expect_grep "'Closes #12'"

msg 'feat(plan): a thing (I0001)\n\nRefs #12; the projection refuses `fixes #3`\n'
expect_exit 0 "$MJ" commit validate "$T/msg"
echo "ok: an adopter's policy asks for the issue and refuses a closing keyword"

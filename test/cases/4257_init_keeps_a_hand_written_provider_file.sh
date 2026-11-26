# majordomus-covers: init update
# init keeps a provider file the project already wrote (I4257). An adopter with a
# hand-written CLAUDE.md got a policy projecting CLAUDE.md whole, and had to know to switch it
# to mode: region before the first update.
#
#   a hand-written CLAUDE.md is projected as a region, and init says so;
#   update then appends the region and leaves every byte the project wrote;
#   targets the project does not have keep the whole-file mode;
#   a target Majordomus already wrote is not turned into a region.
. "$ROOT/test/lib.sh"
printf '# Our rules\n\nUse tabs. Deploy on Fridays never.\n' > CLAUDE.md
git add CLAUDE.md && git commit -qm "project's own CLAUDE.md"
expect_exit 0 "$MJ" init
expect_grep "kept the project's own CLAUDE.md"
awk '/target: CLAUDE.md/{getline; print; exit}' .ai/repo/policy.yaml | grep -q '^    mode: region$' \
  || { echo "    CLAUDE.md is not projected as a region:"; grep -n -A2 'target: CLAUDE.md' .ai/repo/policy.yaml; exit 1; }
if awk '/target: AGENTS.md/{getline; print; exit}' .ai/repo/policy.yaml | grep -q 'mode: region'; then
  echo "    AGENTS.md, which the project does not have, was turned into a region"; exit 1
fi
expect_exit 0 "$MJ" update
head -3 CLAUDE.md | cmp -s - <(printf '# Our rules\n\nUse tabs. Deploy on Fridays never.\n') \
  || { echo "    update changed the project's own text:"; head -5 CLAUDE.md; exit 1; }
expect_grep 'majordomus:begin' CLAUDE.md

# a second repository whose CLAUDE.md Majordomus wrote keeps the whole-file mode
S="$(mktemp -d "${TMPDIR:-/tmp}/mj4257.XXXXXX")"; trap 'rm -rf "$S"' EXIT
# its own identity, as the harness gives the case's repository: a runner has no global one
git -C "$S" init -q . && git -C "$S" config user.email t@example.com && git -C "$S" config user.name t
git -C "$S" commit -q --allow-empty -m base
( cd "$S" && "$MJ" init >/dev/null && "$MJ" update >/dev/null && rm -rf .ai && "$MJ" init > "$S/init2.out" 2>&1 ) \
  || { echo "    re-init over generated files failed"; cat "$S/init2.out"; exit 1; }
if grep -q 'kept the project' "$S/init2.out"; then echo "    a generated CLAUDE.md was taken for the project's own"; exit 1; fi
echo "ok: a hand-written provider file is kept as a region; generated and absent ones are not"

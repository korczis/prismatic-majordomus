# majordomus-covers: none
# The gate that refuses a tree naming the machine it was written on.
#
# `scripts/site-check` already refuses a built site carrying one, and that is the gate that
# caught this — at the moment of deployment, with the text already on master and the deploy
# already red. The reasoning of I0908, an issue whose stated purpose is that "no absolute
# developer path exists anywhere in the image", named a home directory; it reached plan.json,
# reached the site, and broke master's deploy. The cost of the late gate is that it bills the
# person publishing rather than the person writing.
#
# The same fault has a second half the site gate never sees: `share/allow/*.txt` was committed
# twice naming the absolute path of the worktree the generator last ran in. `derive-check` is
# structurally blind to that — it regenerates in the same checkout and gets the same answer —
# so the committed tree is read instead.
#
# What must hold is that the gate is wired into core-check, that this repository passes it,
# and — the part worth a test — that it fails on each half. A gate that cannot fail is
# decoration, so both halves are shown failing against a repository that carries the fault.
. "$ROOT/test/lib.sh"

GATE="$ROOT/scripts/ci/no-machine-paths"

# ---------------------------------------------------------------- wired, and passing here
expect_grep 'scripts/ci/no-machine-paths' "$ROOT/scripts/ci/core-check"
expect_exit 0 "$GATE" "$ROOT"

# ---------------------------------------------------------------- a fixture repository
R="$T/repo"; mkdir -p "$R/.ai/repo/project/issues" "$R/share/allow"
git -C "$R" init -q
git -C "$R" config user.email t@example.com
git -C "$R" config user.name Test
commit_fixture() {
  git -C "$R" add -A
  git -C "$R" -c core.hooksPath=/dev/null commit -qm "${1:-fixture}"
}

printf 'why: "A Machine has no developer'"'"'s checkout."\n' > "$R/.ai/repo/project/issues/I0001.yaml"
printf '# GENERATED FILE — DO NOT EDIT DIRECTLY\n# Source: the document schema `share/schemas/majordomus/rule/rule.v1.proto`\n' \
  > "$R/share/allow/rule.txt"
commit_fixture clean
expect_exit 0 "$GATE" "$R"

# ------------------------------------------------- it fails on an authored document
printf 'why: "A Machine has no %s/dev."\n' "/Users/somebody" > "$R/.ai/repo/project/issues/I0001.yaml"
commit_fixture "authored fault"
expect_exit 1 "$GATE" "$R"

printf 'why: "A Machine has no developer'"'"'s checkout."\n' > "$R/.ai/repo/project/issues/I0001.yaml"
commit_fixture "authored reworded"
expect_exit 0 "$GATE" "$R"

# ------------------------------------------------- and on a generated artifact
# The shape the leak actually had: a banner naming its source schema by the absolute path
# of the worktree the generator ran in, rather than by a repository-relative one.
printf '# GENERATED FILE — DO NOT EDIT DIRECTLY\n# Source: the document schema `%s/dev/repo/share/schemas/majordomus/rule/rule.v1.proto`\n' \
  "/Users/somebody" > "$R/share/allow/rule.txt"
commit_fixture "generated leak"
expect_exit 1 "$GATE" "$R"

printf '# GENERATED FILE — DO NOT EDIT DIRECTLY\n# Source: the document schema `share/schemas/majordomus/rule/rule.v1.proto`\n' \
  > "$R/share/allow/rule.txt"
commit_fixture "generated relative"
expect_exit 0 "$GATE" "$R"

# ------------------------------------------------- and where the two pathspecs used to miss
# site/content/ is derived and was covered by neither of the two steps this gate replaced:
# a leak there reached publication before anything noticed.
mkdir -p "$R/site/content/docs"
printf 'The checkout lives at %s/dev/repo.\n' "/Users/somebody" > "$R/site/content/docs/page.md"
commit_fixture "derived site content"
expect_exit 1 "$GATE" "$R"

# majordomus-covers: none
# Nothing merges a pull request, arms the forge to merge one, or brings the trunk into a
# branch, outside the integrator (ADR 0101; rule project.land-and-publish clause 2). The one
# guard is section 4b of scripts/ci/backlog-check, which reads scripts/, .just/, the justfile,
# .github/, lib/, bin/ and share/ — everything that runs here and is not the Rust integrator
# under apps/. This case plants each form it refuses, one at a time, in a copy of the tree
# the gate reads, and watches the gate fail naming the file; then proves the copy without a
# plant, a comment that only names a form, and this checkout itself all pass.
#
#   gh pr merge, with any flags, --auto among them   the forge merging on whatever master is then
#   REST pulls/<n>/merge                             the same, by hand
#   GraphQL mergePullRequest, enablePullRequestAutoMerge
#   an auto-merge action in a workflow               pascalgn/automerge-action,
#                                                    peter-evans/enable-pull-request-automerge
#   scripts/unblock, the file or a call of it        retired: majordomus prs repair is the gesture
#
# Case 131 holds the forms the gate had before (--admin, a force push to the trunk); the two
# cases are the one gate's two halves, not two greps.
. "$ROOT/test/lib.sh"

GATE="$ROOT/scripts/ci/backlog-check"
expect_file "$GATE"

# ---------------------------------------------------------------- the checkout passes
# The retired script is gone, and nothing in the tree the gate reads names a merge route.
[ ! -e "$ROOT/scripts/unblock" ] || { echo "    scripts/unblock is back; its gesture is majordomus prs repair"; exit 1; }
expect_exit 0 "$GATE"
expect_grep 'OK   backlog-check  nothing merges around the integrator'

# ---------------------------------------------------------------- a copy the gate can be pointed at
# The files every other section of the gate reads, as case 131 copies them, so a clean copy
# exits 0 and a plant is the only finding.
W="$(mktemp -d "${TMPDIR:-/tmp}/mj910.XXXXXX")"; trap 'rm -rf "$W"' EXIT
mkdir -p "$W/scripts/ci" "$W/test/cases" "$W/.ai/repo/rules/project" "$W/docs" "$W/.just" \
  "$W/lib" "$W/bin" "$W/share" "$W/.github/workflows"
cp "$ROOT/.gitattributes" "$W/.gitattributes"
cp "$ROOT/scripts/merge-derived" "$ROOT/scripts/reap-orphans" "$ROOT/scripts/derive" "$W/scripts/"
cp "$GATE" "$W/scripts/ci/backlog-check"
cp "$ROOT/.ai/repo/rules/project/accumulation-is-measured.v2.md" "$W/.ai/repo/rules/project/"
cp "$ROOT/test/cases/131_backlog_hygiene.sh" "$ROOT/test/cases/276_disk_is_bounded.sh" "$W/test/cases/"
cp "$ROOT/lib/rust_bin.sh" "$W/lib/rust_bin.sh"
cp "$ROOT/bin/majordomus-cli" "$W/bin/majordomus-cli"
cp "$ROOT/.just/build.just" "$ROOT/.just/site.just" "$W/.just/"
touch "$W/justfile"
chmod +x "$W/scripts/merge-derived" "$W/scripts/reap-orphans" "$W/scripts/ci/backlog-check"
gate() { MJ_ROOT="$W" "$W/scripts/ci/backlog-check"; }
expect_exit 0 gate
expect_grep 'nothing merges around the integrator'

# ---------------------------------------------------------------- each form, planted, is refused
refused() {   # <file> <content>: plant it, expect the gate to refuse it by name, remove it
  printf '%s\n' "$2" > "$W/$1"
  expect_exit 10 gate || { echo "    planted in $1: $2"; exit 1; }
  expect_grep 'merges around the integrator'
  expect_grep "^       $1:" || { echo "    the refusal does not name $1"; exit 1; }
  rm -f "$W/$1"
}
refused scripts/land 'gh pr merge --auto --merge "$n"'
refused scripts/land 'gh pr merge "$n" --squash --delete-branch'
refused .just/land.just 'land n: gh pr merge {{n}} --auto'
refused lib/land.sh 'gh api -X PUT "repos/$repo/pulls/$n/merge" -f merge_method=merge'
refused bin/land 'gh api graphql -f query="mutation { mergePullRequest(input: {pullRequestId: \"$id\"}) { clientMutationId } }"'
refused share/land.graphql 'mutation($id: ID!) { enablePullRequestAutoMerge(input: {pullRequestId: $id, mergeMethod: MERGE}) { clientMutationId } }'
refused .github/workflows/automerge.yml '      - uses: pascalgn/automerge-action@v0.16.4'
refused .github/workflows/automerge.yml '      - uses: peter-evans/enable-pull-request-automerge@v3'
refused justfile 'unblock *args: scripts/unblock {{args}}'

# the retired script itself, whatever it holds
printf '#!/bin/sh\nexit 0\n' > "$W/scripts/unblock"
expect_exit 10 gate
expect_grep 'scripts/unblock: the script itself is back'
rm "$W/scripts/unblock"

# ---------------------------------------------------------------- and what is not a route passes
# A comment may name every form: that is how a script says what it refuses to do.
printf '#!/bin/sh\n# never gh pr merge --auto, mergePullRequest, pascalgn/automerge-action or scripts/unblock:\n# majordomus prs repair brings master in, majordomus prs drain merges\n' > "$W/scripts/land"
expect_exit 0 gate
rm "$W/scripts/land"

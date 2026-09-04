# Impact of a change on scoped context documents, black-box against the context/v1
# contract: a root document reaches every descendant, a child never reaches its sibling,
# identity survives a directory move, a deleted document breaks whoever supersedes it, the
# manifest touches everything, a `tracks` match is a review item (WARN, never the exit code) and nothing more, and an
# unrelated change is silence.
#
# Every scenario is one change in the working tree against HEAD (the default change set),
# observed through `context affected`, then undone; the tree is committed clean between
# scenarios so that one scenario's mutation cannot leak into the next.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
"$MJ" update >/dev/null
A=.ai/repo/areas

# cdoc FILE ID SCOPE ORDER [FRONT-MATTER LINE ...]  — a minimal valid context document
cdoc() {
  local f="$1" id="$2" scope="$3" order="$4"; shift 4
  mkdir -p "$(dirname "$f")"
  { cat <<Y
---
schema: context/v1
id: $id
kind: context
title: Context $id
description: What $id describes, in one sentence.
status: active
scope: $scope
providers: ["*"]
audience: [human, agent]
composition: extend
order: $order
Y
    for line in "$@"; do printf '%s\n' "$line"; done
    printf -- '---\n\n# %s\n\nProse about %s.\n' "$id" "$id"
  } > "$f"
}
# clean — undo every working-tree mutation and prove the tree is back at HEAD
clean() {
  git checkout -q -- . && git clean -qfd
  [ -z "$(git status --porcelain)" ] || { echo "    the tree is not clean after $1"; git status --porcelain; exit 1; }
}

# --- a small tree: a parent area, two sibling children, one grandchild that replaces alpha
cdoc "$A/guide.md"            ai.repo.areas            subtree 100
cdoc "$A/alpha/guide.md"      ai.repo.areas.alpha      subtree 100
cdoc "$A/alpha/deep/guide.md" ai.repo.areas.alpha.deep subtree 100 'composition: replace' 'supersedes: [ai.repo.areas.alpha]'
cdoc "$A/beta/guide.md"       ai.repo.areas.beta       subtree 100 'tracks: [lib/thing.sh]'
sed -i.bak 's/^composition: extend$//' "$A/alpha/deep/guide.md"; rm -f "$A/alpha/deep/guide.md.bak"
mkdir -p lib src; printf 'thing() { :; }\n' > lib/thing.sh; printf 'unrelated\n' > src/other.txt
git add -A >/dev/null; git commit -qm "context fixtures"
expect_exit 0 "$MJ" context validate
expect_exit 0 "$MJ" context list
expect_grep 'ai\.repo\.areas\.alpha\.deep +.*areas/alpha/deep/guide\.md'

# --- a clean tree touches nothing
expect_exit 0 "$MJ" context affected
expect_no_grep '(ai\.repo\.areas|areas/)'

# --- the root document change reaches every descendant directory
printf '\nMore about the layer.\n' >> .ai/README.md
expect_exit 0 "$MJ" context affected
expect_grep '(ai\.repo\.areas\.alpha\b|areas/alpha)'
expect_grep '(ai\.repo\.areas\.beta\b|areas/beta)'
expect_grep '(ai\.repo\.areas\.alpha\.deep|areas/alpha/deep)'
clean "the root change"

# --- a child change never reaches its sibling
printf '\nMore about alpha.\n' >> "$A/alpha/guide.md"
expect_exit 0 "$MJ" context affected
expect_grep '(ai\.repo\.areas\.alpha\b|areas/alpha)'
expect_grep '(ai\.repo\.areas\.alpha\.deep|areas/alpha/deep)'
expect_no_grep '(ai\.repo\.areas\.beta|areas/beta)'
clean "the child change"

# --- a directory move changes ancestry and the identity survives it
git mv "$A/alpha" "$A/gamma"
expect_exit 0 "$MJ" context affected --staged
expect_grep 'ancestr'
expect_exit 0 "$MJ" context validate
expect_exit 0 "$MJ" context list
expect_grep 'ai\.repo\.areas\.alpha +.*areas/gamma/guide\.md'
expect_grep 'ai\.repo\.areas\.alpha\.deep +.*areas/gamma/deep/guide\.md'
expect_no_grep 'areas/alpha/'
git commit -qm "move alpha to gamma"
expect_exit 0 "$MJ" context validate

# --- a deleted document breaks the reference of the document that supersedes it
rm "$A/gamma/guide.md"
expect_exit 10 "$MJ" context validate
expect_grep 'broken-reference'
expect_grep 'ai\.repo\.areas\.alpha'
clean "the deletion"
expect_exit 0 "$MJ" context validate

# --- a manifest change marks every document affected
printf '\n# a comment is still a change\n' >> .ai/manifest.yaml
expect_exit 0 "$MJ" context affected
expect_grep '(ai\.layer\b|\.ai/README\.md)'
expect_grep '(ai\.repo\.areas\b|areas/guide\.md)'
expect_grep '(ai\.repo\.areas\.beta|areas/beta)'
expect_grep '(ai\.repo\.areas\.alpha\.deep|areas/gamma/deep)'
clean "the manifest change"

# --- a change to a tracked path is a review item, advisory, exit 0
printf 'thing() { echo changed; }\n' > lib/thing.sh
expect_exit 0 "$MJ" context affected
expect_grep 'WARN +context +.*ai\.repo\.areas\.beta.*tracks lib/thing\.sh \(M lib/thing\.sh\)'
expect_no_grep '^FAIL'
expect_no_grep '(ai\.repo\.areas\.alpha|areas/gamma)'
clean "the tracked change"

# --- an unrelated source change produces nothing
printf 'still unrelated\n' > src/other.txt
expect_exit 0 "$MJ" context affected
[ -z "$LAST_OUT" ] || { echo "    an unrelated change produced output:"; printf '%s\n' "$LAST_OUT"; exit 1; }
clean "the unrelated change"

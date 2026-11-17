# majordomus-covers: knowledge init
# Every Markdown file below docs/ is a document of a fresh layer (I4258). An adopter whose
# documents lived in docs/spec/, docs/wiki/ and docs/references/ found that the skeleton
# discovered one level of docs/ only, and declared three classes by hand before any of them
# was knowledge.
#
#   a document at any depth below docs/ is a node, and so is one directly in it;
#   a claim's detail page is discovered once, with no second class of the same kind over it;
#   a Markdown file outside docs/ below the root is still not a document.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
mkdir -p docs/spec docs/wiki/guides docs/claims notes
printf '# Top\n' > docs/top.md
printf '# Spec\n' > docs/spec/product.md
printf '# Guide\n' > docs/wiki/guides/start.md
printf '# A claim\n' > docs/claims/a-claim.md
printf '# Notes\n' > notes/scratch.md
git add . && git commit -qm docs

"$MJ" knowledge nodes --kind document > "$T/nodes.out" 2>&1 \
  || { echo "    knowledge nodes failed:"; cat "$T/nodes.out"; exit 1; }
for p in docs/top.md docs/spec/product.md docs/wiki/guides/start.md docs/claims/a-claim.md; do
  grep -q "document:$p" "$T/nodes.out" || { echo "    $p is not a document node:"; cat "$T/nodes.out"; exit 1; }
done
if grep -q 'notes/scratch.md' "$T/nodes.out"; then
  echo "    a file outside docs/ was discovered as a document"; exit 1
fi
expect_exit 0 "$MJ" knowledge validate
expect_no_grep 'claimed by'
echo "ok: every document below docs/ is discovered once"

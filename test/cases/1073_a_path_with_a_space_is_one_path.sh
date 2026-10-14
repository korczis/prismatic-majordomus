# majordomus-covers: check
# majordomus-negative: check
# A path that holds a space is one path. The scope doctrine read the touched files as
# words, and `git status --porcelain` wraps such a path in quotes, so on 2026-10-11 four
# concept images named like "Futuristic PR Drain Developer Dashboard.png", all inside the
# task's scope, were refused at pre-push as twenty paths outside it.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p "assets/concept" lib && echo x > lib/a.txt && git add . && git commit -qm base
expect_exit 0 "$MJ" start "concept images" --scope assets/concept --owner alice
# committed, staged, modified and untracked, each with spaces, all inside the scope
echo one > "assets/concept/Futuristic PR Drain Developer Dashboard.png"
git add . && git commit -qm "one committed image"
echo two > "assets/concept/Mesh UI interactive.png" && git add "assets/concept/Mesh UI interactive.png"
echo changed >> "assets/concept/Futuristic PR Drain Developer Dashboard.png"
echo three > "assets/concept/Train view draft.png"
expect_exit 0 "$MJ" check
expect_grep 'OK +scope .*3 touched file\(s\), all within scope'
# a rename keeps the new path and does not count the old one as a second change
git add -A && git commit -qm "the rest"
git mv "assets/concept/Train view draft.png" "assets/concept/Train view final.png"
expect_exit 0 "$MJ" check
expect_grep 'OK +scope .*all within scope'
git commit -qm rename
# outside the scope, the whole name is reported once, never in pieces
echo four > "lib/Owner notes.txt"
expect_exit 10 "$MJ" check
expect_grep 'FAIL scope +lib/Owner notes.txt — outside claimed scope'
expect_no_grep 'FAIL scope +notes\.txt'
expect_no_grep 'FAIL scope +lib/Owner +—'
expect_no_grep 'FAIL scope +"'
rm -f "lib/Owner notes.txt"
expect_exit 0 "$MJ" check

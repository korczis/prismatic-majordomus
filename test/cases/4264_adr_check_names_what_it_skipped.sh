# majordomus-covers: adr
# adr check names the decisions it did not examine (I4264). Discovery reads the index, so an
# adopter who proposed nine decisions and had not yet added them read "examined 0
# decision(s) ... every identity unique, every status known, every reference resolves": a
# pass over decisions nobody had looked at.
#
#   an untracked decision file is named, in text and in JSON, with the remedy;
#   once it is added, it is examined and no longer named;
#   a repository with no untracked decision says nothing about untracked files.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add . && git commit -qm base

expect_exit 0 "$MJ" adr check
expect_no_grep 'not tracked'

"$MJ" adr propose "Use one store" >/dev/null
f="$(ls .ai/repo/adrs/0001-*.md)"
expect_exit 0 "$MJ" adr check
expect_grep '1 decision file\(s\) are not tracked and were NOT examined'
expect_grep "$(basename "$f")"
expect_exit 0 "$MJ" adr check --json
expect_grep '"untracked":\[".*0001-'

git add "$f"
expect_exit 0 "$MJ" adr check
expect_grep 'examined 1 decision'
expect_no_grep 'not tracked'
echo "ok: an untracked decision is named until it is added, then examined"

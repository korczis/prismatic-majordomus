# A repository with Majordomus installed and projections generated, and one commit of work.
"$MJ" init >/dev/null
"$MJ" update >/dev/null
mkdir -p lib docs
echo a > lib/a
echo d > docs/d
git add . && git commit -qm base

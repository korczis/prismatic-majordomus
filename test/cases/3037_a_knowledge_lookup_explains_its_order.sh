# majordomus-covers: knowledge
# majordomus-negative: knowledge
# A knowledge lookup answers in an order it can explain, and asking writes nothing (I0811).
#
# `knowledge search` places a node on the first rung of a fixed ladder it reaches — id,
# title, title-prefix, path, body — and says which rung, so the order of an answer is read
# off the answer rather than trusted. `show` prints one node, `related` the edges into and
# out of it. None of the three writes: there is no stored index to go stale, each call
# compiles from the version-control index, and git status reads the same after all of them.
. "$ROOT/test/lib.sh"

"$MJ" init >/dev/null
mkdir -p docs
printf '# Widget\n\nThe exact title.\n' > docs/EXACT.md
printf '# Widget tooling and its limits\n\nA title that begins with the query.\n' > docs/PREFIX.md
printf '# Notes kept beside the code\n\nNothing here names it in the title.\n' > docs/WIDGET_NOTES.md
printf '# Unrelated heading\n\nOnly the body says WIDGET, and only once.\n' > docs/BODY.md
printf '# Nothing at all\n\nNo match anywhere.\n' > docs/NONE.md
printf '# Docs\n\nSee [the exact one](EXACT.md).\n' > docs/README.md
git add -A >/dev/null && git commit -q -m "install"

"$MJ" knowledge nodes --json > nodes.json
id_of() { jq -r --arg p "$1" '.nodes[] | select(.source == $p) | .id' nodes.json | head -n 1; }
exact="$(id_of docs/EXACT.md)"; prefix="$(id_of docs/PREFIX.md)"
path="$(id_of docs/WIDGET_NOTES.md)"; body="$(id_of docs/BODY.md)"; none="$(id_of docs/NONE.md)"
for v in exact prefix path body none; do
  [ -n "$(eval "printf %s \"\$$v\"")" ] || { echo "    the fixture produced no node for $v"; cat nodes.json; exit 1; }
done

before="$(git status --porcelain --untracked-files=all)"

# ---------------------------------------------------------------- the ladder, in order
"$MJ" knowledge search widget --json > s.json || { echo "    search exited non-zero"; cat s.json; exit 1; }
jq -e '.ladder == ["id","title","title-prefix","path","body"]' s.json >/dev/null \
  || { echo "    the answer does not state the ladder"; jq .ladder s.json; exit 1; }
got="$(jq -r '.results[] | "\(.matched) \(.id)"' s.json)"
want="title $exact
title-prefix $prefix
path $path
body $body"
# other nodes may match too (the fixture's own policy and skeleton); the four must be in this
# relative order, each on its own rung
order="$(printf '%s\n' "$got" | grep -F -x -e "title $exact" -e "title-prefix $prefix" -e "path $path" -e "body $body")"
[ "$order" = "$want" ] || { echo "    the rungs came back out of order:"; printf '%s\n' "$got"; exit 1; }
printf '%s\n' "$got" | grep -qF " $none" && { echo "    a node that matches nowhere was returned"; exit 1; }
jq -e 'all(.results[]; has("id") and has("kind") and has("title") and has("source") and has("matched") and (.rung | type == "number"))' s.json >/dev/null \
  || { echo "    a result is missing id, kind, title, source, matched or rung"; jq '.results[0]' s.json; exit 1; }

# a query that matches one node at two levels is placed on the higher rung: the exact id
# beats the body of every document that mentions it
"$MJ" knowledge search "$exact" --json > i.json
[ "$(jq -r '.results[0] | "\(.matched) \(.id)"' i.json)" = "id $exact" ] \
  || { echo "    the exact id was not the first result"; jq -c '.results[0:3]' i.json; exit 1; }
[ "$(jq --arg i "$exact" '[.results[] | select(.id == $i)] | length' i.json)" = 1 ] \
  || { echo "    one node was returned twice"; exit 1; }

# the order is a rule: the same question twice is the same answer
"$MJ" knowledge search widget --json > s2.json
cmp -s s.json s2.json || { echo "    two searches disagreed"; diff s.json s2.json | head; exit 1; }

# ---------------------------------------------------------------- show and related
"$MJ" knowledge show "$exact" --json > show.json
jq -e --arg i "$exact" '.node.id == $i and .node.source == "docs/EXACT.md" and (.node.hash | length) > 0' show.json >/dev/null \
  || { echo "    show did not print the node with its source and hash"; cat show.json; exit 1; }
expect_exit 12 "$MJ" knowledge show no-such-node
"$MJ" knowledge related "$exact" --json > rel.json
jq -e '.edges | type == "array"' rel.json >/dev/null || { echo "    related printed no edge list"; cat rel.json; exit 1; }
jq -e 'all(.edges[]; (.direction == "in" or .direction == "out") and .provenance != "")' rel.json >/dev/null \
  || { echo "    an edge carries no direction or no provenance"; cat rel.json; exit 1; }
expect_exit 12 "$MJ" knowledge related no-such-node
expect_exit 2 "$MJ" knowledge search

# ---------------------------------------------------------------- nothing was written
"$MJ" knowledge search widget > /dev/null
"$MJ" knowledge show "$exact" > /dev/null
"$MJ" knowledge related "$exact" > /dev/null
after="$(git status --porcelain --untracked-files=all)"
# the case's own answer files are the only difference
strip() { printf '%s\n' "$1" | grep -vE '^\?\? (nodes|s|s2|i|show|rel)\.json$' || true; }
[ "$(strip "$before")" = "$(strip "$after")" ] || {
  echo "    a read-only lookup changed the working tree:"; diff <(strip "$before") <(strip "$after"); exit 1; }

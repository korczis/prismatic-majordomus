# majordomus-covers: none
# A file the index refused is named, with what decided it, wherever the layer is said to be
# degraded.
#
# The index already refuses a document that breaks its kind's schema: no object is created,
# a `schema_violation` diagnostic carries the path and the failed constraint, and the layer's
# state becomes Degraded. What a person got was the number — "3 declared file(s) did not
# become objects" — and a remedy, `majordomus capabilities validate`, that answers a
# different question (capabilities against their interfaces) and reports `0 failure(s)` while
# three files sit refused. The list existed only as a log line on standard error, so the one
# reader that had already worked the answer out threw it away.
#
# The subject is knowledge because `class` is the case that found it: the schema makes it
# required and a curated note without one is still prose that reads perfectly well. It is not
# a knowledge check, though — the refusal and the naming hold for any kind whose schema a
# file breaks, which is why two shapes of violation are planted rather than one.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
# The executable resolves its distribution from beside itself; run straight out of the build
# directory it has none, and every command exits 12 before it reads anything.
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
command -v jq >/dev/null 2>&1 || { echo "    skip: no jq"; exit 0; }

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -q -m install

# note <file> <id> <class line, empty for none>
note() {
  {
    printf -- '---\nschema: knowledge/v1\nid: %s\nkind: knowledge\n' "$2"
    if [ -n "$3" ]; then printf '%s\n' "$3"; fi
    printf 'title: A note the fixture plants\ndescription: One line.\nstatus: verified\nepistemics: observed\ndate: 2026-09-18\n---\n\n# A note the fixture plants\n\nProse.\n'
  } > ".ai/repo/knowledge/curated/$1"
  git add -A >/dev/null && git commit -q -m "$2"
}

# --- a layer whose notes all conform is not degraded and names nothing
note valid.md a-valid-note 'class: fact'
"$RB" env status > clean.txt 2>/dev/null
expect_no_grep 'layer_degraded' clean.txt
"$RB" env status --format json > clean.json 2>/dev/null
[ "$(jq -r '.layer.refused // [] | length' clean.json)" = 0 ] \
  || { echo "    a conforming layer named a refused file"; jq -c '.layer' clean.json; exit 1; }

# --- a note with no class: refused, named, and the missing key said out loud
note classless.md a-classless-note ''
"$RB" env status > one.txt 2>/dev/null
expect_grep 'layer_degraded' one.txt
expect_grep 'curated/classless\.md' one.txt
expect_grep '"class" is a required property' one.txt

# --- a class outside the closed set: refused the same way, naming the value
note unknown.md an-unknown-class 'class: rumour'
"$RB" env status > two.txt 2>/dev/null
expect_grep 'curated/unknown\.md' two.txt
expect_grep 'rumour' two.txt

# --- the count and the list are the same answer, and the conforming note is not in it
"$RB" env status --format json > two.json 2>/dev/null
refused="$(jq -r '.layer.refused | length' two.json)"
invalid="$(jq -r '.layer.invalid' two.json)"
[ "$refused" = "$invalid" ] \
  || { echo "    the layer counts $invalid refused file(s) and names $refused"; exit 1; }
[ "$refused" = 2 ] || { echo "    expected 2 refused files, got $refused"; jq -c '.layer.refused' two.json; exit 1; }
jq -e '[.layer.refused[].path] | any(test("valid\\.md"))' two.json >/dev/null 2>&1 \
  && { echo "    the conforming note was named as refused"; exit 1; }
jq -e '.layer.refused | all(.code == "schema_violation")' two.json >/dev/null \
  || { echo "    a refusal carried no code"; jq -c '.layer.refused' two.json; exit 1; }

# --- the warning stays readable: it names three and counts the rest, and the list is whole
note four.md note-four ''
note five.md note-five ''
"$RB" env status > many.txt 2>/dev/null
expect_grep '4 declared file\(s\) did not become objects' many.txt
expect_grep 'and 1 more' many.txt
"$RB" env status --format json > many.json 2>/dev/null
[ "$(jq -r '.layer.refused | length' many.json)" = 4 ] \
  || { echo "    the whole list is not in layer.refused"; exit 1; }

echo "    a refused file is named wherever the layer is said to be degraded"

# A record's prose holds no template delimiter (I3009). A plan record's text is rendered into
# the site's plan pages, and a double brace, a brace-percent or a brace-hash left in it reaches
# the built HTML unresolved and fails the site job: I3007 on 10-18, I3838 on 10-21. `plan
# validate` refuses one where it is written, naming the record, the field and the delimiter in
# words, so that the refusal itself holds none.
#
# What is proved: an issue whose why holds a double brace, an issue whose acceptance criterion
# holds a brace-percent and a milestone whose outcome holds a brace-hash each fail validation
# with a template_delimiter finding naming the record and the field; a single brace passes; a
# clean plan passes; and no line plan validate prints holds a delimiter. This file builds every
# delimiter from one brace, so it holds none either.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
pj_init
b='{'
dbl="$b$b"; pct="${b}%"; hsh="${b}#"

pj_milestone m1
pj_issue I0001 m1
expect_exit 0 "$MJ" plan validate

# --- a single brace is prose, not a delimiter
printf 'why: "A record may say %s and a set like %sa, b} without harm."\n' "$b" "$b" >> .ai/repo/project/issues/I0001.yaml
expect_exit 0 "$MJ" plan validate

has_delim() { grep -qF -e "$dbl" -e "$pct" -e "$hsh" "$1"; }

# --- a double brace in an issue's why
pj_issue I0002 m1
printf 'why: "The template prints %s commit %s on the page."\n' "$dbl" "}}" >> .ai/repo/project/issues/I0002.yaml
"$MJ" plan validate > out.txt 2>&1 && { echo "    a double brace in a why passed plan validate"; cat out.txt; exit 1; }
grep -qE 'template_delimiter.*I0002.*why holds a double brace' out.txt \
  || { echo "    the refusal does not name I0002, its why and the double brace:"; cat out.txt; exit 1; }
! has_delim out.txt || { echo "    the refusal itself holds a delimiter:"; cat out.txt; exit 1; }
rm .ai/repo/project/issues/I0002.yaml

# --- a brace-percent in an acceptance criterion
pj_issue I0003 m1
sed -i.bak "s/^  - The work is done\$/  - \"The loop $pct for f in findings %} draws one row a finding\"/" .ai/repo/project/issues/I0003.yaml
"$MJ" plan validate > out.txt 2>&1 && { echo "    a brace-percent in an acceptance criterion passed"; cat out.txt; exit 1; }
grep -qE 'template_delimiter.*I0003.*acceptance_criteria[^ ]* holds a brace-percent' out.txt \
  || { echo "    the refusal does not name I0003's criterion and the brace-percent:"; cat out.txt; exit 1; }
! has_delim out.txt || { echo "    the refusal itself holds a delimiter:"; cat out.txt; exit 1; }
rm .ai/repo/project/issues/I0003.yaml .ai/repo/project/issues/I0003.yaml.bak

# --- a brace-hash in a milestone's outcome
sed -i.bak "s/^outcome: .*/outcome: \"A comment $hsh like this #} stays out of the page.\"/" .ai/repo/project/milestones/m1.yaml
grep -q "^outcome: " .ai/repo/project/milestones/m1.yaml || { echo "    the fixture milestone carries no outcome to edit"; exit 1; }
"$MJ" plan validate > out.txt 2>&1 && { echo "    a brace-hash in a milestone outcome passed"; cat out.txt; exit 1; }
grep -qE 'template_delimiter.*m1.*outcome holds a brace-hash' out.txt \
  || { echo "    the refusal does not name m1, its outcome and the brace-hash:"; cat out.txt; exit 1; }
! has_delim out.txt || { echo "    the refusal itself holds a delimiter:"; cat out.txt; exit 1; }
mv .ai/repo/project/milestones/m1.yaml.bak .ai/repo/project/milestones/m1.yaml

# --- the clean plan passes again
expect_exit 0 "$MJ" plan validate
echo "    a template delimiter in a record's prose is refused where it is written, and named in words"

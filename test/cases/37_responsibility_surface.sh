# majordomus-covers: none
#
# The responsibilities are a canonical index, not a map inside the site generator.
#
# They were the second: docs/RESPONSIBILITIES.yaml declared ten entries with their files,
# implementation and specification anchors, docs/README.md said the site generator read it,
# and the generator instead carried nine hardcoded titles with claims matched by regular
# expression. `plan` therefore had no owning command and no guarantees on its page, and
# nothing noticed, because nothing compared the two.
#
# This case is the comparison. It runs the generator against a copy so it can break the
# inputs, and every probe asserts it took effect before asserting what it caused.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }

C="$T/copy"
fixture_repo "$C" AGENTS.md docs site/data/marketing.toml site/data/nav.toml site/content-src test/cases test/lib.sh
( cd "$C" && git init -q . && git config user.email t@example.com && git config user.name t && git add -A && git commit -qm fixture ) >/dev/null

took() { grep -q "$1" "$2" || { echo "    the probe did not take: expected /$1/ in $2"; exit 1; }; }
gone() { grep -q "$1" "$2" && { echo "    the probe did not take: /$1/ still in $2"; exit 1; }; return 0; }

expect_exit 0 "$C/scripts/generate-site-data"
R="$C/site/data/generated/readme.json"

# 1. every entry in the canonical file reaches the site, with the fields only it holds
n_yaml="$(grep -c '^  - id:' "$ROOT/docs/RESPONSIBILITIES.yaml")"
n_site="$(jq '.does | length' "$R")"
[ "$n_yaml" = "$n_site" ] || { echo "    RESPONSIBILITIES.yaml has $n_yaml entries, the site has $n_site"; exit 1; }
for id in $(sed -n 's/^  - id: //p' "$ROOT/docs/RESPONSIBILITIES.yaml"); do
  jq -e --arg i "$id" '.does[] | select(.slug==$i)' "$R" >/dev/null \
    || { echo "    $id is in RESPONSIBILITIES.yaml and not on the site"; exit 1; }
  for field in command implementation cli_route schema_route; do
    v="$(jq -r --arg i "$id" --arg f "$field" '.does[] | select(.slug==$i) | .[$f] // ""' "$R")"
    [ -n "$v" ] || { echo "    $id reaches the site without $field"; exit 1; }
  done
  [ "$(jq --arg i "$id" '.does[] | select(.slug==$i) | .files | length' "$R")" -ge 1 ] \
    || { echo "    $id reaches the site with no files"; exit 1; }
done

# 2. claims associate by their own field, not by matching words in their text. Every claim
#    that names a responsibility appears on that responsibility, and nowhere else.
for id in $(sed -n 's/^  - id: //p' "$ROOT/docs/RESPONSIBILITIES.yaml"); do
  want="$(grep -B20 "responsibility: $id\$" "$ROOT/docs/CLAIMS.yaml" | sed -n 's/^  - id: //p' | tail -1)"
  [ -n "$want" ] || continue
  jq -e --arg i "$id" --arg c "$want" '.does[] | select(.slug==$i) | .claim_ids | index($c)' "$R" >/dev/null \
    || { echo "    claim $want declares responsibility $id but is not on its page"; exit 1; }
done
# and no responsibility is left with none, which is what the regular expressions produced
for id in $(sed -n 's/^  - id: //p' "$ROOT/docs/RESPONSIBILITIES.yaml"); do
  [ "$(jq --arg i "$id" '.does[] | select(.slug==$i) | .claim_ids | length' "$R")" -ge 1 ] \
    || { echo "    $id has no claims; the join is not working"; exit 1; }
done

# 3. a README row with no entry in the canonical file is refused, in that direction
cp "$ROOT/README.md" "$C/README.md"
python3 - "$C/README.md" <<'PY' 2>/dev/null || { echo "    python3 absent; skipping the mutations"; exit 0; }
import sys
p = sys.argv[1]
s = open(p).read()
anchor = '| **Watch** |'
assert anchor in s
open(p, 'w').write(s.replace(anchor, '| **Telepathy** | reads the worker\'s mind |\n' + anchor, 1))
PY
took 'Telepathy' "$C/README.md"
rc=0; out="$("$C/scripts/generate-site-data" 2>&1)" || rc=$?
[ "$rc" = 0 ] && { echo "    a README row with no canonical entry was accepted"; exit 1; }
printf '%s\n' "$out" | grep -q 'Telepathy' || {
  echo "    the refusal does not name the row that caused it"; printf '%s\n' "$out" | sed 's/^/      | /'; exit 1; }
cp "$ROOT/README.md" "$C/README.md"
gone 'Telepathy' "$C/README.md"
expect_exit 0 "$C/scripts/generate-site-data"

# 4. an anchor that is not a heading is refused, because a deep link that lands nowhere is
#    worse than no link — it reads as a reference and is not one
sed -i.bak 's|^    cli_anchor: majordomus watch$|    cli_anchor: majordomus telepathy|' "$C/docs/RESPONSIBILITIES.yaml"
rm -f "$C/docs/RESPONSIBILITIES.yaml.bak"
took 'majordomus telepathy' "$C/docs/RESPONSIBILITIES.yaml"
rc=0; out="$("$C/scripts/generate-site-data" 2>&1)" || rc=$?
[ "$rc" = 10 ] || { echo "    an anchor that is not a heading was accepted (exit $rc)"; exit 1; }
printf '%s\n' "$out" | grep -q 'not a heading' || {
  echo "    the refusal does not say what is wrong"; printf '%s\n' "$out" | sed 's/^/      | /'; exit 1; }
cp "$ROOT/docs/RESPONSIBILITIES.yaml" "$C/docs/RESPONSIBILITIES.yaml"

# 5. a file the index names that does not exist is refused
sed -i.bak 's|^    implementation: lib/watch.sh$|    implementation: lib/nosuch.sh|' "$C/docs/RESPONSIBILITIES.yaml"
rm -f "$C/docs/RESPONSIBILITIES.yaml.bak"
took 'lib/nosuch.sh' "$C/docs/RESPONSIBILITIES.yaml"
rc=0; out="$("$C/scripts/generate-site-data" 2>&1)" || rc=$?
[ "$rc" = 10 ] || { echo "    an implementation path that does not exist was accepted (exit $rc)"; exit 1; }
printf '%s\n' "$out" | grep -q 'nosuch' || {
  echo "    the refusal does not name the missing path"; printf '%s\n' "$out" | sed 's/^/      | /'; exit 1; }
cp "$ROOT/docs/RESPONSIBILITIES.yaml" "$C/docs/RESPONSIBILITIES.yaml"
gone 'lib/nosuch.sh' "$C/docs/RESPONSIBILITIES.yaml"
expect_exit 0 "$C/scripts/generate-site-data"

# 6. a claim that declares no responsibility is refused. Under the keyword matching this
#    join replaced, such a claim landed somewhere by accident; under a field join it lands
#    nowhere and would say nothing about it, which is the same silence one level down.
#    `none` is a declared answer; an absent field is not an answer at all.
# `0,/pat/` is a GNU address form; BSD sed has no equivalent, and bash 3.2 with BSD
# userland is this repository's floor. awk drops the first match portably.
awk '!d && /^    responsibility: doctor$/ { d=1; next } { print }' \
  "$C/docs/CLAIMS.yaml" > "$T/claims.probe" && mv "$T/claims.probe" "$C/docs/CLAIMS.yaml"
before="$(grep -c '^    responsibility:' "$ROOT/docs/CLAIMS.yaml")"
after="$(grep -c '^    responsibility:' "$C/docs/CLAIMS.yaml")"
[ "$after" -lt "$before" ] || { echo "    the probe did not take: no responsibility field was removed"; exit 1; }
rc=0; out="$("$C/scripts/generate-site-data" 2>&1)" || rc=$?
[ "$rc" = 10 ] || { echo "    a claim with no responsibility was accepted (exit $rc)"; exit 1; }
printf '%s\n' "$out" | grep -q 'no responsibility field' || {
  echo "    the refusal does not say what is missing"; printf '%s\n' "$out" | sed 's/^/      | /'; exit 1; }
cp "$ROOT/docs/CLAIMS.yaml" "$C/docs/CLAIMS.yaml"

# 7. and a claim naming a responsibility that does not exist is refused, in the other
#    direction — a join is only as good as both of its sides
awk '!d && /^    responsibility: doctor$/ { print "    responsibility: nosuchthing"; d=1; next } { print }' \
  "$C/docs/CLAIMS.yaml" > "$T/claims.probe" && mv "$T/claims.probe" "$C/docs/CLAIMS.yaml"
took 'responsibility: nosuchthing' "$C/docs/CLAIMS.yaml"
rc=0; out="$("$C/scripts/generate-site-data" 2>&1)" || rc=$?
[ "$rc" = 10 ] || { echo "    a claim naming an unknown responsibility was accepted (exit $rc)"; exit 1; }
printf '%s\n' "$out" | grep -q 'does not exist' || {
  echo "    the refusal does not name the problem"; printf '%s\n' "$out" | sed 's/^/      | /'; exit 1; }
cp "$ROOT/docs/CLAIMS.yaml" "$C/docs/CLAIMS.yaml"

printf '    %s responsibilities, each with its files, command, anchors and claims derived\n' "$n_yaml"

# majordomus-covers: plan
# `plan evidence` puts the new entry inside the `evidence:` sequence wherever the key sits in
# the record, and leaves every other line as it was (I4661).
#
# It used to append the entry to the end of the file. That is the end of the sequence only
# while `evidence:` is the record's last key; on 2026-11-19 an OpenTTD issue whose
# `evidence:` was followed by `started_at:`, `started_event:` and `updated_at:` got its entry
# after `updated_at:`, a list item without a parent key, and `plan validate` refused the
# record until a person moved the entry back by hand.
#
#   1  evidence: followed by two keys: the entry ends the sequence, before the first of them
#   2  the sequence's items in column 0: the entry is in column 0 too
#   3  evidence: with no item yet, followed by a key: the entry is its first item
#   4  no evidence: key: it is created at the end with the entry
#   5  evidence: as the last key, a milestone's: the entry ends the file, as it always did
# Each record validates afterwards, the tool reads both entries back, and the only lines that
# differ from the record before are the entry's and `updated_at:`.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true

pj_init
pj_milestone M000
I=.ai/repo/project/issues
fail=0
note() { printf '    %s\n' "$*"; fail=1; }

# the evidence block of one earlier run, indented by $1
old_item() {
  printf '%s- covers: "proof"\n%s  type: test\n%s  command: "first"\n' "$1" "$1" "$1"
  printf '%s  commit: abc\n%s  recorded_at: 2026-11-18T05:45:41Z\n' "$1" "$1"
}

pj_issue I0001 M000
{ printf 'evidence:\n'; old_item '  '
  printf 'created_at: %s\nupdated_at: 2026-11-18T05:45:58Z\n' "'2026-11-18'"; } >> "$I/I0001.yaml"
pj_issue I0002 M000
{ printf 'evidence:\n'; old_item ''; printf 'updated_at: 2026-11-18T05:45:58Z\n'; } >> "$I/I0002.yaml"
pj_issue I0003 M000
printf 'evidence:\nupdated_at: 2026-11-18T05:45:58Z\n' >> "$I/I0003.yaml"
pj_issue I0004 M000
{ printf 'evidence:\n'; old_item '  '; } >> .ai/repo/project/milestones/M000.yaml
git add -A >/dev/null; git commit -qm fixture >/dev/null
expect_exit 0 "$MJ" plan validate || { echo "    the fixture itself does not validate"; exit 1; }
B="$(mktemp -d)"
trap 'rm -rf "$B"' EXIT
for id in I0001 I0002 I0003 I0004; do cp "$I/$id.yaml" "$B/$id.yaml"; done
cp .ai/repo/project/milestones/M000.yaml "$B/M000.yaml"

for id in I0001 I0002 I0003 I0004 M000; do
  expect_exit 0 "$MJ" plan evidence "$id" --covers proof --type test --command "second $id" \
    --result "it held" || exit 1
done
expect_exit 0 "$MJ" plan validate || { echo "    a record no longer validates after plan evidence"; exit 1; }

# rec <id>: the record's file now
rec() { if [ -f "$I/$1.yaml" ]; then printf '%s' "$I/$1.yaml"; else printf '%s' ".ai/repo/project/milestones/$1.yaml"; fi; }
# The six lines of the new entry with indentation $2, read from the record's line where the
# entry starts, and the line that follows them.
entry_at() {
  local f; f="$(rec "$1")"
  awk -v p="$2" -v id="$1" '
    $0 == p "- covers: \"proof\"" { s = NR }
    s && NR == s + 2 && $0 == p "  command: \"second " id "\"" { at = s }
    END { print at + 0 }' "$f"
}
check() {
  local id="$1" ind="$2" next="$3" f at want got
  f="$(rec "$id")"
  at="$(entry_at "$id" "$ind")"
  [ "$at" -gt 0 ] || { note "$id: no entry with the sequence's indentation '$ind'"; return 0; }
  grep -qE "^${ind}  commit: [0-9a-f]{40}\$" <(sed -n "$((at + 4))p" "$f") || note "$id: the entry's commit line is not where it belongs"
  grep -qE "^${ind}  recorded_at: " <(sed -n "$((at + 5))p" "$f") || note "$id: the entry's recorded_at line is not where it belongs"
  [ "$(sed -n "$((at + 1))p" "$f")" = "${ind}  type: test" ] || note "$id: the entry's type line is not where it belongs"
  [ "$(sed -n "$((at + 3))p" "$f")" = "${ind}  result: \"it held\"" ] || note "$id: the entry's result line is not where it belongs"
  got="$(sed -n "$((at + 6))p" "$f")"
  case "$next" in
    EOF) [ -z "$got" ] && [ "$(wc -l < "$f")" -eq $((at + 5)) ] || note "$id: the entry does not end the file: '$got' follows it" ;;
    *) case "$got" in "$next"*) ;; *) note "$id: the entry is followed by '$got', not by $next" ;; esac ;;
  esac
  # every other line byte-identical: the record without the entry and without updated_at is
  # the record before without updated_at
  want="$(grep -v '^updated_at:' "$B/$id.yaml")"
  grep -q '^evidence:' "$B/$id.yaml" || want="$(printf '%s\nevidence:' "$want")"
  got="$(sed "${at},$((at + 5))d" "$f" | grep -v '^updated_at:')"
  [ "$want" = "$got" ] || { note "$id: a line other than the entry and updated_at changed:"; diff <(printf '%s\n' "$want") <(printf '%s\n' "$got") | sed 's/^/      /'; }
  # and the tool reads both entries, the old one first where there was one
  expect_exit 0 "$MJ" plan show "$id" || { note "$id: plan show failed"; return 0; }
  printf '%s\n' "$LAST_OUT" | grep -qE "^\| proof \| test \| \`second $id\` \| it held \|" || note "$id: plan show does not list the new entry"
}
check I0001 '  ' 'created_at:'
check I0002 '' 'updated_at:'
check I0003 '  ' 'updated_at:'
check I0004 '  ' EOF
check M000 '  ' EOF
grep -qx 'evidence:' "$I/I0004.yaml" || note "I0004: the evidence: key was not created"
for id in I0001 I0002 M000; do
  expect_exit 0 "$MJ" plan show "$id"
  printf '%s\n' "$LAST_OUT" | grep -qE '^\| proof \| test \| `first` \|' || note "$id: the earlier entry is no longer read"
done
# updated_at is written where it stood, and once
for id in I0001 I0002 I0003; do
  [ "$(grep -c '^updated_at:' "$I/$id.yaml")" = 1 ] || note "$id: updated_at is not one line"
done

[ "$fail" = 0 ] || exit 1
echo "    plan evidence ends the evidence: sequence wherever the key sits, column 0 or indented, empty or absent; nothing else moves"

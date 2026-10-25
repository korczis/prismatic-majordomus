# majordomus-covers: none
# majordomus-negative: rules
# A translation-memory entry keyed the way the build no longer computes is refused (I3777).
#
# I3758 made a commit id, a version or a date that is the whole text of an inline element a void
# placeholder, and scripts/site-translate rekey moved the memory once. An entry written in the old
# form afterwards (an import from an old missing.json, a row of a later train) is reached by no
# page, and its sentence reads in English. scripts/site-check --memory, check 22 of the full run,
# refuses such an entry with its key, the pairs and the remedy; passes the same entry once it is
# in the new form; and refuses an entry whose translation lost the value, which rekey cannot move.
# Nothing in the repository is written.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/site-check"
[ -x "$CHECK" ] || { echo "    scripts/site-check is missing"; exit 1; }

mem="$PWD/i18n/cs/memory"
mkdir -p "$mem"
entry() { printf '%s\n' "$1" > "$mem/aa.jsonl"; }

# 1. an entry keyed the old way is refused, naming its key, the value and the remedy
entry '{"k":"aa0000000000000000a1","en":"Changes the release record of <t1>v0.28.0</t1> carries","cs":"Změny, které nese záznam vydání <t1>v0.28.0</t1>"}'
rc=0; "$CHECK" --memory "$PWD/i18n" > out.txt 2>&1 || rc=$?
[ "$rc" = 10 ] || { echo "    an old-form entry was not refused (exit $rc)"; cat out.txt; exit 1; }
grep -q 'cs entry aa0000000000000000a1 carries <t1>v0.28.0</t1>' out.txt || { echo "    the finding does not name the entry and its value"; cat out.txt; exit 1; }
grep -q 'site-translate rekey --lang cs' out.txt || { echo "    the finding does not name the remedy"; cat out.txt; exit 1; }

# 2. the same entry in the form the build computes passes
entry '{"k":"aa0000000000000000a2","en":"Changes the release record of <t1/> carries","cs":"Změny, které nese záznam vydání <t1/>"}'
rc=0; "$CHECK" --memory "$PWD/i18n" > out.txt 2>&1 || rc=$?
[ "$rc" = 0 ] || { echo "    an entry in the new form was refused (exit $rc)"; cat out.txt; exit 1; }
grep -q '^OK   i18n' out.txt || { echo "    a clean memory reported no OK line"; cat out.txt; exit 1; }

# 3. an entry whose translation lost the value cannot be moved, and is refused for that
entry '{"k":"aa0000000000000000a3","en":"inferred at <t1>a8d7dec</t1>","cs":"odvozeno u <t1>jiného</t1>"}'
rc=0; "$CHECK" --memory "$PWD/i18n" > out.txt 2>&1 || rc=$?
[ "$rc" = 10 ] && grep -q 'lost that value' out.txt || { echo "    an entry whose translation lost the value was not refused as such (exit $rc)"; cat out.txt; exit 1; }

# 4. the full run calls the same rule
grep -q '^memory_keyed "\$SITE/i18n"' "$CHECK" || { echo "    the full site-check run does not call memory_keyed"; exit 1; }
echo "    an old-keyed or unmovable memory entry is refused with its remedy, a moved one passes"

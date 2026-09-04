# The status vocabulary is declared once and read everywhere.
#
# Every surface used to name done, ready, active, verify and blocked itself: five tiles in a
# template, seven columns in a table, seven keys in a JSON object. Each was a second copy of
# a list the engine already owns, and a status added to the engine reached none of them.
#
# Nothing here enumerates the statuses. The declaration is read out of the engine, the
# assignments are read out of the same file, and the surfaces are asked whether they carry
# what those two agree on.
. "$ROOT/test/lib.sh"
AWKF="$ROOT/lib/project.awk"

# --- the declaration and the assignments agree, in both directions
declared() { # kind: ISTATUS or MSTATUS
  sed -n "s/^  $1 = \"\\(.*\\)\"\$/\\1/p" "$AWKF" | tr ' ' '\n' | sort -u
}
assigned() { # variable the derivation assigns: st or ms
  grep -oE "(^|[^a-z_])$1 = \"[A-Z_]+\"" "$AWKF" | sed -E 's/.*"([A-Z_]+)".*/\1/' | sort -u
}
for pair in "ISTATUS st" "MSTATUS ms"; do
  set -- $pair
  d="$(declared "$1")"; a="$(assigned "$2")"
  [ -n "$d" ] || { echo "    $1 is not declared in lib/project.awk"; exit 1; }
  [ -n "$a" ] || { echo "    nothing assigns $2 in lib/project.awk; this case cannot check it"; exit 1; }
  missing="$(printf '%s\n' "$a" | grep -vxF -f <(printf '%s\n' "$d") || true)"
  [ -z "$missing" ] || { echo "    assigned but not declared in $1: $(printf '%s' "$missing" | tr '\n' ' ')"; exit 1; }
  extra="$(printf '%s\n' "$d" | grep -vxF -f <(printf '%s\n' "$a") || true)"
  [ -z "$extra" ] || { echo "    declared in $1 but never assigned: $(printf '%s' "$extra" | tr '\n' ' ')"; exit 1; }
done

# --- the vocabulary reaches the command surface, and the counts are keyed by it
"$MJ" init >/dev/null
pj_init
pj_milestone M000
pj_issue I0001 M000
pj_issue I0002 M000 I0001
expect_exit 0 "$MJ" plan validate
st_json="$("$MJ" --json plan status)"
vocab="$(printf '%s' "$st_json" | sed -n 's/.*"statuses":\[\([^]]*\)\].*/\1/p' | tr -d '"' | tr ',' ' ')"
[ -n "$vocab" ] || { echo "    plan status does not carry the status vocabulary"; exit 1; }
for st in $vocab; do
  printf '%s' "$st_json" | grep -qF "\"$st\":" \
    || { echo "    the counts carry no entry for the declared status $st"; exit 1; }
done
printf '%s\n' "$vocab" | tr ' ' '\n' | sort > "$T/declared.txt"
printf '%s' "$st_json" | sed -n 's/.*"by_status":{\([^}]*\)}.*/\1/p' | tr ',' '\n' \
  | sed 's/:.*//; s/"//g' | sort > "$T/counted.txt"
diff "$T/declared.txt" "$T/counted.txt" >/dev/null \
  || { echo "    the counts and the declared vocabulary are different lists"; diff "$T/declared.txt" "$T/counted.txt"; exit 1; }

# --- the denominator is the one the engine derives DONE from, not the raw total
"$MJ" plan start I0001 >/dev/null
"$MJ" plan evidence I0001 --covers proof --type test --command true --result ok >/dev/null
"$MJ" plan "done" I0001 >/dev/null
printf 'cancelled: true\n' >> .majordomus/project/issues/I0002.yaml
expect_exit 0 "$MJ" plan validate
[ "$("$MJ" --json plan status | sed -n 's/.*"required":\([0-9]*\).*/\1/p')" = 1 ] \
  || { echo "    a cancelled issue still counts towards the denominator"; "$MJ" --json plan status; exit 1; }
"$MJ" plan status | grep -qE ' 1/1 ' \
  || { echo "    the table denominator does not match the derivation"; "$MJ" plan status; exit 1; }

# --- a status added to the engine reaches every surface without any of them being edited.
# The tool is copied so the mutation cannot escape this case; the site generator is included
# because it is the surface most likely to hold a list of its own.
mkdir -p "$T/tool"
cp -R "$ROOT/bin" "$ROOT/lib" "$ROOT/share" "$ROOT/docs" "$ROOT/scripts" "$ROOT/site" "$T/tool/"
cp "$ROOT/README.md" "$ROOT/LICENSE" "$T/tool/"
rm -rf "$T/tool/site/public"
cd "$T/tool" || exit 1
git init -q .; git config user.email t@example.com; git config user.name t; git commit -q --allow-empty -m init
MJ2="$T/tool/bin/majordomus"
"$MJ2" init >/dev/null
pj_init
pj_milestone M000
pj_issue I0001 M000
expect_exit 0 "$MJ2" plan validate
before="$("$MJ2" --json plan status)"
sed -i.bak 's/^  ISTATUS = "\(.*\)"$/  ISTATUS = "\1 PARKED"/' "$T/tool/lib/project.awk"; rm -f "$T/tool/lib/project.awk.bak"
after="$("$MJ2" --json plan status)"
[ "$before" != "$after" ] || { echo "    a status added to the engine changed no surface"; exit 1; }
printf '%s' "$after" | grep -qF '"PARKED":0' \
  || { echo "    the new status is declared but the counts do not carry it"; printf '%s\n' "$after"; exit 1; }
printf '%s' "$after" | grep -qF '"PARKED"' \
  || { echo "    the new status does not reach the vocabulary the command publishes"; exit 1; }
if command -v jq >/dev/null 2>&1 && "$T/tool/scripts/generate-site-data" --out "$T/gen" >/dev/null 2>&1; then
  jq -e '.statuses.issue | index("PARKED") != null' "$T/gen/plan.json" >/dev/null \
    || { echo "    the generated site data does not carry the new status"; exit 1; }
  jq -e '.milestones[0].counts.by_status | has("PARKED")' "$T/gen/plan.json" >/dev/null \
    || { echo "    the generated milestone counts do not carry the new status"; exit 1; }
fi

# majordomus-covers: none
# Every schema this tool ships is examined, and each one is applied by something.
#
# The check in lib/doctor.sh globbed share/schemas/*.schema.json — one level, while every
# schema sits two directories down, because the identifier `majordomus.adr/v1` fixes the
# path share/schemas/majordomus/adr/adr.v1.schema.json. It therefore examined nothing and
# reported that nothing was wrong. This case is the reason that cannot happen twice: it
# counts the files on disk and requires the doctrine to have looked at all of them.
. "$ROOT/test/lib.sh"

n="$(find "$ROOT/share/schemas" -name '*.schema.json' | wc -l | tr -d ' ')"
[ "$n" -gt 0 ] || { echo "    no schemas found at all; this case is measuring the wrong tree"; exit 1; }

out="$(cd "$ROOT" && bin/majordomus doctor 2>&1)" || true
line="$(printf '%s' "$out" | grep -E '^(OK|FAIL|WARN)[[:space:]]+schema[[:space:]]+share/schemas ' | head -1)"
[ -n "$line" ] || { echo "    doctor reports no verdict on share/schemas"; exit 1; }

case "$line" in
  OK*) ;;
  *) echo "    $line"; exit 1 ;;
esac

# the count it reports is the count on disk: a check that examines fewer files than exist
# is the defect this case was written for, and it looks exactly like a pass
got="$(printf '%s' "$line" | sed -n 's/.*— \([0-9][0-9]*\) schema(s).*/\1/p')"
[ -n "$got" ] || { echo "    doctor does not say how many schemas it examined: $line"; exit 1; }
[ "$got" = "$n" ] \
  || { echo "    $n schema(s) on disk and the doctrine examined $got"; exit 1; }

# majordomus-covers: none
# claims: none
# A payload the generators pass to jq is read from a file, not passed through argv.
#
# Linux caps each single argument at MAX_ARG_STRLEN (131072 bytes), independently of the
# total; macOS caps only the total. So a payload that grows past 128KB keeps working on every
# developer's machine and dies on the runner with `jq: Argument list too long`, after which
# the site data is never written and thirteen site cases fail on data that does not exist.
# That is what happened to `git ls-files` (~130KB) and to the claims array (~135KB).
#
# The fix was to read both from a file. This case holds that shape, because the failure is
# invisible on the machine where the change would be written: it asserts that the two payloads
# are read rather than passed, and it measures the real payloads against the cap so that a
# third one crossing it is reported here rather than on the runner.
#
# What it cannot do is prove the general rule for every jq call in every script; it names the
# two that have crossed the cap and the sizes of the payloads they carry.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq is required"; exit 1; }

G="$ROOT/scripts/generate-site-data"
J="$ROOT/scripts/lib/executable-site.jq"
[ -f "$G" ] || { echo "    $G is not there"; exit 1; }
[ -f "$J" ] || { echo "    $J is not there"; exit 1; }

# --- the cap itself, and the two payloads measured against it
CAP=131072
tracked_bytes="$(git -C "$ROOT" ls-files | wc -c | tr -d ' ')"
claims_bytes=0
CAPS="$ROOT/site/data/generated/capabilities.json"
[ -f "$CAPS" ] || { echo "    $CAPS is not there; the claims payload cannot be measured"; exit 1; }
claims_bytes="$(jq -c '.claims' "$CAPS" | wc -c | tr -d ' ')"
[ "$claims_bytes" -gt 1000 ] || { echo "    the claims payload measured $claims_bytes bytes, which means this case is reading the wrong file"; exit 1; }
printf '    tracked listing %s bytes, claims %s bytes, cap %s\n' "$tracked_bytes" "$claims_bytes" "$CAP"

# --- the tracked listing is read, never passed
grep -q -- '--rawfile files' "$G" \
  || { echo "    forge_paths no longer reads the tracked listing with --rawfile; over $CAP bytes it dies on Linux only"; exit 1; }
if grep -q -- '--arg files "$(git' "$G"; then
  echo "    the tracked listing is passed through argv again (--arg files \"\$(git ...)\"); Linux caps one argument at $CAP bytes"; exit 1
fi

# --- the claims array is read, never passed
grep -q -- '--slurpfile claims "$CLAIMS_TMP"' "$G" \
  || { echo "    the executable projection no longer reads the claims with --slurpfile"; exit 1; }
if grep -q -- '--argjson claims "$(' "$G"; then
  echo "    the claims array is passed through argv again (--argjson claims \"\$(...)\"); Linux caps one argument at $CAP bytes"; exit 1
fi
grep -q '\$claims\[0\]' "$J" \
  || { echo "    executable-site.jq does not read \$claims[0]; --slurpfile wraps the array in one"; exit 1; }

# --- a payload that has crossed the cap and is still passed through argv is the failure this
#     case exists to name. Both are read from files now, so the sizes are reported, not refused.
if [ "$tracked_bytes" -gt "$CAP" ]; then
  printf '    the tracked listing is over the cap (%s > %s) and is read from a file: correct\n' "$tracked_bytes" "$CAP"
fi
if [ "$claims_bytes" -gt "$CAP" ]; then
  printf '    the claims array is over the cap (%s > %s) and is read from a file: correct\n' "$claims_bytes" "$CAP"
fi

# --- the temporary files the two sites create are removed again
grep -q 'rm -f "$tracked"' "$G" || { echo "    forge_paths leaks its temporary file"; exit 1; }
grep -q 'rm -f "$CLAIMS_TMP"' "$G" || { echo "    the claims temporary file is not removed"; exit 1; }

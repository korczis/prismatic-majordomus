# ... and an active task that owes an obligation, with work done inside its scope. The
# obligation is written into the record the way a caller would declare it, because what the
# scenario is about is the discharge, not how the list got there.
. "$FIXTURE_SETUP/installed.sh"
"$MJ" start "narrow the parser" --scope lib >/dev/null
echo work >> lib/a
python3 - <<'PY'
import io
p = '.ai/local/state/current.yaml'
s = io.open(p, encoding='utf-8').read()
io.open(p, 'w', encoding='utf-8').write(s.replace('outcome:', 'requires:\n  - tests\noutcome:'))
PY

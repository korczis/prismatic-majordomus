# ... and an active task that owes what reaches past the working tree: the change is
# committed nowhere and pushed nowhere, and the record says the task promised both. The work
# sits in the tree on purpose — the point of the scenario is that a token whose fact the tool
# can hold is settled by asking, not by a worker saying so.
. "$FIXTURE_SETUP/installed.sh"
"$MJ" start "publish the parser fix" --scope lib >/dev/null
echo work >> lib/a
python3 - <<'PY'
import io
p = '.ai/local/state/current.yaml'
s = io.open(p, encoding='utf-8').read()
io.open(p, 'w', encoding='utf-8').write(s.replace('outcome:', 'requires:\n  - commit\n  - push\noutcome:'))
PY

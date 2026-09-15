# majordomus-covers: none
# What a lease contest is judged by is declared, and the declaration is enforced.
#
# One server serves a checkout and a lease file decides which process owns it. Three numbers
# judge a contest between two processes, and until 2026-09-15 all three were compiled
# constants in apps/majordomus-cli/src/lease.rs: `BIND_GRACE` 15s, `PROBE_TIMEOUT` 2s,
# `JOIN_TIMEOUT` 20s. `probe_timeout` is the one that decides whether a live-but-slow owner
# keeps its lease or is taken over while it is still serving — the race
# `three_ensures_at_once_share_one_server` kept losing, on branches that had nothing to do
# with it. A decision about how this repository is supervised, unchangeable without a
# rebuild, stated nowhere a reader would look, and invisible to every projection.
#
# Three things are asserted here, and they fail for three different reasons:
#
#   1. the schema owns the shape — so an undeclared key cannot enter by being written;
#   2. this repository declares the block — so the values are stated where they are read;
#   3. the schema still refuses what it should — the mutation, without which §1 and §2 would
#      both pass over a schema that accepted anything.
#
# What this case deliberately does NOT assert: that a changed value changes behaviour. That is
# a property of the mapping, not of the files, and it is held where it can be held honestly —
# `Timings::from_policy`'s doc test, which drives the pure function. Asserting it here would
# mean starting servers and racing them, and a timing race in a test suite is the thing this
# whole branch exists to remove.
. "$ROOT/test/lib.sh"

SCHEMA="$ROOT/share/schemas/majordomus/policy/policy.v1.schema.json"
POLICY="$ROOT/.ai/repo/policy.yaml"
[ -f "$SCHEMA" ] || { echo "    no policy schema at share/schemas/majordomus/policy/"; exit 1; }
[ -f "$POLICY" ] || { echo "    no policy at .ai/repo/policy.yaml"; exit 1; }

# --- 1. the schema owns the block, and every key it may carry
python3 - "$SCHEMA" <<'PY' || exit 1
import json, sys
s = json.load(open(sys.argv[1], encoding='utf-8'))
srv = s.get('properties', {}).get('server')
if srv is None:
    print("    the policy schema declares no `server:` block, so the lease timings are owned"
          " by nothing and a policy naming them would be refused")
    sys.exit(1)
if srv.get('additionalProperties') is not False:
    print("    the `server:` block accepts additional properties, so a misspelt timing would"
          " be carried silently and read as absent")
    sys.exit(1)
want = {'probe_timeout_seconds', 'bind_grace_seconds', 'join_timeout_seconds'}
have = set(srv.get('properties', {}))
missing = want - have
if missing:
    print(f"    the `server:` block does not own {', '.join(sorted(missing))}")
    sys.exit(1)
for k in sorted(want):
    if not (srv['properties'][k].get('description') or '').strip():
        print(f"    server.{k} is declared with no description; a number whose meaning is"
              " not written down is a constant with a longer path")
        sys.exit(1)
print("    the schema owns `server:` and every timing it may carry, each with its meaning")
PY

# --- 2. this repository declares them
for k in probe_timeout_seconds bind_grace_seconds join_timeout_seconds; do
  grep -qE "^  $k: [0-9]+$" "$POLICY" \
    || { echo "    .ai/repo/policy.yaml declares no server.$k; the code would fall back to the"
         echo "    compiled constant and nothing would say which value is in force"; exit 1; }
done
echo "    this repository declares every lease timing it is judged by"

# --- 3. the mutation: the schema still refuses a key it does not own
# Without this, §1 and §2 pass over a schema that accepts anything — the block would be
# decoration and a typo in a timing would be read as "absent, keep the constant".
command -v python3 >/dev/null 2>&1 || { echo "    skip: no python3 for the refusal check"; exit 0; }
python3 - "$SCHEMA" <<'PY' || exit 1
import json, sys
s = json.load(open(sys.argv[1], encoding='utf-8'))
srv = s['properties']['server']

def accepts(doc):
    """The subset of JSON Schema this block uses, applied by hand: unknown keys and the
    declared types. Hand-applied rather than with a validator, because the case must not
    depend on a library the repository does not vendor."""
    if srv.get('additionalProperties') is False:
        for k in doc:
            if k not in srv.get('properties', {}):
                return False
    for k, v in doc.items():
        spec = srv['properties'].get(k, {})
        if spec.get('type') == 'integer' and not isinstance(v, int):
            return False
        if 'minimum' in spec and isinstance(v, int) and v < spec['minimum']:
            return False
    return True

if not accepts({'probe_timeout_seconds': 2}):
    print("    the schema refuses a value it is supposed to own"); sys.exit(1)
if accepts({'probe_timeout_sec': 2}):
    print("    the schema accepts `probe_timeout_sec`, a key it does not own: a misspelt"
          " timing would be read as absent and the constant used instead")
    sys.exit(1)
if accepts({'probe_timeout_seconds': 0}):
    print("    the schema accepts a probe timeout of 0, which judges every owner silent"
          " before it can answer")
    sys.exit(1)
print("    a key the block does not own, and a timing below its minimum, are both refused")
PY

echo "    a lease contest is judged by a declaration, and the declaration is owned"

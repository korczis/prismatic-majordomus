# majordomus-covers: none
# The version is measured against the public surface, not claimed.
#
# `scripts/ci/version-matches-surface` compares the capability registry this repository
# commits at every commit — the one declaration MCP, HTTP, OpenAPI and the command line are
# projections of — between the last release and the tree, and refuses a version smaller than
# what moved. This case builds the four trees the gate has to tell apart, in fixtures of its
# own, so that it is measured by what it decides rather than by what its own repository
# happens to contain.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq absent; skipping"; exit 0; }
GATE="$ROOT/scripts/ci/version-matches-surface"

# A repository with a released version and a surface: one capability, exposed three ways.
# `atoms_of` reads nothing else, so a fixture needs nothing else.
fixture() {  # fixture <dir> <released version>
  local d="$1" v="$2"
  mkdir -p "$d/docs/generated" "$d/bin"
  ( cd "$d" && git init -q . && git config user.email t@example.com && git config user.name t )
  cat > "$d/docs/generated/registry.json" <<JSON
{"schema":"majordomus/capability-registry/v1","capabilities":[
 {"id":"alpha.list","kind":"query","visibility":"public",
  "exposure":{"mcp":{"tool":"majordomus_alpha"},"http":{"method":"GET","path":"/api/v1/alpha"},"cli":{"path":["alpha","list"]}}},
 {"id":"alpha.get","kind":"query","visibility":"public",
  "exposure":{"http":{"method":"GET","path":"/api/v1/alpha/get"}}}
]}
JSON
  printf 'MJ_VERSION="%s"\n' "$v" > "$d/bin/majordomus"
  ( cd "$d" && git add -A && git commit -qm released && git tag "v$v" )
}

# Change the tree's declared version without touching the surface.
declare_version() { printf 'MJ_VERSION="%s"\n' "$2" > "$1/bin/majordomus"; }

# ---------------------------------------------------------------- an unchanged surface
# Most commits are behind the boundary, and this gate runs over every tree: a surface that
# did not move owes no bump at all, or the gate would refuse every ordinary commit and be
# worked around within a day.
T1="$T/unchanged"; fixture "$T1" 0.3.1
expect_exit 0 env MJ_ROOT="$T1" "$GATE"
expect_grep 'no bump is owed'
expect_no_grep 'BREAKING'
expect_no_grep 'REFUSE'
# even when the version stood still, which is what an ordinary commit looks like
declare_version "$T1" 0.3.1
expect_exit 0 env MJ_ROOT="$T1" "$GATE"

# ---------------------------------------------------------------- something arrives
T2="$T/added"; fixture "$T2" 0.3.1
python3 - "$T2/docs/generated/registry.json" <<'PY'
import json, sys
p = sys.argv[1]; d = json.load(open(p))
d["capabilities"].append({"id": "beta.list", "kind": "query", "visibility": "public",
  "exposure": {"http": {"method": "GET", "path": "/api/v1/beta"}}})
json.dump(d, open(p, "w"))
PY
# a patch is not enough for an addition, at 0.x or anywhere
declare_version "$T2" 0.3.2
expect_exit 10 env MJ_ROOT="$T2" "$GATE"
expect_grep 'minor required'
expect_grep 'only ask for it by version'
expect_grep 'release bump --level minor'
# a minor is
declare_version "$T2" 0.4.0
expect_exit 0 env MJ_ROOT="$T2" "$GATE"
expect_no_grep 'BREAKING'

# ---------------------------------------------------------------- something is taken away
T3="$T/removed"; fixture "$T3" 0.3.1
python3 - "$T3/docs/generated/registry.json" <<'PY'
import json, sys
p = sys.argv[1]; d = json.load(open(p))
# the capability survives; the command line that reached it does not
del d["capabilities"][0]["exposure"]["cli"]
json.dump(d, open(p, "w"))
PY
declare_version "$T3" 0.3.2
expect_exit 10 env MJ_ROOT="$T3" "$GATE"
expect_grep 'major implied'
expect_grep 'BREAKING'
expect_grep 'command   alpha list'
expect_grep 'breaks on 0.3.2'
# below 1.0.0 a minor carries it — and the removal is still named, which is the point
declare_version "$T3" 0.4.0
expect_exit 0 env MJ_ROOT="$T3" "$GATE"
expect_grep 'BREAKING'
expect_grep 'command   alpha list'
expect_grep 'belongs in the release record'

# ---------------------------------------------------------------- above 1.0.0 the shift ends
T4="$T/stable"; fixture "$T4" 1.2.0
python3 - "$T4/docs/generated/registry.json" <<'PY'
import json, sys
p = sys.argv[1]; d = json.load(open(p))
d["capabilities"] = [c for c in d["capabilities"] if c["id"] != "alpha.get"]
json.dump(d, open(p, "w"))
PY
declare_version "$T4" 1.3.0
expect_exit 10 env MJ_ROOT="$T4" "$GATE"
expect_grep 'major required'
expect_grep 'breaks on 1.3.0'
declare_version "$T4" 2.0.0
expect_exit 0 env MJ_ROOT="$T4" "$GATE"

# ---------------------------------------------------------------- what it refuses to guess
T5="$T/nothing"; mkdir -p "$T5" && ( cd "$T5" && git init -q . )
expect_exit 12 env MJ_ROOT="$T5" "$GATE"
expect_grep 'no version tag'
expect_exit 12 env MJ_ROOT="$T1" "$GATE" --since v9.9.9
expect_grep 'not a commit'

# ---------------------------------------------------------------- and it can just say the level
expect_exit 0 env MJ_ROOT="$T3" "$GATE" --implied
[ "$LAST_OUT" = "major" ] || { echo "    --implied said '$LAST_OUT', not the implied level"; exit 1; }

echo "    the version is measured against the surface: a removal is named, a version smaller than the change is refused, and the 0.x shift ends at 1.0.0"

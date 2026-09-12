# majordomus-covers: none
# The gate of project.mesh-is-observation-not-authority, driven against fixture trees.
#
# ADR 0043 gives network discovery three boundaries a grep can hold, and
# `scripts/ci/mesh-check` is the grep. Until this case it was a gate nobody had seen fail:
# no test in the suite named it, so "a second UDP socket outside src/mesh/ is refused" was
# a claim about a program that had only ever been run on a tree that satisfies it — which
# is indistinguishable from a program that returns 0 unconditionally.
#
# 130_mesh.sh is the mesh's behavioural case: the command line, the declaration, the
# self-check. It proves the subsystem works. It cannot prove that the *boundary* around the
# subsystem is enforced, because enforcement is a property of a tree that breaks it, and
# this checkout does not break it. So each boundary is exercised here by writing the
# violation into a fixture:
#
#   1. a UDP socket outside apps/majordomus-cli/src/mesh/ — a second discovery mechanism
#      growing outside the provider contract;
#   2. a second MeshRegistry construction site — a second truth about who exists;
#   3. a declaration whose kind, schema and rule have come apart — an object nothing
#      validates.
#
# The exemptions are exercised too, because a gate that also catches the crate's own tests
# or a document describing the mesh is one people route around within a week.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/mesh-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }

# A tree of the shape the gate reads: the crate's source with a mesh module that legally
# binds a socket and legally builds the one registry, and a complete declaration — the kind
# in share/kinds.yaml, the schema beside it, the rule that holds it.
fixture() {   # fixture <n> -> prints the tree
  F="$T/tree$1"
  C="$F/apps/majordomus-cli/src"
  mkdir -p "$C/mesh" "$F/share/schemas/majordomus/mesh-declaration" \
           "$F/.ai/repo/rules/project" "$F/apps/majordomus-cli/tests" "$F/docs"
  cat > "$C/mesh/net.rs" <<'RS'
use std::net::UdpSocket;
pub fn bind() -> std::io::Result<UdpSocket> { UdpSocket::bind("0.0.0.0:0") }
RS
  cat > "$C/mesh/registry.rs" <<'RS'
pub struct MeshRegistry;
impl MeshRegistry {
    pub fn new() -> Self { MeshRegistry }
}
pub fn store() -> MeshRegistry { MeshRegistry::new() }
RS
  printf 'pub mod mesh;\npub mod server;\n' > "$C/main.rs"
  printf 'pub fn serve() {}\n' > "$C/server.rs"
  printf 'kinds:\n  mesh-declaration:\n    schema: majordomus/mesh-declaration/v1\n' > "$F/share/kinds.yaml"
  printf '{ "type": "object" }\n' > "$F/share/schemas/majordomus/mesh-declaration/mesh-declaration.v1.schema.json"
  printf '# The mesh observes; it grants nothing.\n' \
    > "$F/.ai/repo/rules/project/mesh-is-observation-not-authority.v1.md"
  printf '%s' "$F"
}

# ---------------------------------------------------------------- the tree it accepts
F="$(fixture 0)"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep 'mesh-check: OK   sockets under src/mesh/, one registry, declaration complete'

# ---------------------------------------------------------------- 1. one transport
# The violation: a second place in the crate that opens a UDP socket. This is how a
# discovery mechanism grows outside the provider contract — not by decision, but because
# some module needed to find a peer and a socket was two lines away.
F="$(fixture 1)"
cat > "$F/apps/majordomus-cli/src/discovery.rs" <<'RS'
use std::net::UdpSocket;
pub fn probe() { let _ = UdpSocket::bind("0.0.0.0:0"); }
RS
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'apps/majordomus-cli/src/discovery.rs opens a UDP socket outside apps/majordomus-cli/src/mesh/'
expect_grep 'a discovery transport belongs behind the provider contract'

# restored: the same socket, moved inside the module that owns transports, is not a finding
mkdir -p "$F/apps/majordomus-cli/src/mesh"
mv "$F/apps/majordomus-cli/src/discovery.rs" "$F/apps/majordomus-cli/src/mesh/discovery.rs"
expect_exit 0 env MJ_ROOT="$F" "$GATE"

# the crate's own tests may bind: the subject under test is not a discovery mechanism, and
# a gate that refused this would make the boundary unprovable
F="$(fixture 2)"
cat > "$F/apps/majordomus-cli/tests/mesh.rs" <<'RS'
use std::net::UdpSocket;
#[test]
fn two_runtimes_meet() { let _ = UdpSocket::bind("127.0.0.1:0"); }
RS
expect_exit 0 env MJ_ROOT="$F" "$GATE"

# and a document that describes the transport describes it: prose opens nothing
F="$(fixture 3)"
printf '# The mesh\n\nEach runtime opens a UdpSocket and constructs a MeshRegistry.\n' > "$F/docs/MESH.md"
expect_exit 0 env MJ_ROOT="$F" "$GATE"

# ---------------------------------------------------------------- 2. one peer store
# The violation: a second construction site for the registry. Two stores is two answers to
# "who exists", and the second one is always the stale one.
F="$(fixture 4)"
cat > "$F/apps/majordomus-cli/src/server.rs" <<'RS'
use crate::mesh::registry::MeshRegistry;
pub fn peers() -> MeshRegistry { MeshRegistry::new() }
RS
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'apps/majordomus-cli/src/server.rs constructs MeshRegistry'
expect_grep 'the one peer store is built inside src/mesh/ and reached through the runtime'

# restored: reaching the store through the runtime rather than building one is the shape
# the rule asks for, and it passes
cat > "$F/apps/majordomus-cli/src/server.rs" <<'RS'
use crate::mesh::registry::MeshRegistry;
pub fn peers(runtime: &dyn Fn() -> MeshRegistry) -> MeshRegistry { runtime() }
RS
expect_exit 0 env MJ_ROOT="$F" "$GATE"

# ---------------------------------------------------------------- 3. the declaration
# A kind without its schema is an object nothing validates; a schema without its kind is a
# file nothing reaches; a rule missing is a boundary nobody wrote down. The gate names
# which of the three is absent, because "the declaration is incomplete" is not an
# instruction and `kind=1 schema=0 rule=1` is.
F="$(fixture 5)"
rm -f "$F/share/schemas/majordomus/mesh-declaration/mesh-declaration.v1.schema.json"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'the mesh declaration is incomplete: kind=1 schema=0 rule=1'
expect_grep 'the kind in share/kinds.yaml, its schema and the rule move together'

F="$(fixture 6)"
printf 'kinds:\n  session:\n    schema: majordomus/session/v1\n' > "$F/share/kinds.yaml"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'incomplete: kind=0 schema=1 rule=1'

F="$(fixture 7)"
rm -f "$F/.ai/repo/rules/project/mesh-is-observation-not-authority.v1.md"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'incomplete: kind=1 schema=1 rule=0'

# and all three back is the tree it accepted at the start
F="$(fixture 8)"
expect_exit 0 env MJ_ROOT="$F" "$GATE"

# ---------------------------------------------------------------- every finding, at once
# A gate that stops at the first finding makes a broken tree take three runs to fix. This
# one reports all three and exits once.
F="$(fixture 9)"
cat > "$F/apps/majordomus-cli/src/discovery.rs" <<'RS'
use std::net::UdpSocket;
pub fn probe() { let _ = UdpSocket::bind("0.0.0.0:0"); }
RS
cat > "$F/apps/majordomus-cli/src/server.rs" <<'RS'
use crate::mesh::registry::MeshRegistry;
pub fn peers() -> MeshRegistry { MeshRegistry::new() }
RS
rm -f "$F/.ai/repo/rules/project/mesh-is-observation-not-authority.v1.md"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'discovery.rs opens a UDP socket'
expect_grep 'server.rs constructs MeshRegistry'
expect_grep 'incomplete: kind=1 schema=1 rule=0'

echo "    the mesh boundary refuses a second transport, a second peer store and a half-declared kind"

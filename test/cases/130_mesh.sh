# majordomus-covers: doctor
# The mesh, through the command line and the declaration, without a network: the
# self-check proves the machine's prerequisites and never writes; the identity is
# absent until a mesh activates, and absent is an answer; the declaration parses with
# its safe defaults and a malformed one is one failed check naming the reason; and
# with no running server, `mesh status` says where the mesh lives instead of erroring.
#
# The crate's own suite (apps/majordomus-cli/src/mesh/*, tests/mesh.rs) covers the
# protocol's refusals, replay, trust, deduplication and the two-runtime rendezvous
# round-trip in depth. What is here is the operator's path across the two programs.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?

R="$T/repo"
fixture_repo "$R" >/dev/null
git -C "$R" init -q .
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
git -C "$R" add -A >/dev/null
git -C "$R" commit -qm fixture >/dev/null

# The node identity must never touch the person's real state directory: the case owns one.
STATE="$T/state"
mkdir -p "$STATE"
mj() { local cwd="$1"; shift; ( cd "$cwd" && MAJORDOMUS_SHARE="$R/share" XDG_STATE_HOME="$STATE" "$RB" "$@" ); }

# ---------------------------------------------------------------- 1. the self-check, no declaration
expect_exit 0 mj "$R" mesh doctor
expect_grep "declaration"
expect_grep "default posture"
expect_grep "protocol"
[ -e "$STATE/majordomus/node.json" ] && { echo "    the self-check created an identity; it must only read"; exit 1; }

# ---------------------------------------------------------------- 2. identity: absent is an answer
expect_exit 0 mj "$R" mesh identity
expect_grep "present    false"
expect_grep "$STATE"

# ---------------------------------------------------------------- 3. a declaration with safe defaults
mkdir -p "$R/.ai/repo/mesh"
cat > "$R/.ai/repo/mesh/majordomus.yaml" <<'YAML'
schema: mesh/v1
kind: mesh-declaration
id: majordomus
enabled: false
YAML
git -C "$R" add .ai/repo/mesh/majordomus.yaml
expect_exit 0 mj "$R" mesh doctor
expect_grep "enabled=false"
expect_grep "trust=deny_unknown"

# ---------------------------------------------------------------- 4. a malformed declaration never reaches the mesh
# The kind's JSON schema refuses the object at the index (code=unknown_key, the layer's
# own diagnostic names the file), so the mesh sees no declaration and stays off — the
# safe answer, diagnosed upstream rather than half-parsed downstream.
cat > "$R/.ai/repo/mesh/majordomus.yaml" <<'YAML'
schema: mesh/v1
kind: mesh-declaration
id: majordomus
surprise: true
YAML
git -C "$R" add .ai/repo/mesh/majordomus.yaml
expect_exit 0 mj "$R" mesh doctor
expect_grep "unknown_key"
expect_grep "default posture"

# ---------------------------------------------------------------- 5. no server is an answer, not an error
expect_exit 0 mj "$R" mesh status
expect_grep "inactive"
expect_grep "no running server"
expect_exit 0 mj "$R" mesh nodes
expect_grep "no running server"

# ---------------------------------------------------------------- 6. json and text are one answer
expect_exit 0 mj "$R" mesh identity --format json
expect_grep '"present": false'

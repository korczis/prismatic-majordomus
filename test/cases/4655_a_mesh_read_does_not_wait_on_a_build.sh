# claims: none
# A read of the mesh does not wait on a build (ADR 0234 decision 4, I2411).
#
# On 2026-11-22 a session in the owner's primary checkout could not run `majordomus mesh
# status` at all: the tree's executable was not built, the installed release was not of the
# tree's generation, and the resolver refused every command alike with exit 12 naming
# `just build` (ADR 0224). A read of the mesh asks the running runtime and the network what
# they hold, signs nothing and writes nothing, so the release answers it now; everything else
# is refused as before.
#
# A copy of lib/rust_bin.sh runs in a small repository whose crate is one file, with no
# executable, no sibling worktree, a registry projection that declares three mesh
# capabilities, and an installed release stand-in stamped with a commit this repository does
# not have:
#
#   1  `mesh status` and `mesh peers --full` (queries): answered by the release, said on stderr
#   2  `mesh build-cache env` (a query two words deep): answered by the release
#   3  `mesh claim x` (a command), `mesh nope` (not declared) and `context` (not the mesh):
#      refused with exit 12 naming `just build`
#   4  `mesh firewall apply`: refused, although `mesh firewall` is a query — the longest name
#      the registry declares decides
#   5  without the projection: refused, because nothing says what is a read
#
# and in none of them is anything built.
. "$ROOT/test/lib.sh"
unset MAJORDOMUS_BIN MAJORDOMUS_BIN_COMMIT CARGO_TARGET_DIR CARGO_BUILD_TARGET_DIR

fail() { printf '    %s\n' "$*"; exit 1; }
mkdir -p repo/lib repo/apps/majordomus-cli/src repo/docs/generated
cp "$ROOT/lib/rust_bin.sh" repo/lib/rust_bin.sh
printf '[package]\nname = "fixture"\n' > repo/apps/majordomus-cli/Cargo.toml
printf 'fn main() {}\n' > repo/apps/majordomus-cli/src/main.rs
printf 'target/\n' > repo/.gitignore
# the shape the generator writes: one capability object per entry, its keys six spaces in
cat > repo/docs/generated/registry.json <<'JSON'
{
  "capabilities": [
    {
      "id": "mesh.status",
      "module": "mesh",
      "kind": "query"
    },
    {
      "id": "mesh.peers",
      "module": "mesh",
      "kind": "query"
    },
    {
      "id": "mesh.claim",
      "module": "mesh",
      "kind": "command"
    },
    {
      "id": "mesh.firewall",
      "module": "mesh",
      "kind": "query"
    },
    {
      "id": "mesh.firewall.apply",
      "module": "mesh",
      "kind": "command"
    },
    {
      "id": "mesh.build_cache.env",
      "module": "mesh",
      "kind": "query"
    }
  ]
}
JSON
gitc() { git -C "$1" -c user.email=case@example.invalid -c user.name=case "${@:2}"; }
git -C repo init -q && git -C repo add -A && gitc repo commit -qm crate || fail "could not build the fixture repository"

# the installed release: an installer's launcher execing a version whose RELEASE.json names a
# commit of another repository, so it is never of this tree's generation
REL="$T/release/versions/9.9.9"
mkdir -p "$REL/bin" "$T/release/launch"
printf '#!/usr/bin/env bash\necho release "$@"\n' > "$REL/bin/majordomus"; chmod +x "$REL/bin/majordomus"
printf '{"version": "9.9.9", "commit": "%s"}\n' "$(printf 'ab%.0s' $(seq 20))" > "$REL/RELEASE.json"
printf '#!/bin/sh\nexec "%s" "$@"\n' "$REL/bin/majordomus" > "$T/release/launch/majordomus"
chmod +x "$T/release/launch/majordomus"
export MAJORDOMUS_INSTALLED_BIN="$T/release/launch/majordomus"

resolve() {
  ( cd "$T/repo" && . lib/rust_bin.sh && mj_rust_resolve "$T/repo" 'x: ' "$@" ) > "$T/out" 2> "$T/err"
}
answered() {   # words...
  resolve "$@" || fail "\`$*\` was not answered: $(cat "$T/err")"
  [ "$(cat "$T/out")" = "$MAJORDOMUS_INSTALLED_BIN" ] || fail "\`$*\` resolved to $(cat "$T/out"), not the release"
  grep -q 'this is a read of the mesh' "$T/err" || fail "\`$*\`: the fallback was not said: $(cat "$T/err")"
  grep -q 'not of this tree.s generation' "$T/err" || fail "\`$*\`: the generation was not named: $(cat "$T/err")"
}
refused() {   # words...
  rc=0; resolve "$@" || rc=$?
  [ "$rc" = 12 ] || fail "\`$*\` exited $rc, not 12: $(cat "$T/out") $(cat "$T/err")"
  grep -q 'run: just build' "$T/err" || fail "\`$*\`: the refusal does not name the build: $(cat "$T/err")"
  ! grep -q 'this is a read of the mesh' "$T/err" || fail "\`$*\` was called a read of the mesh"
}

# ---------------------------------------------------------------- 1, 2. reads
answered mesh status
answered mesh peers --full
answered mesh build-cache env

# ---------------------------------------------------------------- 3. not reads
refused mesh claim x
refused mesh nope
refused context

# ---------------------------------------------------------------- 4. the longest name decides
answered mesh firewall
refused mesh firewall apply

# ---------------------------------------------------------------- 5. no projection
rm -f "$T/repo/docs/generated/registry.json"
refused mesh status

[ ! -d "$T/repo/apps/majordomus-cli/target" ] || fail "a target directory appeared in the fixture"
exit 0

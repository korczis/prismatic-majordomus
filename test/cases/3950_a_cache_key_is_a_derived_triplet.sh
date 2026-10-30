# majordomus-covers: capability:mesh.build_cache.key
# A cache key is a versioned triplet of platform, toolchain and inputs, derived from the host and
# the repository (mesh.build_cache.key, I3950, ADR 0157 as amended on 2026-10-22).
#
# A disposable repository tracking a Cargo.lock, a package-lock.json and a mix.lock:
#
#   every lockfile                 one cacheable thing each, every key `v1/<kind>/...`
#   cargo                          the platform is rustc's own host triple
#   a changed Cargo.lock           the cargo key moves, the npm key does not
#   a changed member Cargo.toml    the cargo key moves
#   RUSTFLAGS set                  the cargo key moves
#   an unrelated file              no key moves
#   no node on PATH                npm has no key and says why
#   over MCP                       the same keys as the command line
#   the command line               one line per thing, `--kind` narrows it
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is not installed"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-3950.XXXXXX")"
trap 'rm -rf "$S"' EXIT

"$MJ" init >/dev/null
mkdir -p crates/a site app
printf '[workspace]\nmembers = ["crates/a"]\n' > Cargo.toml
printf '[package]\nname = "a"\nversion = "0.1.0"\n' > crates/a/Cargo.toml
printf 'version = 4\n' > Cargo.lock
printf '{"lockfileVersion":3}\n' > site/package-lock.json
printf '%%{}\n' > app/mix.lock
git add -A >/dev/null && git commit -qm install

keys() { # [kind] -> the report as JSON
  if [ -n "${1:-}" ]; then input="{\"kind\":\"$1\"}"; else input='{}'; fi
  "$RB" run mesh.build_cache.key --input "$input" --quiet --format json 2>/dev/null | jq -c '.output'
}
key_of() { printf '%s' "$1" | jq -r --arg k "$2" '.kinds[] | select(.kind == $k) | .key // "none"'; }

# ---------------------------------------------------------------- 1. every lockfile
r="$(keys)"
[ "$(printf '%s' "$r" | jq '.kinds | length')" = 3 ] || { echo "    not one thing per lockfile: $r"; exit 1; }
printf '%s' "$r" | jq -e '[.kinds[] | .kind] == ["cargo-build-dir","mix-deps","npm-modules"]' >/dev/null \
  || { echo "    the kinds are not cargo, mix, npm in order: $r"; exit 1; }
printf '%s' "$r" | jq -e '[.kinds[] | select(.key) | .key | startswith("v1/")] | all' >/dev/null \
  || { echo "    a key carries no schema version: $r"; exit 1; }
echo "    every tracked lockfile is a cacheable thing with a versioned key"

# ---------------------------------------------------------------- 2. cargo's platform is rustc's
cargo_key="$(key_of "$r" cargo-build-dir)"
if command -v rustc >/dev/null 2>&1; then
  host="$(rustc -vV | sed -n 's/^host: //p')"
  case "$cargo_key" in
    "v1/cargo-build-dir/$host/rustc-"*) ;;
    *) echo "    the cargo key is not on rustc's host $host: $cargo_key"; exit 1 ;;
  esac
  echo "    the cargo key's platform is rustc's own host triple ($host)"
fi
npm_key="$(key_of "$r" npm-modules)"

# ---------------------------------------------------------------- 3. what moves the key
printf 'version = 4\n# changed\n' > Cargo.lock
r2="$(keys)"
[ "$(key_of "$r2" cargo-build-dir)" != "$cargo_key" ] || { echo "    a changed Cargo.lock left the cargo key"; exit 1; }
[ "$(key_of "$r2" npm-modules)" = "$npm_key" ] || { echo "    a changed Cargo.lock moved the npm key"; exit 1; }
echo "    a changed Cargo.lock moves the cargo key and no other"

before="$(key_of "$r2" cargo-build-dir)"
printf '[package]\nname = "a"\nversion = "0.2.0"\n' > crates/a/Cargo.toml
[ "$(key_of "$(keys cargo-build-dir)" cargo-build-dir)" != "$before" ] \
  || { echo "    a changed member Cargo.toml left the cargo key"; exit 1; }
echo "    a changed member Cargo.toml moves the cargo key"

before="$(key_of "$(keys cargo-build-dir)" cargo-build-dir)"
flagged="$(RUSTFLAGS='-C target-cpu=native' "$RB" run mesh.build_cache.key --input '{"kind":"cargo-build-dir"}' --quiet --format json 2>/dev/null | jq -c '.output')"
[ "$(key_of "$flagged" cargo-build-dir)" != "$before" ] || { echo "    RUSTFLAGS left the cargo key"; exit 1; }
echo "    RUSTFLAGS moves the cargo key"

r3="$(keys)"
echo unrelated > NOTES.txt && git add NOTES.txt
[ "$(keys)" = "$r3" ] || { echo "    an unrelated file moved a key"; exit 1; }
echo "    an unrelated file moves no key"

# ---------------------------------------------------------------- 4. an absent toolchain is said
mkdir -p "$S/bin"
ln -s "$(command -v git)" "$S/bin/git"
bare="$(PATH="$S/bin" "$RB" run mesh.build_cache.key --input '{"kind":"npm-modules"}' --quiet --format json 2>/dev/null | "$(command -v jq)" -c '.output')"
printf '%s' "$bare" | jq -e '.kinds[0].key == null and (.kinds[0].unavailable | test("node does not answer"))' >/dev/null \
  || { echo "    npm without node has a key or no reason: $bare"; exit 1; }
echo "    with no node on PATH, npm has no key and says why"

# ---------------------------------------------------------------- 5. MCP agrees
mcp="$( { printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case3950","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_mesh_build_cache_key","arguments":{}}}\n'
} | "$RB" mcp --standalone --repo "$PWD" 2>"$S/mcp.err" | sed -n 2p | jq -c '.result.structuredContent // empty')"
[ "$(printf '%s' "$mcp" | jq -c '[.kinds[].key]')" = "$(keys | jq -c '[.kinds[].key]')" ] \
  || { echo "    MCP and the command line disagree: $mcp"; cat "$S/mcp.err"; exit 1; }
echo "    a tools/call over MCP answers the same keys"

# ---------------------------------------------------------------- 6. the command line
out="$("$RB" mesh build-cache key --kind npm-modules 2>&1)"
printf '%s\n' "$out" | grep -q '^platform ' || { echo "    no platform line: $out"; exit 1; }
printf '%s\n' "$out" | grep -q '^npm-modules ' || { echo "    no npm line: $out"; exit 1; }
printf '%s\n' "$out" | grep -q '^cargo-build-dir' && { echo "    --kind did not narrow: $out"; exit 1; }
echo "    mesh build-cache key prints one line per thing, narrowed by --kind"

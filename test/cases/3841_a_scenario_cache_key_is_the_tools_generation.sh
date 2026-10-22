# majordomus-covers: usecase
# The scenario cache knows an executable by its generation, not by its bytes (I2868). The key
# of I0832 hashed the executable MAJORDOMUS_BIN names, and build.rs compiles the commit and the
# dirty flag into it, so two builds of identical sources never shared a key and every CI build
# was a new tool whose store held nothing. The key now names the generation `distribution
# build` reports, and only when the executable reports the same digest for the tree's crate
# (`tree_generation`): the two-sided comparison `majordomus generate` makes, by its one
# definition, with no copy of the digest in the shell.
#
# Parts a to d drive the cache's own functions against a fixture clone shaped like the tool's
# source tree, with the scenario runner stubbed and stand-in executables that answer
# `distribution build` the way the real one does: each reports the generation it was "built
# from" (baked into it) and the digest of the fixture crate's source it is asked about.
#
#   a  two executables that differ in bytes and report the same generation, equal to the
#      tree's, give one key: the entries written through the first are read through the second
#   b  a one-line committed change to a crate source, with an executable built from it, is
#      another key: both scenarios run again
#   c  an executable whose generation is not the tree's gives no key: the scenarios run and
#      nothing is cached
#   d  an executable that reports no generation (one built before it did) gives no key
#   e  the real executable reports both digests: a 64-digit generation, a tree generation for
#      a crate that a one-line change moves, and none for a repository without a crate
. "$ROOT/test/lib.sh"

R="$T/repo"
mkdir -p "$R/apps/majordomus-cli/src" "$R/lib" "$R/bin"
printf '[package]\nname = "fixture"\n' > "$R/apps/majordomus-cli/Cargo.toml"
printf 'pub fn f() {}\n' > "$R/apps/majordomus-cli/src/lib.rs"
printf 'tool\n' > "$R/lib/tool.sh"; printf 'launcher\n' > "$R/bin/majordomus"
commit() { git -C "$R" add -A && git -C "$R" -c user.email=case@example.invalid -c user.name=case commit -qm "$1"; }
{ git -C "$R" init -q && commit fixture; } || { echo "    could not build the fixture clone"; exit 1; }
STORE="$T/store"
printf 'use case alpha\n' > "$T/alpha.md"; printf 'use case beta\n' > "$T/beta.md"
LOG="$T/executed"; : > "$LOG"

# The fixture's stand-in for crate_generation: the digest of its one source file.
digest() { mj_case_sha256 "$R/apps/majordomus-cli/src/lib.rs"; }
mj_case_sha256() { if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1"; else sha256sum "$1"; fi | cut -d ' ' -f 1; }

# standin <path> <generation|none> <note>: an executable "built from" <generation>; <note> only
# makes its bytes differ, as the commit build.rs compiles in makes two real builds differ
standin() {
  { printf '#!/usr/bin/env bash\n# %s\n' "$3"
    printf 'BUILT_FROM=%s\n' "$2"
    cat <<'SH'
[ "$1 $2" = "distribution build" ] || exit 2
repo=""; while [ $# -gt 0 ]; do [ "$1" = --repo ] && repo="$2"; shift; done
src="$repo/apps/majordomus-cli/src/lib.rs"
if [ "$BUILT_FROM" = none ]; then printf '{\n  "version": "0.0.0",\n  "commit": "stand-in"\n}\n'; exit 0; fi
tree="null"
if [ -f "$src" ]; then
  tree="\"$(if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$src"; else sha256sum "$src"; fi | cut -d ' ' -f 1)\""
fi
printf '{\n  "version": "0.0.0",\n  "generation": "%s",\n  "tree_generation": %s\n}\n' "$BUILT_FROM" "$tree"
SH
  } > "$1"
  chmod +x "$1"
}

drive() {   # drive <executable> <label>: both scenarios through the cache; prints "key=<key or none>"
  ( set -eu
    bin="$1" label="$2"   # sourcing may reset the positional parameters
    unset MJ_UC_CACHE
    MJ_LIB_DIR="$ROOT/lib"
    # shellcheck source=../../lib/common.sh
    . "$ROOT/lib/common.sh"
    # shellcheck source=../../lib/usecase.sh
    . "$ROOT/lib/usecase.sh"
    # shellcheck disable=SC2034  # these globals are read by the sourced lib/usecase.sh
    { MJ_ROOT="$R"; MJ_LIB_DIR="$R/bin/../lib"
      MJ_UC_DIR="$R/.ai/repo/use-cases"; MJ_AI_LOCAL_DIR="$T/local-$label"
      MJ_UC_CACHE_DIR="$STORE"; MJ_UC_FILE_1="$T/alpha.md"; MJ_UC_FILE_2="$T/beta.md"; }
    MAJORDOMUS_BIN="$bin"; export MAJORDOMUS_BIN
    mj_uc_v() { if [ "$1" = 1 ]; then printf alpha; else printf beta; fi; }
    mj_uc_run_one() {
      local id; id="$(mj_uc_v "$1" id)"
      printf '%s\n' "$id" >> "$LOG"
      printf '{"evidence":"%s run %s %s"}\n' "$id" "$$" "$(od -An -N8 -tx1 /dev/urandom | tr -d ' \n')" > "$2"
    }
    MJ_UC_TOOL_KEY="$(mj_uc_tool_key)"
    mkdir -p "$T/out/$label"
    mj_uc_run_cached 1 "$T/out/$label/alpha.json" 0
    mj_uc_run_cached 2 "$T/out/$label/beta.json" 0
    printf 'key=%s\n' "${MJ_UC_TOOL_KEY:-none}" )
}
runs() { wc -l < "$LOG" | tr -d ' '; }
entries() { local n=0 f; for f in "$STORE"/*.json; do [ -f "$f" ] && n=$((n + 1)); done; echo "$n"; }
same() { cmp -s "$T/out/$1/alpha.json" "$T/out/$2/alpha.json" && cmp -s "$T/out/$1/beta.json" "$T/out/$2/beta.json"; }

# ---------------------------------------------------------------- a. one generation, one key
G1="$(digest)"
standin "$T/one" "$G1" "built at commit one"; standin "$T/two" "$G1" "built at commit two, dirty"
cmp -s "$T/one" "$T/two" && { echo "    the two stand-ins are byte-identical, so they prove nothing"; exit 1; }
got1="$(drive "$T/one" one)" || { echo "    the cache functions failed: $got1"; exit 1; }
[ "$got1" != key=none ] && [ "$(runs)" = 2 ] && [ "$(entries)" = 2 ] \
  || { echo "    an executable of the tree's generation was not keyed: $got1, $(runs) run(s), $(entries) entries"; exit 1; }
got2="$(drive "$T/two" two)" || { echo "    the cache functions failed: $got2"; exit 1; }
[ "$got2" = "$got1" ] \
  || { echo "    two builds of one generation gave two keys: $got1 and $got2"; exit 1; }
[ "$(runs)" = 2 ] && same one two \
  || { echo "    the second build of the same generation ran the scenarios again: $(runs) run(s) in all"; exit 1; }

# ---------------------------------------------------------------- b. a changed source misses
printf '// one line\npub fn f() {}\n' > "$R/apps/majordomus-cli/src/lib.rs"
commit "a changed crate source" || { echo "    could not commit"; exit 1; }
G2="$(digest)"
[ "$G2" != "$G1" ] || { echo "    the fixture's change did not move its generation"; exit 1; }
standin "$T/three" "$G2" "built from the changed source"
got3="$(drive "$T/three" three)" || { echo "    the cache functions failed: $got3"; exit 1; }
[ "$got3" != key=none ] && [ "$got3" != "$got1" ] \
  || { echo "    a changed crate source did not change the key: $got1, then $got3"; exit 1; }
[ "$(runs)" = 4 ] && ! same one three \
  || { echo "    a changed crate source was served the old source's evidence: $(runs) run(s) in all"; exit 1; }

# ---------------------------------------------------------------- c. another generation: no key
before="$(entries)"
got="$(drive "$T/one" stale)" || { echo "    the cache functions failed: $got"; exit 1; }
[ "$got" = key=none ] \
  || { echo "    an executable built from other sources than the tree's was keyed: $got"; exit 1; }
[ "$(runs)" = 6 ] && [ "$(entries)" = "$before" ] \
  || { echo "    with no key the scenarios did not both run uncached: $(runs) run(s), $(entries) entries (was $before)"; exit 1; }

# ---------------------------------------------------------------- d. no generation: no key
standin "$T/old" none "built before it reported a generation"
got="$(drive "$T/old" old)" || { echo "    the cache functions failed: $got"; exit 1; }
[ "$got" = key=none ] && [ "$(runs)" = 8 ] && [ "$(entries)" = "$before" ] \
  || { echo "    an executable that reports no generation was keyed: $got, $(runs) run(s), $(entries) entries"; exit 1; }

# ---------------------------------------------------------------- e. the real executable answers
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# a quote before the name, so that tree_generation is never read as generation
field() { sed -nE "s/.*\"$1\": *\"?([0-9a-f]*|null)\"?,?\$/\\1/p" "$2"; }
digest64() { case "$1" in ''|*[!0-9a-f]*) return 1 ;; esac; [ "${#1}" = 64 ]; }
ask() { "$RB" distribution build --repo "$1" --format json > "$2" 2> "$2.err" \
  || { echo "    distribution build in $1 failed:"; sed 's/^/      /' "$2.err"; exit 1; }; }
# the executable reads a repository only where the layer is: as case 33's fixtures do
[ -f "$R/.ai/manifest.yaml" ] || (cd "$R" && "$MJ" init >/dev/null 2>&1) \
  || { echo "    could not initialise the fixture for the executable"; exit 1; }
ask "$R" "$T/real1.json"
gen="$(field generation "$T/real1.json")"; tree1="$(field tree_generation "$T/real1.json")"
digest64 "$gen" || { echo "    the executable reports no 64-digit generation: '$gen'"; exit 1; }
digest64 "$tree1" || { echo "    the executable reports no tree generation for a crate: '$tree1'"; exit 1; }
printf '// another line\n' >> "$R/apps/majordomus-cli/src/lib.rs"
ask "$R" "$T/real2.json"
tree2="$(field tree_generation "$T/real2.json")"
[ -n "$tree2" ] && [ "$tree2" != "$tree1" ] \
  || { echo "    a one-line change to the crate did not move the tree generation: $tree1, then $tree2"; exit 1; }
mkdir -p "$T/no-crate" && git -C "$T/no-crate" init -q && (cd "$T/no-crate" && "$MJ" init >/dev/null 2>&1) \
  || { echo "    could not initialise the crate-less fixture"; exit 1; }
ask "$T/no-crate" "$T/real3.json"
[ "$(field tree_generation "$T/real3.json")" = null ] \
  || { echo "    a repository without a crate was given a tree generation"; sed 's/^/      /' "$T/real3.json"; exit 1; }

echo "    the cache keys an executable by its generation: two builds share it, a changed source misses, another or no generation gives no key"

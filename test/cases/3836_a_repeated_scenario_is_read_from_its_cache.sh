# majordomus-covers: usecase
# A scenario whose tool and use case are unchanged is read from its cache the second time
# (I0832). The cache shipped first never engaged: mj_uc_tool_key compared "$MJ_LIB_DIR" with
# "$MJ_ROOT/lib" as strings, bin/majordomus spells the first "<bin>/../lib", so no key was ever
# computed and every scenario ran again, while `generate-site-data --check` still passed with
# zero cache entries. A proof that passes on zero entries cannot tell a working cache from an
# absent one, so this case fails on zero reuse.
#
# The cache's own functions run against a fixture shaped like the tool's source tree (a crate
# manifest, lib/, bin/, committed), with MJ_LIB_DIR spelled the way bin/majordomus spells it
# and the scenario runner stubbed, so no scenario executes:
#
#   1  the key is computed; two runs of one use case execute it once, leave one entry, and
#      the second evidence is byte-identical to the first
#   2  the tool's tree carrying an uncommitted change computes no key, and both runs execute
#   3  the string comparison the first version made computes no key: the mutant is caught
. "$ROOT/test/lib.sh"

R="$T/repo"
mkdir -p "$R/apps/majordomus-cli" "$R/lib" "$R/bin"
printf '[package]\nname = "fixture"\n' > "$R/apps/majordomus-cli/Cargo.toml"
printf 'tool\n' > "$R/lib/tool.sh"
git -C "$R" init -q && git -C "$R" add -A \
  && git -C "$R" -c user.email=case@example.invalid -c user.name=case commit -qm fixture \
  || { echo "    could not build the fixture tree"; exit 1; }
printf 'executable bytes\n' > "$T/bin.fixture"
printf 'use case\n' > "$T/demo.md"

drive() {   # drive <usecase.sh to load> <cache dir>: prints "key=<0|1> calls=<n> entries=<n> same=<yes|no>"
  ( set -eu
    lib="$1" dir="$2"   # sourcing may reset the positional parameters
    MJ_LIB_DIR="$ROOT/lib"
    # shellcheck source=../../lib/common.sh
    . "$ROOT/lib/common.sh"
    # shellcheck disable=SC1090
    . "$lib"
    # shellcheck disable=SC2034  # these globals are read by the sourced lib/usecase.sh
    { MJ_ROOT="$R"; MJ_LIB_DIR="$R/bin/../lib"
      MJ_UC_DIR="$R/.ai/repo/use-cases"; MJ_AI_LOCAL_DIR="$dir"; }
    MAJORDOMUS_BIN="$T/bin.fixture"; export MAJORDOMUS_BIN
    calls=0
    mj_uc_run_one() { calls=$((calls + 1)); printf '{"evidence":"run %s"}\n' "$calls" > "$2"; }
    mj_uc_v() { printf 'demo'; }
    # shellcheck disable=SC2034  # read by mj_uc_tool_key in the sourced lib/usecase.sh
    MJ_UC_FILE_1="$T/demo.md"
    MJ_UC_TOOL_KEY="$(mj_uc_tool_key)"
    mkdir -p "$dir"
    mj_uc_run_cached 1 "$dir/one.json" 0
    mj_uc_run_cached 1 "$dir/two.json" 0
    n=0; for f in "$dir"/cache/use-cases/*.json; do [ -f "$f" ] && n=$((n + 1)); done
    printf 'key=%s calls=%s entries=%s same=%s\n' "$([ -n "$MJ_UC_TOOL_KEY" ] && echo 1 || echo 0)" \
      "$calls" "$n" "$(cmp -s "$dir/one.json" "$dir/two.json" && echo yes || echo no)" )
}

# ---------------------------------------------------------------- 1. reused
got="$(drive "$ROOT/lib/usecase.sh" "$T/local1")" || { echo "    the cache functions failed: $got"; exit 1; }
[ "$got" = "key=1 calls=1 entries=1 same=yes" ] \
  || { echo "    a repeated scenario was not read from its cache: $got"; exit 1; }

# ---------------------------------------------------------------- 2. a dirty tool is not keyed
printf 'changed\n' >> "$R/lib/tool.sh"
got="$(drive "$ROOT/lib/usecase.sh" "$T/local2")" || { echo "    the cache functions failed: $got"; exit 1; }
[ "$got" = "key=0 calls=2 entries=0 same=no" ] \
  || { echo "    a tool with an uncommitted change was keyed: $got"; exit 1; }
git -C "$R" checkout -q -- lib/tool.sh

# ---------------------------------------------------------------- 3. the string comparison
mutant="$T/usecase.mutant.sh"
sed 's|"$(cd "$MJ_LIB_DIR" 2>/dev/null \&\& pwd -P)" = "$(cd "$MJ_ROOT/lib" \&\& pwd -P)"|"$MJ_LIB_DIR" = "$MJ_ROOT/lib"|' \
  "$ROOT/lib/usecase.sh" > "$mutant"
cmp -s "$mutant" "$ROOT/lib/usecase.sh" \
  && { echo "    lib/usecase.sh no longer resolves the lib dirs where this case can mutate it"; exit 1; }
got="$(drive "$mutant" "$T/local3")" || { echo "    the mutant failed to load: $got"; exit 1; }
[ "$got" = "key=0 calls=2 entries=0 same=no" ] \
  || { echo "    the string comparison was not caught: $got"; exit 1; }

echo "    a repeated scenario is read from its cache; a dirty tool and a string-compared lib dir are not"

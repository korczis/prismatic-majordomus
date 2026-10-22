# majordomus-covers: usecase
# A passing scenario's evidence is reused by every checkout of the clone whose tool is the same
# (I2867). The store of I0832 lived under each checkout's .ai/local, so every train, repair lane
# and fresh worktree paid the whole fill again (797 of 952 s of a generation, 2026-10-21) for a
# tool an earlier checkout had already run; and a write removed every other key's entry of its
# use case, so two checkouts sharing it would have evicted each other on every run.
#
# The cache's own functions run against a fixture clone shaped like the tool's source tree, and
# a linked worktree of it, with the scenario runner stubbed: each execution appends to a log
# and writes evidence no other execution writes, so a reused entry is told from a rerun by the
# log and by the bytes:
#
#   a  the worktree at the primary's tool reuses every entry the primary wrote, under git's
#      common directory, and its evidence is byte-identical
#   b  a committed change to the tool in the worktree misses, and both scenarios run again
#   c  the two tools, read alternately, find their entries: neither write evicted the other
#   d  the bound prunes: one entry per use case keeps the newest tool's only; an entry unused
#      for longer than the age bound goes at the next write
#   e  no store holds a temporary file afterwards: an entry is renamed into place whole
. "$ROOT/test/lib.sh"

R="$T/repo"; W2="$T/wt2"
mkdir -p "$R/apps/majordomus-cli" "$R/lib" "$R/bin"
printf '[package]\nname = "fixture"\n' > "$R/apps/majordomus-cli/Cargo.toml"
printf 'tool\n' > "$R/lib/tool.sh"; printf 'launcher\n' > "$R/bin/majordomus"   # bin/ tracked, so a worktree has it
commit() { git -C "$1" add -A && git -C "$1" -c user.email=case@example.invalid -c user.name=case commit -qm "$2"; }
{ git -C "$R" init -q && commit "$R" fixture && git -C "$R" worktree add -q --detach "$W2" HEAD; } \
  || { echo "    could not build the fixture clone and its worktree"; exit 1; }
STORE="$(cd "$R/.git" && pwd -P)/majordomus/use-case-cache"
# a stand-in executable of the fixture tree's generation: the key names what `distribution
# build` reports (I2868, case 3841), not the executable's bytes
cat > "$T/bin.fixture" <<'SH'
#!/usr/bin/env bash
[ "$1 $2" = "distribution build" ] || exit 2
g=0000000000000000000000000000000000000000000000000000000000000001
printf '{\n  "generation": "%s",\n  "tree_generation": "%s"\n}\n' "$g" "$g"
SH
chmod +x "$T/bin.fixture"
printf 'use case alpha\n' > "$T/alpha.md"; printf 'use case beta\n' > "$T/beta.md"
LOG="$T/executed"; : > "$LOG"

drive() {   # drive <checkout> <label>: both scenarios through the cache; prints "key=<0|1>"
  ( set -eu
    root="$1" label="$2"   # sourcing may reset the positional parameters
    unset MJ_UC_CACHE_DIR MJ_UC_CACHE
    MJ_LIB_DIR="$ROOT/lib"
    # shellcheck source=../../lib/common.sh
    . "$ROOT/lib/common.sh"
    # shellcheck source=../../lib/usecase.sh
    . "$ROOT/lib/usecase.sh"
    # shellcheck disable=SC2034  # these globals are read by the sourced lib/usecase.sh
    { MJ_ROOT="$root"; MJ_LIB_DIR="$root/bin/../lib"
      MJ_UC_DIR="$root/.ai/repo/use-cases"; MJ_AI_LOCAL_DIR="$T/local-$label"
      MJ_UC_FILE_1="$T/alpha.md"; MJ_UC_FILE_2="$T/beta.md"; }
    MAJORDOMUS_BIN="$T/bin.fixture"; export MAJORDOMUS_BIN
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
    printf 'key=%s\n' "$([ -n "$MJ_UC_TOOL_KEY" ] && echo 1 || echo 0)" )
}
runs() { wc -l < "$LOG" | tr -d ' '; }
entries() { local n=0 f; for f in "$STORE"/"$1".*.json; do [ -f "$f" ] && n=$((n + 1)); done; echo "$n"; }
same() { cmp -s "$T/out/$1/alpha.json" "$T/out/$2/alpha.json" && cmp -s "$T/out/$1/beta.json" "$T/out/$2/beta.json"; }

# ---------------------------------------------------------------- a. shared by the worktree
got="$(drive "$R" primary)" || { echo "    the cache functions failed: $got"; exit 1; }
[ "$got" = key=1 ] && [ "$(runs)" = 2 ] && [ "$(entries alpha)" = 1 ] && [ "$(entries beta)" = 1 ] \
  || { echo "    the primary did not fill the clone's store at $STORE: $got, $(runs) run(s)"; ls -la "$STORE" 2>&1 | sed 's/^/      /'; exit 1; }
first_entries="$(cd "$STORE" && ls)"
got="$(drive "$W2" worktree)" || { echo "    the cache functions failed: $got"; exit 1; }
[ "$got" = key=1 ] && [ "$(runs)" = 2 ] \
  || { echo "    the worktree at the same tool ran its scenarios again: $got, $(runs) run(s) in all"; exit 1; }
same primary worktree \
  || { echo "    the worktree's reused evidence is not the primary's, byte for byte"; exit 1; }
[ ! -d "$T/local-primary/cache" ] && [ ! -d "$T/local-worktree/cache" ] \
  || { echo "    an entry was written to a per-checkout store"; exit 1; }

# ---------------------------------------------------------------- b. another tool misses
printf 'changed\n' >> "$W2/lib/tool.sh"; commit "$W2" "another tool" || { echo "    could not commit"; exit 1; }
got="$(drive "$W2" changed)" || { echo "    the cache functions failed: $got"; exit 1; }
[ "$got" = key=1 ] && [ "$(runs)" = 4 ] \
  || { echo "    a committed change to the tool was served the old tool's evidence: $got, $(runs) run(s)"; exit 1; }
same worktree changed && { echo "    the changed tool's evidence is the old tool's"; exit 1; }

# ---------------------------------------------------------------- c. alternate writers keep both
[ "$(entries alpha)" = 2 ] && [ "$(entries beta)" = 2 ] \
  || { echo "    one tool's write evicted the other's entries: alpha $(entries alpha), beta $(entries beta)"; exit 1; }
for f in $first_entries; do
  [ -f "$STORE/$f" ] || { echo "    the primary's entry $f was evicted by the worktree's write"; exit 1; }
done
got="$(drive "$R" primary-again)" && got="$got $(drive "$W2" changed-again)" \
  || { echo "    the cache functions failed: $got"; exit 1; }
[ "$got" = "key=1 key=1" ] && [ "$(runs)" = 4 ] \
  && same primary primary-again && same changed changed-again \
  || { echo "    the two tools, read alternately, did not both find their entries: $got, $(runs) run(s)"; exit 1; }

# ---------------------------------------------------------------- d. the bound
printf 'third\n' >> "$W2/lib/tool.sh"; commit "$W2" "a third tool" || { echo "    could not commit"; exit 1; }
got="$(MJ_UC_CACHE_KEEP=1 drive "$W2" third)" || { echo "    the cache functions failed: $got"; exit 1; }
[ "$got" = key=1 ] && [ "$(runs)" = 6 ] && [ "$(entries alpha)" = 1 ] && [ "$(entries beta)" = 1 ] \
  || { echo "    a bound of one entry per use case did not prune the older tools: alpha $(entries alpha), beta $(entries beta)"; exit 1; }
got="$(drive "$R" primary-pruned)" || { echo "    the cache functions failed: $got"; exit 1; }
[ "$(runs)" = 8 ] || { echo "    the primary's pruned entries were still served: $(runs) run(s)"; exit 1; }
# the primary's entries back, then left unused past the age bound: the next write sweeps them
old="$(cd "$STORE" && ls -t | sed -n '1,2p')"
for f in $old; do touch -t 202001010000 "$STORE/$f"; done
printf 'fourth\n' >> "$W2/lib/tool.sh"; commit "$W2" "a fourth tool" || { echo "    could not commit"; exit 1; }
got="$(drive "$W2" fourth)" || { echo "    the cache functions failed: $got"; exit 1; }
for f in $old; do
  [ ! -e "$STORE/$f" ] || { echo "    an entry unused for longer than the age bound survived a write: $f"; exit 1; }
done
[ "$(entries alpha)" = 2 ] && [ "$(entries beta)" = 2 ] \
  || { echo "    the age sweep removed more than the old entries: alpha $(entries alpha), beta $(entries beta)"; exit 1; }

# ---------------------------------------------------------------- e. whole entries only
stray="$(find "$STORE" -type f ! -name '*.json')"
[ -z "$stray" ] || { echo "    the store holds what is not an entry: $stray"; exit 1; }

echo "    one store per clone: a worktree reuses the primary's entries, another tool misses, neither evicts the other, the bound prunes"

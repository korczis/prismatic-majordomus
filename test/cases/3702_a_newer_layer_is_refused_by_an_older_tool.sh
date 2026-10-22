# majordomus-covers: none
# A layer whose manifest names a newer tool is refused by the older tool with one message
# naming both versions (I1980). Read further, a newer layer failed later in ways nobody
# could trace back to the version: OSCILLA's migrated layer was rejected by 0.10, so the
# launcher had to flip before anything else worked. This case holds:
#
#   1  the shell tool refuses it, exit 10, with one sentence naming the layer's version, its
#      own version and the install command
#   2  the Rust executable refuses the same layer the same way
#   3  `migrate --layer` cannot move a newer layer backwards: it is refused before it writes
#   4  a patch-level difference is not a newer layer: the same major.minor is read
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_BIN="$RB"

full="$("$MJ" version | sed -n 's/^majordomus \([^ ]*\).*/\1/p')"
mine="$(printf '%s' "$full" | cut -d. -f1-2)"
[ -n "$mine" ] || { echo "    'majordomus version' named no version"; exit 1; }

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm "a fresh layer"
sed 's/^tool:.*/tool: "99.0"/' .ai/manifest.yaml > manifest.tmp && cat manifest.tmp > .ai/manifest.yaml && rm -f manifest.tmp

# ---------------------------------------------------------------- 1. the shell tool
expect_exit 10 "$MJ" doctor
expect_grep "written for majordomus 99\.0, newer than this one \($full\); install 99\.0 or newer"
expect_grep 'install\.sh'

# ---------------------------------------------------------------- 2. the Rust executable
expect_exit 10 "$RB" knowledge candidates
expect_grep "written for majordomus 99\.0, newer than this one \($full\)"

# ---------------------------------------------------------------- 3. never moved backwards
expect_exit 10 "$MJ" migrate --layer
grep -qx 'tool: "99.0"' .ai/manifest.yaml || { echo "    migrate --layer rewrote a newer layer's tool:"; head -5 .ai/manifest.yaml; exit 1; }

# ---------------------------------------------------------------- 4. a patch is not newer
sed "s/^tool:.*/tool: \"$mine\"/" .ai/manifest.yaml > manifest.tmp && cat manifest.tmp > .ai/manifest.yaml && rm -f manifest.tmp
"$RB" knowledge candidates > rb.out 2>&1 || true
if grep -q 'newer than this one' rb.out; then echo "    the same major.minor was refused as newer:"; cat rb.out; exit 1; fi

echo "    a layer written for 99.0 is refused by both programs naming 99.0 and $full; migrate --layer leaves it; $mine is read"

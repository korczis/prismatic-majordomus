# majordomus-exclusive: compares measured durations with thresholds; a busy pool would add the noise it refuses
# majordomus-covers: bench
# majordomus-negative: bench
#
# The baseline is the accepted performance state: written only by --write-baseline, only on
# a clean tree unless forced, into the tracked half of the layer where it is reviewed like
# any other change; it carries its own schema and the commit it was measured at, and a second
# write reports what moved against it.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq is required to read the baseline"; exit 1; }
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
B=.ai/repo/benchmarks/baseline.json

# --- a dirty tree is refused: a baseline records a commit
echo dirty > lib/b
expect_exit 15 "$MJ" bench version --samples 1 --warmup 0 --write-baseline --no-save
expect_grep 'working tree is dirty'
[ ! -e "$B" ] || { echo "    a refused write still produced a baseline"; exit 1; }
# --- --force writes anyway, and says so in the document
expect_exit 0 "$MJ" bench version --samples 1 --warmup 0 --write-baseline --force --no-save
expect_grep "wrote $B"
jq -e '.schema == "majordomus/benchmark-baseline/v1" and .repository.dirty == true' "$B" >/dev/null || { echo "    the forced baseline does not say the tree was dirty"; cat "$B"; exit 1; }
rm -f lib/b "$B"

# --- on a clean tree: the document, its schema, its commit, and its place under review
expect_exit 0 "$MJ" bench version --samples 2 --warmup 0 --write-baseline --no-save
expect_grep "wrote $B"
jq -e '.schema == "majordomus/benchmark-baseline/v1" and .repository.dirty == false and (.results | length == 2)' "$B" >/dev/null || { echo "    the baseline is not a complete document"; cat "$B"; exit 1; }
[ "$(jq -r .repository.commit "$B")" = "$(git rev-parse HEAD)" ] || { echo "    the baseline does not record the commit it was measured at"; exit 1; }
[ "$(git status --porcelain -uall)" = "?? $B" ] || { echo "    the baseline is not the one reviewable change"; git status --porcelain; exit 1; }
[ -z "$(find .ai/repo/benchmarks -name '.*' -type f)" ] || { echo "    a temporary file survived the write"; exit 1; }
git add "$B" && git commit -qm baseline
# --- the written file round-trips: the same machine, the same command, zero regressions
expect_exit 0 "$MJ" bench version --samples 2 --warmup 0 --check --no-save
expect_grep 'OK +bench +version cold .* within thresholds'
expect_grep 'OK +bench +version warm .* within thresholds'

# --- a second write reports the move against the previous baseline, per target and mode
expect_exit 0 "$MJ" bench version --samples 2 --warmup 0 --write-baseline --no-save
expect_grep "^baseline $B -> new run"
expect_grep '^  version +cold +[0-9]+/[0-9]+/[0-9]+ -> [0-9]+/[0-9]+/[0-9]+'
expect_grep '^  version +warm +[0-9]+/[0-9]+/[0-9]+ -> [0-9]+/[0-9]+/[0-9]+'
[ "$(git status --porcelain)" = " M $B" ] || { echo "    the rewrite is not a reviewable diff of the baseline"; git status --porcelain; exit 1; }
echo "    refused dirty, forced with the flag, written clean with its commit, rewritten as a reviewable diff"

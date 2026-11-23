# majordomus-timeout: 900
# majordomus-covers: doctor watch context check history prompt search skills version doctrine evidence knowledge bench
# A cache never changes a read-only answer (I0841). Every command the registry classes
# read-only, and every read-only subcommand its effects name, is run three times against one
# fixture grown synthetically: with the flatten cache cold, warm, and unavailable. Each run must print the same
# bytes and exit with the same status. Then one file a command reads is changed: the answer
# changes with it and the counters show the read that was not served from the cache.
#
# The list is the registry's, read with the tool's own flattener, so a read-only command added
# later is covered here without a line of this case changing; an empty list is a failure,
# not a pass over nothing. Two lines are dropped before comparing, and only these: the wall
# time doctor and watch report against their budget, which differs run to run with or
# without a cache. The clock context prints is pinned.
. "$ROOT/test/lib.sh"

fail() { printf '    %s\n' "$*"; exit 1; }

# ---------------------------------------------------------------- the commands, from the registry
. "$ROOT/lib/common.sh"
REG="$T/commands.flat"
mj_yaml_flatten "$ROOT/share/commands.yaml" > "$REG" || fail "share/commands.yaml does not parse"
# one invocation per line: a read-only command bare, a read-only subcommand after its command
awk -F= '
  { k = $1; v = $0; sub(/^[^=]*=/, "", v) }
  k ~ /^commands\.[0-9]+\.id$/ { split(k, p, "."); id[p[2]] = v; next }
  k ~ /^commands\.[0-9]+\.class$/ { split(k, p, "."); cls[p[2]] = v; next }
  k ~ /^commands\.[0-9]+\.effects\.[0-9]+\.subcommand$/ { split(k, p, "."); sub_[p[2] "." p[4]] = v; next }
  k ~ /^commands\.[0-9]+\.effects\.[0-9]+\.class$/ { split(k, p, "."); ecls[p[2] "." p[4]] = v; next }
  END {
    for (n in id) if (cls[n] == "read-only") print id[n]
    for (e in sub_) if (ecls[e] == "read-only") { split(e, p, "."); print id[p[1]] " " sub_[e] }
  }' "$REG" | LC_ALL=C sort -u > "$T/invocations"
[ -s "$T/invocations" ] || fail "the registry names no read-only command; there is nothing to compare"
grep -qx doctor "$T/invocations" || fail "doctor is not among the read-only commands: $(tr '\n' ' ' < "$T/invocations")"
total="$(wc -l < "$T/invocations" | tr -d ' ')"

# ---------------------------------------------------------------- the fixture
export MAJORDOMUS_NOW=2026-10-16T00:00:00Z
# a repository of its own below the scratch directory, so the answers kept in $T are not
# files of the tree whose cleanliness is asserted below
R="$T/repo"; mkdir -p "$R"; cd "$R" || exit 1
git init -q .; git config user.email t@example.com; git config user.name t
"$MJ" init >/dev/null; "$MJ" update >/dev/null
# grown by records of every kind the validators read, so a cache entry is asked for many
# files of one kind and not only for the skeleton's few
synthetic_repo 6
git add -A >/dev/null; git commit -qm fixture
grep -q '^  flatten_max_entries: [1-9]' .ai/repo/policy.yaml || fail "the skeleton's policy declares no flatten cache"
CACHE=.ai/local/cache/flat

# run_all TAG: every invocation's stdout and exit status into $T/TAG.<n>, its counters into
# $T/TAG.err; each command keeps its own timing report and prints it on stderr at its exit
run_all() {
  local n=0 c s
  : > "$T/$1.err"
  while IFS= read -r c; do
    n=$((n + 1))
    # shellcheck disable=SC2086  # an invocation is a command and its subcommand, two words
    { s=0; MJ_TIMING=1 "$MJ" $c 2>>"$T/$1.err" </dev/null || s=$?; printf 'exit=%s\n' "$s"; } \
      | grep -vE '^(INFO|WARN) +budget +(doctor|watch) ' > "$T/$1.$n"
  done < "$T/invocations"
}
counted() { awk -v k="$2" '$1 == "count" && $3 == k { n += $2 } END { print n + 0 }' "$T/$1.err"; }
same() { # TAG-A TAG-B: every invocation printed the same and exited the same
  local n=0 c
  while IFS= read -r c; do
    n=$((n + 1))
    cmp -s "$T/$1.$n" "$T/$2.$n" || { diff "$T/$1.$n" "$T/$2.$n" | head -n 10; fail "'majordomus $c' answered differently $1 and $2"; }
  done < "$T/invocations"
}

# cold: no cache directory at all; warm: the one the cold run wrote
rm -rf .ai/local/cache
run_all cold
[ -d "$CACHE" ] || fail "the read-only commands wrote no flatten cache"
run_all warm
same cold warm
[ "$(counted warm flatten_cache_hit)" -gt "$(counted cold flatten_cache_hit)" ] \
  || fail "a warm cache counted $(counted warm flatten_cache_hit) hits, a cold one $(counted cold flatten_cache_hit)"

# unavailable: the cache's directory cannot be made, so there is no cache and nothing is served
rm -rf .ai/local/cache; mkdir -p .ai/local/cache; : > "$CACHE"
run_all off
[ -f "$CACHE" ] || fail "a command replaced the blocked cache path"
[ "$(counted off flatten_cache_hit)" = 0 ] || fail "$(counted off flatten_cache_hit) hit(s) were served with no cache"
[ "$(counted off yaml_flatten)" = "$(counted cold yaml_flatten)" ] \
  || fail "yaml_flatten counted $(counted cold yaml_flatten) cold and $(counted off yaml_flatten) with no cache: it counts the asking, not the parsing"
same cold off
rm -f "$CACHE"
[ -z "$(git status --porcelain)" ] || fail "a read-only command changed the tree: $(git status --porcelain | head -n 3)"

# ---------------------------------------------------------------- a changed file is read again
# A profile's limit is printed by doctor; changing it must change doctor's answer even with
# the old profile's flat lines sitting in the cache, and the read of it is a miss.
P=.ai/repo/profiles/debugging.yaml
grep -q '^checkpoint_interval: 15m$' "$P" || fail "the fixture's $P sets no checkpoint_interval of 15m"
run_all prime   # the cache was removed above: warm it, so that "before" is a warm run
run_all before
same prime before
sed -i.bak 's/^checkpoint_interval: 15m$/checkpoint_interval: 16m/' "$P"; rm -f "$P.bak"
grep -q '^checkpoint_interval: 16m$' "$P" || fail "the probe did not take"
run_all after
i="$(grep -nx doctor "$T/invocations" | cut -d: -f1)"
grep -q 'profile-limits debugging .*checkpoint_max_age_seconds=900' "$T/before.$i" || fail "doctor did not print the profile's limit: $(grep profile-limits "$T/before.$i" | head -n 2)"
grep -q 'profile-limits debugging .*checkpoint_max_age_seconds=960' "$T/after.$i" || fail "doctor was served the old profile: $(grep 'profile-limits debugging' "$T/after.$i")"
miss() { echo $(( $(counted "$1" yaml_flatten) - $(counted "$1" flatten_cache_hit) )); }
[ "$(miss after)" -gt "$(miss before)" ] || fail "the changed profile was not a miss: $(miss before) misses before, $(miss after) after"
printf '    %s read-only invocation(s) compared cold, warm and without a cache\n' "$total"
exit 0

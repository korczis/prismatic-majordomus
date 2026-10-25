# majordomus-timeout: 600
# A flatten is cached by its content (I0821). mj_yaml_flatten keeps each file's flat lines
# under .ai/local/cache/flat/<parser>/ and serves them only to a file with exactly the bytes
# they were parsed from: a warm cache prints what a cold one prints, byte for byte, for every
# YAML file of this repository and for the read-only commands of a fresh one; a changed file
# is parsed again even where it lands in the same slot; a truncated, forged or unreadable
# entry is never served; a failed parse is never kept; the directory never holds more than
# cache.flatten_max_entries, the oldest written removed first; a policy without the key, or
# with 0, has no cache; a changed parser reads none of what an older one wrote; and
# MJ_TIMING counts every hit as flatten_cache_hit while yaml_flatten counts what it did.
. "$ROOT/test/lib.sh"

fail() { printf '    %s\n' "$*"; exit 1; }

# ---------------------------------------------------------------- the function itself
export MJ_TIMING=1 MJ_TIMING_FILE="$T/timing"
: > "$MJ_TIMING_FILE"
. "$ROOT/lib/common.sh"
hits() { grep -c '	flatten_cache_hit	' "$MJ_TIMING_FILE" || true; }
LOC="$T/local"; mkdir -p "$LOC"
MJ_AI_LOCAL_DIR="$LOC"; MJ_POL_FLAT="$T/pol.flat"
printf 'cache.flatten_max_entries=5000\n' > "$MJ_POL_FLAT"
mj_flat_cache_init
[ -n "$MJ_FLAT_CACHE_DIR" ] && [ -d "$MJ_FLAT_CACHE_DIR" ] || fail "a policy with a cap made no cache"
case "$MJ_FLAT_CACHE_DIR" in "$LOC/cache/flat/"????????????????) ;; *) fail "the cache is not under cache/flat/<parser>: $MJ_FLAT_CACHE_DIR" ;; esac
D="$MJ_FLAT_CACHE_DIR"
# out <file> -> the flatten's stdout exactly (the status after it keeps trailing newlines),
# its exit status and its stderr; fresh <file> -> the same from the parser alone
out() { local o; o="$(mj_yaml_flatten "$1" 2>"$T/err"; printf 'exit=%s' "$?")"; printf '%s\n%s' "$o" "$(cat "$T/err")"; }
fresh() { local o; o="$(MJ_FLAT_CACHE_DIR='' mj_yaml_flatten "$1" 2>"$T/err"; printf 'exit=%s' "$?")"; printf '%s\n%s' "$o" "$(cat "$T/err")"; }

# every YAML file of the repository: cold and warm both print the parser's own bytes
n=0
while IFS= read -r f; do
  want="$(fresh "$f")"
  [ "$(out "$f")" = "$want" ] || fail "cold: $f differs from a fresh flatten"
  [ "$(out "$f")" = "$want" ] || fail "warm: $f differs from a fresh flatten"
  case "$want" in *exit=0) n=$((n + 1)) ;; esac   # a file that does not parse is never kept
done < <(cd "$ROOT" && git ls-files '*.yaml' | sed "s|^|$ROOT/|")
[ "$n" -gt 100 ] || fail "only $n YAML files that parse were compared"
[ "$(hits)" -ge "$n" ] || fail "$n files flattened twice, but only $(hits) hits were counted"
[ -z "$(find "$D" -name '.w.*')" ] || fail "a writer left its temporary file"

# a changed file is parsed again, even when it lands in the same slot: the slot samples
# sixteen positions, so a change of one character at any other leaves the name as it was
mkdir -p "$T/m"; F="$T/m/item.yaml"
printf 'id: X1\ntitle: abcdefghijklmnopqrstuvwxyz\nnote: the quick brown fox jumps over the lazy dog\n' > "$F"
before="$(out "$F")"
len="$(wc -c < "$F" | tr -d ' ')"
pos=""; i=1
while [ "$i" -lt "$((len - 1))" ]; do
  sampled=0; for k in 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do [ "$i" = "$(( (len - 1) * k / 15 ))" ] && sampled=1; done
  c="$(dd if="$F" bs=1 skip="$i" count=1 2>/dev/null)"
  case "$c" in [a-y]) [ "$sampled" = 0 ] && { pos="$i"; break; } ;; esac
  i=$((i + 1))
done
[ -n "$pos" ] || fail "no unsampled letter in the fixture"
entries_before="$(ls "$D" | LC_ALL=C sort)"
{ head -c "$pos" "$F"; printf 'z'; tail -c +"$((pos + 2))" "$F"; } > "$F.new" && mv "$F.new" "$F"
[ "$(wc -c < "$F" | tr -d ' ')" = "$len" ] || fail "the mutation changed the length"
h0="$(hits)"
after="$(out "$F")"
[ "$after" = "$(fresh "$F")" ] || fail "a changed file was not parsed again"
[ "$after" != "$before" ] || fail "the mutation changed nothing the flatten prints"
[ "$(hits)" = "$h0" ] || fail "a changed file was served from the cache"
[ "$(ls "$D" | LC_ALL=C sort)" = "$entries_before" ] || fail "the same-length change did not land in the same slot, so the case proves nothing"

# a damaged entry is never served, and the miss writes a good one back
printf 'id: X2\nlist: [a, b]\nmap:\n  k: v\n' > "$F"
want="$(fresh "$F")"; out "$F" >/dev/null
E="$D/$(ls -t "$D" | head -n 1)"
src="$(cat "$F"; printf x)"; src="${src%x}"
damage() { # <name> : write $E as that kind of damage
  case "$1" in
    truncated) head -c 20 "$E" > "$E.t" && mv "$E.t" "$E" ;;
    garbage) printf 'nonsense\n' > "$E" ;;
    forged-source) printf 'mjflat 1 %s 7\n%s%s' "${#src}" "${src/X2/Y2}" 'evil=1
' > "$E" ;;
    forged-length) printf 'mjflat 1 %s 999\n%s%s' "${#src}" "$src" 'evil=1
' > "$E" ;;
    empty) : > "$E" ;;
    unreadable) chmod 000 "$E" ;;
  esac
}
for kind in truncated garbage forged-source forged-length empty unreadable; do
  out "$F" >/dev/null   # a good entry
  damage "$kind"
  h0="$(hits)"
  got="$(out "$F")"
  [ "$got" = "$want" ] || fail "a $kind entry changed what the flatten prints"
  case "$kind" in unreadable) ;; *) [ "$(hits)" = "$h0" ] || fail "a $kind entry was served as a hit" ;; esac
  [ "$(out "$F")" = "$want" ] && [ "$(hits)" -gt "$h0" ] || fail "after a $kind entry the miss did not write a good one back"
done

# a failed parse prints and exits as it always did, and is never kept
printf 'a: 1\nb:\n\tc: 2\n' > "$T/m/bad.yaml"
before="$(ls "$D" | wc -l)"
got="$(out "$T/m/bad.yaml")"; want="$(fresh "$T/m/bad.yaml")"
[ "$got" = "$want" ] || fail "a failed parse printed differently through the cache"
printf '%s' "$got" | grep -q 'ERROR:tab character on line 3' || fail "the parse error did not reach stderr"
s=0; mj_yaml_flatten "$T/m/bad.yaml" >/dev/null 2>&1 || s=$?
[ "$s" = 3 ] || fail "a failed parse exited $s, not 3"
[ "$(ls "$D" | wc -l)" = "$before" ] || fail "a failed parse was kept"
[ -z "$(find "$D" -name '.w.*')" ] || fail "a failed parse left its temporary file"

# the cap: never more entries than it, the oldest written removed first
rm -rf "$LOC/cache"; printf 'cache.flatten_max_entries=3\n' > "$MJ_POL_FLAT"; mj_flat_cache_init
D="$MJ_FLAT_CACHE_DIR"
for i in 1 2 3 4 5 6; do
  printf 'id: C%s\n' "$i" > "$T/m/c$i.yaml"
  mj_yaml_flatten "$T/m/c$i.yaml" >/dev/null
  [ "$(ls "$D" | wc -l | tr -d ' ')" -le 3 ] || fail "the cache holds $(ls "$D" | wc -l) entries over a cap of 3"
  # written in order, a minute apart, on a file system of any resolution
  touch -t "20200101000$i" "$D/$(ls -t "$D" | head -n 1)"
done
h0="$(hits)"
for i in 4 5 6; do mj_yaml_flatten "$T/m/c$i.yaml" >/dev/null; done
[ "$(hits)" = "$((h0 + 3))" ] || fail "the three newest entries were not the ones kept"
mj_yaml_flatten "$T/m/c1.yaml" >/dev/null
[ "$(hits)" = "$((h0 + 3))" ] || fail "the oldest entry was kept past the cap"

# no key, or 0, is no cache; a changed parser reads none of what an older one wrote
: > "$MJ_POL_FLAT"; mj_flat_cache_init
[ -z "$MJ_FLAT_CACHE_DIR" ] || fail "a policy without cache.flatten_max_entries has a cache"
printf 'cache.flatten_max_entries=0\n' > "$MJ_POL_FLAT"; mj_flat_cache_init
[ -z "$MJ_FLAT_CACHE_DIR" ] || fail "a cap of 0 has a cache"
printf 'cache.flatten_max_entries=10\n' > "$MJ_POL_FLAT"; mj_flat_cache_init; prev="$MJ_FLAT_CACHE_DIR"
MJ_FLATTEN_AWK="$MJ_FLATTEN_AWK
  # another parser"
MJ_FLAT_CACHE_KEY=""; mj_flat_cache_init
[ -n "$MJ_FLAT_CACHE_DIR" ] && [ "$MJ_FLAT_CACHE_DIR" != "$prev" ] || fail "a changed parser reads the old parser's directory"

# ---------------------------------------------------------------- the commands
# A fresh repository's read-only commands print the same with a cold cache and a warm one.
# Doctor's own wall time is the one line that differs from run to run, cache or none, and
# the clock is pinned for the header context prints.
export MAJORDOMUS_NOW=2026-10-16T00:00:00Z
R="$T/repo"; mkdir -p "$R"; cd "$R" || exit 1
git init -q .; git config user.email t@example.com; git config user.name t
MJ_TIMING="" "$MJ" init >/dev/null; MJ_TIMING="" "$MJ" update >/dev/null
git add -A >/dev/null; git commit -qm fixture
grep -q '^  flatten_max_entries: [1-9]' .ai/repo/policy.yaml || fail "the skeleton's policy declares no flatten cache"
# each command keeps its own counters and reports them at its exit
MJ_TIMING_FILE=""
run_all() { # <tag>: each command's stdout and exit status into $T/<tag>.<n>, the timing reports into $T/<tag>.err
  local i=0 c s
  : > "$T/$1.err"
  for c in "doctor" "context" "history" "prompt list" "skills list" "check"; do
    i=$((i + 1))
    # shellcheck disable=SC2086
    { s=0; "$MJ" $c 2>>"$T/$1.err" || s=$?; printf 'exit=%s\n' "$s"; } | grep -v ' budget  *doctor ' > "$T/$1.$i"
  done
}
counted() { awk -v k="$2" '$1 == "count" && $3 == k { n += $2 } END { print n + 0 }' "$T/$1.err"; }
rm -rf .ai/local/cache
run_all cold
[ -d .ai/local/cache/flat ] || fail "the commands wrote no cache"
run_all warm
for i in 1 2 3 4 5 6; do
  cmp -s "$T/cold.$i" "$T/warm.$i" || { diff "$T/cold.$i" "$T/warm.$i" | head -n 10; fail "command $i printed differently with a warm cache"; }
done
[ "$(counted cold yaml_flatten)" -gt 0 ] || fail "the cold run counted no flatten: $(head -n 3 "$T/cold.err")"
[ "$(counted cold yaml_flatten)" = "$(counted warm yaml_flatten)" ] \
  || fail "yaml_flatten counted $(counted cold yaml_flatten) cold and $(counted warm yaml_flatten) warm: it counts the asking, not the parsing"
[ "$(counted warm flatten_cache_hit)" -gt "$(counted cold flatten_cache_hit)" ] \
  || fail "a warm cache counted $(counted warm flatten_cache_hit) hits, a cold one $(counted cold flatten_cache_hit)"
[ -z "$(git status --porcelain)" ] || fail "the cache is visible to git: $(git status --porcelain | head -n 3)"
exit 0

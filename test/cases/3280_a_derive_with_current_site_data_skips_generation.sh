# majordomus-covers: none
# A derive whose site data is current skips its generation, and only then (I2880).
#
# scripts/derive used to run stage B, scripts/generate-site-data, on every call. Stage B is
# most of a derivation's work, and on 2026-10-17 derives waited 15.3 h on the machine's lock
# against 9.5 h of work. It now asks `generate-site-data --derive-key` and skips stage B when
# the key equals the derive_key the committed source.json records.
#
# Two halves, each held here:
#   1. scripts/derive skips stage B on an equal key, says so with the key, and runs it on an
#      unequal one, an absent record, or MAJORDOMUS_DERIVE_FULL=1 (a fixture with a stub
#      generator, the real scripts/derive unmodified);
#   2. the real generator's key moves with everything the site data depends on: a canonical
#      input, the generator itself, a library under lib/ or scripts/lib/, and the executable's
#      sources. The input fingerprint alone moves with the first only, so a derive keyed on it
#      would skip a generation a changed generator owes.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"

# ---------------------------------------------------------------- 1. the skip in scripts/derive
G="$T/graph"; TOOL="$T/tool"; LOG="$T/calls.log"
mkdir -p "$G/scripts" "$G/lib" "$G/site/data/generated" "$G/out" "$TOOL"
cp "$ROOT/scripts/derive" "$G/scripts/"
cp "$ROOT/lib/rust_bin.sh" "$ROOT/lib/machine_lock.sh" "$G/lib/"
cat > "$TOOL/majordomus" <<'SH'
#!/usr/bin/env bash
printf 'generate\n' >> "$STUB_LOG"
SH
# The stub generator: its key is key.txt, and a generation records it the way the real one
# records derive_key in source.json.
cat > "$G/scripts/generate-site-data" <<'SH'
#!/usr/bin/env bash
set -eu
cd "$(dirname "$0")/.."
if [ "${1:-}" = --derive-key ]; then cat key.txt; exit 0; fi
printf 'site-data\n' >> "$STUB_LOG"
printf '{\n  "schema": 1,\n  "derive_key": "%s"\n}\n' "$(cat key.txt)" > site/data/generated/source.json
SH
for s in gitattributes rules-package; do printf '#!/usr/bin/env bash\nexit 0\n' > "$G/scripts/$s"; done
chmod +x "$TOOL/majordomus" "$G/scripts/"*
STUB_LOG="$LOG"; export STUB_LOG
( cd "$G" && git init -q . && git config user.email t@example.com && git config user.name t \
  && git add -A && git commit -qm sources )
export MAJORDOMUS_DERIVE_LOCK="$T/derive.lock" MAJORDOMUS_DERIVE_PRIORITY="$T/derive.priority"
export MAJORDOMUS_DERIVE_ALLOW_UNTRACKED=1
derive() { MAJORDOMUS_BIN="$TOOL/majordomus" bash "$G/scripts/derive"; }
ran_b() { grep -qx 'site-data' "$LOG"; }

printf 'aaaa1111\n' > "$G/key.txt"
# no record yet: stage B runs and writes one
: > "$LOG"; expect_exit 0 derive
ran_b || { echo "    a tree with no recorded key did not generate"; exit 1; }
# the record equals the key: stage B is skipped, and the derive says which key it matched
: > "$LOG"; expect_exit 0 derive
# MUTATION, RUN: never skip (the `if` always takes the else branch) -> stage B runs here.
ran_b && { echo "    stage B ran although the recorded key equals the tree's"; exit 1; }
expect_grep 'stage B skipped: the committed site data is current \(derive key aaaa1111\)'
# MAJORDOMUS_DERIVE_FULL=1 runs it anyway
# MUTATION, RUN: skip whenever a record exists (`if [ -n "$B_WANT" ]`) -> skipped here, and
# on the different key below.
: > "$LOG"; MAJORDOMUS_DERIVE_FULL=1 expect_exit 0 derive
ran_b || { echo "    MAJORDOMUS_DERIVE_FULL=1 did not generate"; exit 1; }
# a different key: stage B runs, and records the new key
printf 'bbbb2222\n' > "$G/key.txt"
: > "$LOG"; expect_exit 0 derive
ran_b || { echo "    stage B was skipped although the tree's key differs from the record"; exit 1; }
expect_grep '"derive_key": "bbbb2222"' "$G/site/data/generated/source.json"
# a generator that cannot compute a key never matches, even an empty record
# MUTATION, RUN: drop `[ -n "$B_HAVE" ]` from the skip condition -> skipped here.
: > "$G/key.txt"
printf '{\n  "derive_key": ""\n}\n' > "$G/site/data/generated/source.json"
: > "$LOG"; expect_exit 0 derive
ran_b || { echo "    an empty key was taken as a match"; exit 1; }

# ---------------------------------------------------------------- 2. what the real key covers
# A copy of the committed tree, with this checkout's scripts/generate-site-data and
# scripts/derive over it, under a git of its own: the key reads git's file list.
C="$T/copy"; mkdir -p "$C"
( cd "$ROOT" && git archive HEAD ) | ( cd "$C" && tar -xf - )
cp "$ROOT/scripts/generate-site-data" "$ROOT/scripts/derive" "$C/scripts/"
( cd "$C" && git init -q . && git add -A >/dev/null 2>&1 )
key() { ( cd "$C" && MAJORDOMUS_SHARE="$C/share" scripts/generate-site-data --derive-key ); }
fpr() { ( cd "$C" && MAJORDOMUS_SHARE="$C/share" scripts/generate-site-data --fingerprint ); }
k0="$(key)"; f0="$(fpr)"
printf '%s' "$k0" | grep -Eq '^[0-9a-f]{64}$' || { echo "    --derive-key printed '$k0', not a sha256"; exit 1; }
[ "$(key)" = "$k0" ] || { echo "    --derive-key is not stable on an unchanged tree"; exit 1; }

moves() { # moves WHAT FILE: append a line to FILE; the key must move, then restore it
  local what="$1" f="$C/$2" k
  [ -f "$f" ] || { echo "    the fixture has no $2"; exit 1; }
  cp "$f" "$T/keep"; printf '\n# %s\n' "$what" >> "$f"
  k="$(key)"; cp "$T/keep" "$f"
  [ "$k" != "$k0" ] || { echo "    the derive key did not move when $what changed ($2)"; exit 1; }
  [ "$(key)" = "$k0" ] || { echo "    restoring $2 did not restore the derive key"; exit 1; }
}
# MUTATION, RUN: key on the input fingerprint alone (mj_site_derive_key printing
# mj_site_inputs_hash) -> the second line fails: the generator changed, the key did not.
moves "a canonical input" "$(cd "$C" && scripts/generate-site-data --inputs | grep -m1 '^docs/.*\.md$')"
moves "the generator" scripts/generate-site-data
moves "a library it sources" lib/common.sh
moves "an awk program it reads" scripts/lib/project-markdown.awk
moves "the executable's sources" apps/majordomus-cli/Cargo.toml
# and the input fingerprint itself does not see a generator change, which is why the key exists
printf '\n# a generator change\n' >> "$C/lib/common.sh"
[ "$(fpr)" = "$f0" ] || { echo "    the input fingerprint moved with lib/common.sh; the case's premise no longer holds"; exit 1; }

# ---------------------------------------------------------------- 3. the committed record
# The committed source.json carries a derive_key, so a derive on a current tree can skip.
expect_grep '"derive_key": "[0-9a-f]{64}"' "$ROOT/site/data/generated/source.json"

# majordomus-covers: archive capability:pack.plan capability:pack.verify
# majordomus-negative: capability:pack.plan capability:pack.verify
# majordomus-skill: pack-for-chatgpt
# A source pack is read at the far end by a model that cannot check it against the
# repository, so what it must never carry is proved here by putting each thing into a
# disposable repository and showing it absent from what is written: a binary by extension
# and by content, a gitlink (a nested worktree), a symbolic link, build output under
# target/, a worktree container, a minified bundle, and a generated projection
# (project.packs-carry-only-sources). Then every refusal is proved by breaking the pack it
# guards: a shard changed after it was verified, a file dropped into the pack, and a
# source that names the machine it was packed on, which refuses the build outright. Last,
# the shell's `archive` reads the same profile and drops the same artifacts.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null || skip "jq absent"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN
S="$(mktemp -d "${TMPDIR:-/tmp}/mj830.XXXXXX")"; trap 'rm -rf "$S"' EXIT
"$MJ" init >/dev/null; "$MJ" update >/dev/null

mkdir -p src target/debug apps/x/target/release repo-wt/feature/a vendor gen
printf 'fn main() {}\n' > src/main.rs
printf '# Notes\n\n````md\nfour ticks inside\n````\n' > src/notes.md
printf 'PNG' > src/logo.png
printf 'text\000with a nul' > src/blob.dat
printf 'fn built() {}\n' > target/debug/out.rs
printf 'fn built() {}\n' > apps/x/target/release/y.rs
printf 'fn other() {}\n' > repo-wt/feature/a/file.rs
printf 'var a=1;\n' > vendor/lib.min.js
printf '{}\n' > gen/projection.json
printf 'gen/** merge=derived\n' >> .gitattributes
printf '/tmp/\n' >> .gitignore
ln -s /elsewhere src/link
git add -A >/dev/null
git update-index --add --cacheinfo 160000,1111111111111111111111111111111111111111,nested-wt
git commit -qm base

# ---- the plan names every file it leaves out, and why ----------------------------------
expect_exit 0 "$RB" pack plan chatgpt
expect_grep '^pack plan: clean$'
"$RB" pack plan chatgpt --format json 2>/dev/null > "$S/plan.json"
reason() { jq -r --arg p "$1" '.dropped_files[] | select(.path == $p) | .reason' "$S/plan.json"; }
for want in 'src/logo.png binary' 'src/blob.dat binary' 'target/debug/out.rs artifact' \
            'apps/x/target/release/y.rs artifact' 'repo-wt/feature/a/file.rs artifact' \
            'vendor/lib.min.js artifact' 'nested-wt worktree' 'src/link link'; do
  set -- $want
  [ "$(reason "$1")" = "$2" ] || { echo "    $1 is not dropped as $2:"; cat "$S/plan.json"; exit 1; }
done
[ "$(jq -r '.dropped.derived' "$S/plan.json")" = 1 ] \
  || { echo "    the derived projection is not counted as dropped"; cat "$S/plan.json"; exit 1; }
jq -e '.passes and .measured and .selected > 0 and (.shards | length) >= 1' "$S/plan.json" >/dev/null \
  || { echo "    the plan does not pass with a selection"; cat "$S/plan.json"; exit 1; }

# ---- the build writes it, verifies it, and nothing forbidden is in it -------------------
P=tmp/packs/t
expect_exit 0 "$RB" pack build chatgpt --out "$P"
expect_grep '^pack verify: clean$'
[ -f "$P/00-INDEX.md" ] && [ -f "$P/pack.json" ] || { echo "    the index or the manifest is missing"; ls -la "$P"; exit 1; }
cat "$P"/0[1-9]-*.md > "$S/all.md"
grep -q '^## src/main.rs$' "$S/all.md" || { echo "    a source is not carried"; exit 1; }
for gone in src/logo.png src/blob.dat target/debug/out.rs apps/x/target/release/y.rs \
            repo-wt/feature/a/file.rs vendor/lib.min.js nested-wt src/link gen/projection.json; do
  if grep -qF "## $gone" "$S/all.md"; then echo "    $gone travelled in the pack"; exit 1; fi
done
if LC_ALL=C tr -d '\000' < "$S/all.md" | cmp -s - "$S/all.md"; then :; else
  echo "    a shard holds a NUL byte"; exit 1
fi
# a fence longer than the content's own backticks, so the content cannot close it
grep -q '^`````markdown$' "$S/all.md" || { echo "    the four-backtick file is not fenced with five"; exit 1; }
expect_exit 0 "$RB" pack verify "$P"
# a second build refuses to overwrite without --force, and replaces with it
expect_exit 13 "$RB" pack build chatgpt --out "$P"
expect_grep 'already holds a pack'
expect_exit 0 "$RB" pack build chatgpt --out "$P" --force

# ---- every refusal of the verifier, by breaking what it guards --------------------------
shard="$(ls "$P"/01-*.md)"
cp "$shard" "$S/shard.bak"
printf 'tampered\n' >> "$shard"
expect_exit 10 "$RB" pack verify "$P"
expect_grep 'pack.tampered'
cp "$S/shard.bak" "$shard"
expect_exit 0 "$RB" pack verify "$P"
printf 'PNG' > "$P/logo.png"
expect_exit 10 "$RB" pack verify "$P"
expect_grep 'pack.stray'
rm "$P/logo.png"
expect_exit 12 "$RB" pack verify tmp/packs/absent
expect_grep 'not measured'
expect_exit 13 "$RB" pack verify ../outside
expect_grep 'leaves the repository'

# ---- a source naming this machine refuses the build, and nothing is written -------------
printf '// built in %s/src\n' "$PWD" > src/where.rs
git add -A >/dev/null && git commit -qm where
expect_exit 10 "$RB" pack plan chatgpt
expect_grep 'pack.leak +src/where.rs +line 1 names this machine'
expect_exit 10 "$RB" pack build chatgpt --out tmp/packs/leaky
[ ! -e tmp/packs/leaky ] || { echo "    a pack was written from a plan with a finding"; exit 1; }
git rm -q src/where.rs && git commit -qm 'where goes'

# ---- the shell's archive reads the same profile and drops the same artifacts ------------
# by extension and by content, as the pack does: the NUL-byte file is not zipped as text
expect_exit 0 "$MJ" archive chatgpt --dry-run
expect_grep 'left out: 4 \(artifact\)'
expect_grep 'left out: 2 \(binary\)'
expect_grep 'left out: 1 \(link\)'

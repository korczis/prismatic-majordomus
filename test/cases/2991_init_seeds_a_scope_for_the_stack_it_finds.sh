# majordomus-covers: init
# claims: scope-declared
# The scope `init` seeds says what a worker reads of the repository, so it has to be about
# that repository. The skeleton names the layer and the places any stack keeps its source,
# and no language; what this tree is made of is read from git: every entry tracked at the
# root that the skeleton does not already reach becomes a line of `in`, and each guess is
# said. A JavaScript repository is given no Elixir and no Rust path, an Elixir one is given
# its own, and neither is given the other's.
#
# What it must not become:
#
#   a list of stacks. Nothing here recognises a language; a tree of a kind nobody has seen
#   is given its own root, by the same reading.
#
#   a way past `out`. A committed build directory stays out and a file over the limit stays
#   out; what changes is that init says so, with the key to change, rather than leaving the
#   adopter to find out when a worker cannot read the deliverable.
#
#   a rewrite. An existing scope is the repository's, and --extend leaves it byte for byte.
. "$ROOT/test/lib.sh"
SKEL="$ROOT/share/skeleton/ai/repo/scope.yaml"
ERR="$(mktemp "${TMPDIR:-/tmp}/mj.case.XXXXXX")"
HERE="$PWD"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj2991.XXXXXX")"; trap 'rm -rf "$S" "$ERR"' EXIT
repo() { mkdir -p "$S/$1" && cd "$S/$1" && git init -q && git config user.email t@example.invalid && git config user.name t; }

# ---------------------------------------------------------------- the skeleton names no stack
for stack in 'mix\.' 'Cargo\.' 'priv/' 'apps/' '\.exs' 'package\.json' 'go\.mod' 'pyproject'; do
  grep -Eq "^ *- .*$stack" "$SKEL" && { echo "    the skeleton scope names a stack: $(grep -E "^ *- .*$stack" "$SKEL" | head -1)"; exit 1; }
done

# ---------------------------------------------------------------- a JavaScript repository
repo js
mkdir -p src scripts dist .github/workflows "odd dir"
echo '{}' > package.json; echo x > src/a.js; echo x > scripts/build.sh; echo x > README.md
echo x > .github/workflows/ci.yml; echo x > "odd dir/f"
head -c 1200000 /dev/zero | tr '\0' 'a' > dist/index.html
git add -A >/dev/null; git commit -qm base
"$MJ" init > /dev/null 2> "$ERR"
grep -Eq '^  - (mix\.|Cargo\.|priv/|apps/)' .ai/repo/scope.yaml \
  && { echo "    a JavaScript repository was seeded with another stack's paths"; grep -E '^  - ' .ai/repo/scope.yaml; exit 1; }
for want in 'package.json' 'scripts/**' '.github/**'; do
  grep -qxF -- "  - $want" .ai/repo/scope.yaml || { echo "    the seeded scope does not name $want"; sed -n '/^in:/,/^out:/p' .ai/repo/scope.yaml; exit 1; }
done
# what the skeleton already reaches is not said twice, and an excluded directory is not let in
[ "$(grep -cxF -- '  - src/**' .ai/repo/scope.yaml)" = 1 ] || { echo "    src/** is named twice"; exit 1; }
[ "$(grep -cxF -- '  - README.md' .ai/repo/scope.yaml)" = 1 ] || { echo "    README.md is named twice"; exit 1; }
grep -Eq '^  - dist' .ai/repo/scope.yaml && { echo "    a directory out.paths excludes was added to in"; exit 1; }
grep -q 'odd dir' .ai/repo/scope.yaml && { echo "    a name the glob subset cannot carry plainly was guessed at"; exit 1; }
# the additions sit inside `in`, and the rest of the file is the skeleton's
awk '/^in:/ { i = 1 } /^out:/ { i = 0 } i && /scripts\/\*\*/ { f = 1 } END { exit !f }' .ai/repo/scope.yaml \
  || { echo "    the additions are not inside in:"; exit 1; }
grep -v -e '^  - package.json$' -e '^  - scripts/\*\*$' -e '^  - .github/\*\*$' -e '^  # found tracked at the root' -e '^  # wins over every line here' .ai/repo/scope.yaml \
  | cmp -s - "$SKEL" || { echo "    init changed more of the scope than the lines it added"; exit 1; }
# every guess is said, with the key that changes it
grep -Eq 'note: \.ai/repo/scope\.yaml names 3 path\(s\) found tracked at the root of this repository: \.github/\*\* package\.json scripts/\*\*$' "$ERR" \
  || { echo "    the additions were not reported"; cat "$ERR"; exit 1; }
grep -Eq 'note: .* leaves dist/ out although 1 file\(s\) under it are tracked \(out\.paths\)' "$ERR" \
  || { echo "    the excluded committed directory was not reported"; cat "$ERR"; exit 1; }
grep -Eq 'note: .* 1 file\(s\) over out\.max_bytes 1048576 are tracked and out, the largest dist/index\.html \(1200000 bytes\)' "$ERR" \
  || { echo "    the file over the limit was not reported"; cat "$ERR"; exit 1; }
grep -Eq 'note: .* 1 tracked root entr\(ies\) with a name outside ' "$ERR" \
  || { echo "    the name that was not guessed was not reported"; cat "$ERR"; exit 1; }
# and the declaration it wrote is one doctor reads: no unknown key, no malformed list
out="$("$MJ" doctor 2>&1 || true)"
grep -Eq 'FAIL layout +\.ai/repo/scope\.yaml' <<<"$out" && { echo "    doctor refuses the seeded scope"; grep scope <<<"$out"; exit 1; }
n_skel="$(awk '/^in:/ { i = 1; next } /^[a-z]/ { i = 0 } i && /^  - / { n++ } END { print n }' "$SKEL")"
grep -Eq "OK +layout +\.ai/repo/scope\.yaml .* version 1; $((n_skel + 3)) in pathspec" <<<"$out" \
  || { echo "    doctor did not count the seeded pathspecs"; grep scope <<<"$out"; exit 1; }

# --extend leaves a scope that exists alone, whatever the tree has become
echo x > go.mod; git add -A >/dev/null; git commit -qm more
cp .ai/repo/scope.yaml "$S/before"
"$MJ" init --extend > /dev/null 2> "$ERR"
cmp -s .ai/repo/scope.yaml "$S/before" || { echo "    init --extend rewrote an existing scope"; exit 1; }
grep -q 'found tracked' "$ERR" && { echo "    init --extend reported guesses it did not write"; exit 1; }

# ---------------------------------------------------------------- an Elixir repository gets its own
repo ex
mkdir -p apps/core/lib priv/repo config
echo x > mix.exs; echo x > mix.lock; echo x > apps/core/lib/a.ex; echo x > priv/repo/seeds.exs; echo x > config/config.exs
git add -A >/dev/null; git commit -qm base
"$MJ" init > /dev/null 2> "$ERR"
for want in 'mix.exs' 'mix.lock' 'apps/**' 'priv/**'; do
  grep -qxF -- "  - $want" .ai/repo/scope.yaml || { echo "    the Elixir repository's scope does not name $want"; exit 1; }
done
grep -Eq '^  - (package\.json|scripts/)' .ai/repo/scope.yaml && { echo "    the Elixir repository was given the JavaScript one's paths"; exit 1; }
[ "$(grep -cxF -- '  - config/**' .ai/repo/scope.yaml)" = 1 ] || { echo "    config/** is named twice"; exit 1; }

# ---------------------------------------------------------------- nothing to find: the skeleton, and no word
repo empty
echo x > README.md; git add -A >/dev/null; git commit -qm base
"$MJ" init > /dev/null 2> "$ERR"
cmp -s .ai/repo/scope.yaml "$SKEL" || { echo "    a tree the skeleton already covers was not given the skeleton"; exit 1; }
grep -q 'scope.yaml' "$ERR" && { echo "    init reported a guess where it made none"; cat "$ERR"; exit 1; }
cd "$HERE"
exit 0

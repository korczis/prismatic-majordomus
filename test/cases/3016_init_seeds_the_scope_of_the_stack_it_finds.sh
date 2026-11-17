# majordomus-covers: none
# `majordomus init` seeds a scope that names what the adopter's tree holds (I1989): the
# entries every repository shares, and a stack's block only when that stack's marker is at
# the root. An empty repository and a JavaScript one get no Elixir or Rust path, a Rust
# repository gets Cargo.toml and Cargo.lock and nothing of Elixir, an Elixir repository
# gets mix.exs and priv/ and nothing of Rust, and init says what it found. The skeleton
# itself, the distribution's default for a manifest that names no scope, keeps every block.
# Python (pyproject.toml), Node (package.json) and Go (go.mod) are stacks too (I4252): an
# adopter's Python repository was told no stack was recognised and got no virtual-environment
# or cache exclusion. A stack's blocks in several sections are looked for, and named, once.
. "$ROOT/test/lib.sh"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj3016.XXXXXX")"; trap 'rm -rf "$S"' EXIT
SKEL="$ROOT/share/skeleton/ai/repo/scope.yaml"

# the distribution's default still names both stacks, so a layer without a scope section
# reads what it read before
for p in mix.exs mix.lock 'priv/\*\*/\*.exs' 'apps/\*\*/lib/\*\*' Cargo.toml Cargo.lock; do
  grep -q "^  - $p\$" "$SKEL" || { echo "    the skeleton lost its entry $p"; exit 1; }
done

ELIXIR='^  - (mix\.exs|mix\.lock|priv/|apps/\*\*/lib/)'
PYTHON='^ +- (pyproject\.toml|uv\.lock|'"'"'\*\*/\.venv/'"'"')$'
NODE='^  - (package\.json|tsconfig\*\.json)$'
GO='^  - (go\.mod|go\.sum)$'
RUST='^  - (Cargo\.toml|Cargo\.lock)$'
SHARED='^  - (\.ai/repo/\*\*|src/\*\*|docs/\*\*|README\.md|AGENTS\.md)$'

# fresh <name> [<file>...]: a new repository holding those files at its root, initialised;
# init's output is left in $S/<name>.out
fresh() {
  local name="$1" f; shift
  mkdir -p "$S/$name"
  git -C "$S/$name" init -q .
  for f in "$@"; do printf 'x\n' > "$S/$name/$f"; done
  ( cd "$S/$name" && "$MJ" init > "$S/$name.out" 2>&1 ) \
    || { echo "    init failed in the $name repository:"; cat "$S/$name.out"; exit 1; }
  [ -f "$S/$name/.ai/repo/scope.yaml" ] || { echo "    init seeded no scope in the $name repository"; exit 1; }
  grep -cE "$SHARED" "$S/$name/.ai/repo/scope.yaml" | grep -qx 5 \
    || { echo "    the $name repository's scope lost an entry every repository shares"; exit 1; }
}
has()  { grep -qE "$2" "$S/$1/.ai/repo/scope.yaml" || { echo "    the $1 repository's scope has no line matching $2:"; cat "$S/$1/.ai/repo/scope.yaml"; exit 1; }; }
lacks() { if grep -qE "$2" "$S/$1/.ai/repo/scope.yaml"; then echo "    the $1 repository's scope names a path of a stack it does not have:"; grep -E "$2" "$S/$1/.ai/repo/scope.yaml"; exit 1; fi; }

# --- no stack at all: the shared entries, and init says it recognised nothing
fresh empty
lacks empty "$ELIXIR"; lacks empty "$RUST"; lacks empty '^  # stack:'
grep -q '^scope: no stack recognised at the root (looked for mix.exs, Cargo.toml, pyproject.toml, package.json, go.mod);' "$S/empty.out" \
  || { echo "    init did not say it recognised no stack:"; cat "$S/empty.out"; exit 1; }

# --- a JavaScript repository: no Elixir or Rust path (the issue's acceptance criterion)
fresh javascript package.json
lacks javascript "$ELIXIR"; lacks javascript "$RUST"; lacks javascript "$PYTHON"; lacks javascript "$GO"
has javascript '^  - package\.json$'; has javascript "^    - '\*\*/\.next/'$"

# --- Python: its project files in, its environments and caches out, and nothing else
fresh python pyproject.toml
has python '^  - pyproject\.toml$'; has python '^  - uv\.lock$'
has python "^    - '\*\*/\.venv/'$"; has python "^    - '\*\*/__pycache__/'$"
lacks python "$ELIXIR"; lacks python "$RUST"; lacks python "$NODE"; lacks python "$GO"
grep -q '^scope: seeded for the stack found at the root: python (pyproject.toml);' "$S/python.out" \
  || { echo "    init did not say it found the Python stack, once:"; cat "$S/python.out"; exit 1; }

# --- Go: its module files and the conventional trees
fresh go go.mod
has go '^  - go\.mod$'; has go '^  - internal/\*\*$'; lacks go "$PYTHON"; lacks go "$NODE"

# --- Rust: its two files, and nothing of Elixir
fresh rust Cargo.toml
has rust '^  - Cargo\.toml$'; has rust '^  - Cargo\.lock$'; lacks rust "$ELIXIR"
grep -q '^scope: seeded for the stack found at the root: rust (Cargo.toml)' "$S/rust.out" \
  || { echo "    init did not say it found the Rust stack:"; cat "$S/rust.out"; exit 1; }

# --- Elixir: mix, priv/ and an umbrella's apps, and nothing of Rust
fresh elixir mix.exs
has elixir '^  - mix\.exs$'; has elixir '^  - priv/\*\*/\*\.exs$'; has elixir '^  - apps/\*\*/lib/\*\*$'
lacks elixir "$RUST"

# --- both: both blocks, each exactly as the skeleton has it
fresh both mix.exs Cargo.toml
sed -n '/^  # stack: elixir/,/^  # end stack: elixir/p; /^  # stack: rust/,/^  # end stack: rust/p' "$SKEL" > "$S/blocks.expected"
sed -n '/^  # stack: /,/^  # end stack: /p' "$S/both/.ai/repo/scope.yaml" > "$S/blocks.actual"
cmp -s "$S/blocks.expected" "$S/blocks.actual" || { echo "    the seeded stack blocks differ from the skeleton's:"; diff "$S/blocks.expected" "$S/blocks.actual"; exit 1; }

# every seeded scope is one the executable reads, when there is one to ask
if RB="$(rust_bin 2>/dev/null)" && [ -x "$RB" ]; then
  for name in empty rust elixir both python javascript go; do
    ( cd "$S/$name" && MAJORDOMUS_SHARE="$ROOT/share" "$RB" scope > "$S/$name.scope" 2>&1 ) \
      || { echo "    the executable refused the $name repository's seeded scope:"; cat "$S/$name.scope"; exit 1; }
  done
fi
exit 0

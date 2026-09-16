# majordomus-covers: none
# The Rust crate did not grow network code, and something says so.
#
# ADR 0032 draws a boundary around `majordomus-cli`: the code that reaches an external
# workspace does not enter the crate — "not an async runtime, not an HTTP client, not TLS,
# not a CDP client" — and its consequences say plainly that "a gate must assert that
# majordomus-cli's dependency list did not grow, or the boundary is a sentence rather than
# a constraint". `scripts/ci/crate-boundary` is that gate and this is what holds it.
#
# Every assertion below runs against a fixture tree, never against this checkout's own
# manifest: the gate takes MJ_ROOT, so a case can plant a forbidden dependency without
# editing a file the rest of the suite is reading at the same time.
#
# What is pinned:
#
#   1. the checkout passes — the boundary holds today, and a gate that cannot say so about
#      a clean tree is measuring something else
#   2. a forbidden crate reached through the lock is refused, by name and category — the
#      feature-flag path, which the manifest alone cannot see
#   3. a dependency the allow-list does not know is refused — the "did not grow" half
#   4. both spellings of a declaration are measured: `foo = "1"` and `[dependencies.foo]`,
#      and a target-specific table too
#   5. a gate that cannot reach its subject exits 12 rather than passing — an absent
#      manifest is not an absence of forbidden dependencies
. "$ROOT/test/lib.sh"

GATE="$ROOT/scripts/ci/crate-boundary"
expect_file "$GATE"
CRATE="$ROOT/apps/majordomus-cli"
expect_file "$CRATE/Cargo.toml"
expect_file "$CRATE/Cargo.lock"

# A fixture tree is the two files the gate reads, and nothing else.
fixture() { # -> prints a fresh tree's root
  local d="$T/fix.$1"
  mkdir -p "$d/apps/majordomus-cli"
  cp "$CRATE/Cargo.toml" "$CRATE/Cargo.lock" "$d/apps/majordomus-cli/"
  printf '%s' "$d"
}

# ------------------------------------------------------------------ 1. the tree passes
expect_exit 0 env MJ_ROOT="$ROOT" "$GATE"
expect_grep 'links no async runtime, HTTP client, TLS stack or CDP client'

# ------------------------------------------------------------------ 2. the lock is read
# `jsonschema` is taken with `default-features = false` because "nothing may resolve over
# the network"; turning that back on pulls an HTTP client in without touching the
# dependency list. The lock is where that shows, so the lock is what this asserts on.
d="$(fixture lock)"
printf '\n[[package]]\nname = "reqwest"\nversion = "0.12.0"\n' >> "$d/apps/majordomus-cli/Cargo.lock"
expect_exit 10 env MJ_ROOT="$d" "$GATE"
expect_grep 'reqwest is an HTTP client'
expect_grep 'ADR 0032'

d="$(fixture lock2)"
printf '\n[[package]]\nname = "tokio"\nversion = "1.0.0"\n' >> "$d/apps/majordomus-cli/Cargo.lock"
expect_exit 10 env MJ_ROOT="$d" "$GATE"
expect_grep 'tokio is an async runtime'

d="$(fixture lock3)"
printf '\n[[package]]\nname = "rustls"\nversion = "0.23.0"\n' >> "$d/apps/majordomus-cli/Cargo.lock"
expect_exit 10 env MJ_ROOT="$d" "$GATE"
expect_grep 'rustls is a TLS stack'

d="$(fixture lock4)"
printf '\n[[package]]\nname = "chromiumoxide"\nversion = "0.5.0"\n' >> "$d/apps/majordomus-cli/Cargo.lock"
expect_exit 10 env MJ_ROOT="$d" "$GATE"
expect_grep 'chromiumoxide is a browser/CDP client'

# A compound name belongs to the category it is, not to the category its prefix names:
# `tokio-rustls` is a TLS stack and `tokio-tungstenite` a WebSocket client, and both would
# read as "an async runtime" if the `tokio-*` glob were matched first. The refusal is what
# a reader acts on, so the wrong category is a wrong refusal.
d="$(fixture lock5)"
printf '\n[[package]]\nname = "tokio-rustls"\nversion = "0.26.0"\n' >> "$d/apps/majordomus-cli/Cargo.lock"
expect_exit 10 env MJ_ROOT="$d" "$GATE"
expect_grep 'tokio-rustls is a TLS stack'

d="$(fixture lock6)"
printf '\n[[package]]\nname = "tokio-tungstenite"\nversion = "0.24.0"\n' >> "$d/apps/majordomus-cli/Cargo.lock"
expect_exit 10 env MJ_ROOT="$d" "$GATE"
expect_grep 'tokio-tungstenite is a browser/CDP client'

# ------------------------------------------------------------------ 3. the list did not grow
# A dependency nobody argued for, in the plain spelling, inside the real table.
d="$(fixture grew)"
awk '{ print } /^\[dependencies\]$/ { print "some_new_crate = \"1\"" }' \
  "$CRATE/Cargo.toml" > "$d/apps/majordomus-cli/Cargo.toml"
expect_exit 10 env MJ_ROOT="$d" "$GATE"
expect_grep 'some_new_crate is a dependency this gate was never told about'
expect_grep 'add some_new_crate to ALLOWED'

# ------------------------------------------------------------------ 4. every spelling is measured
# `[dependencies.foo]` declares foo as surely as `foo = "1"` does, and a table that names a
# target is still a dependency table. A gate that reads one spelling and not the others is
# a gate with a documented way around it.
d="$(fixture subtable)"
printf '\n[dependencies.serde_yaml]\nversion = "0.9"\n' >> "$d/apps/majordomus-cli/Cargo.toml"
expect_exit 10 env MJ_ROOT="$d" "$GATE"
expect_grep 'serde_yaml is a dependency this gate was never told about'

d="$(fixture target)"
printf '\n[target."cfg(unix)".dependencies]\nhyper = "1"\n' >> "$d/apps/majordomus-cli/Cargo.toml"
expect_exit 10 env MJ_ROOT="$d" "$GATE"
expect_grep 'hyper is a dependency this gate was never told about'

# A dev-dependency is held to the same list: a CDP client in the test harness is still the
# thing ADR 0032 rejected, reimplemented in the language with the strictest budget.
d="$(fixture dev)"
awk '{ print } /^\[dev-dependencies\]$/ { print "fantoccini = \"0.21\"" }' \
  "$CRATE/Cargo.toml" > "$d/apps/majordomus-cli/Cargo.toml"
expect_exit 10 env MJ_ROOT="$d" "$GATE"
expect_grep 'fantoccini is a dependency this gate was never told about'

# ------------------------------------------------------------------ 5. it refuses what it cannot measure
# The failure this guards against is the quiet one: a gate pointed at a tree with no crate
# in it, reporting a clean boundary because it found no forbidden dependency in no file.
d="$T/empty"
mkdir -p "$d"
expect_exit 12 env MJ_ROOT="$d" "$GATE"
expect_grep 'cannot measure'

# ------------------------------------------------------------------ 6. the blessed neighbours stay blessed
# Two dependencies sit one word away from the forbidden set and are not in it. If a later
# edit widens the categories carelessly, this is what says so before the gate starts
# refusing the tree it was written to protect.
expect_grep '^tiny_http = ' "$CRATE/Cargo.toml"
expect_grep '^ed25519-dalek = ' "$CRATE/Cargo.toml"
expect_exit 0 env MJ_ROOT="$ROOT" "$GATE"

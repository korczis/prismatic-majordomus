# majordomus-exclusive: builds nothing, but reads and briefly edits this checkout's Cargo.toml
# majordomus-covers: none
# An executable of another generation may not derive this tree.
#
# `generate` projects the executable's own model onto the repository's committed artifacts.
# The model is compiled in, so a binary built from another revision of the crate rewrites
# all of them from a model the tree does not have — and nothing about the bytes says so, so
# the damage reads as an ordinary regeneration. It happened twice on 2026-09-10: a binary
# that called itself the same version as the tree, handed to `scripts/derive` through
# MAJORDOMUS_BIN, reported eleven artifacts stale on a pristine `origin/master` with the
# remedy printed as `run: majordomus generate`, and rewriting them that way removed 16,625
# lines — 9,724 of them from `docs/generated/changelog.json` — at exit 0.
#
# A version string is not a generation. This case pins what replaced it:
#
#   1. the version still has to agree, because it is stamped into every artifact
#   2. an executable built from the tree's own crate sources is that tree's executable
#   3. one built elsewhere is still accepted when it projects the registry the tree has
#      committed — that is what keeps a correctly-matching prebuilt MAJORDOMUS_BIN working
#   4. anything else is refused, with exit 15 and a verdict about the executable
#
# The verdict matters as much as the refusal. `CONTRACT_UNMET` (10) is what a genuinely
# stale artifact reports and its remedy is `scripts/derive`; running that with the
# executable that just refused is the destructive move. So the refusal is `REFUSED` (15),
# and it must not tell the operator that the tree is stale.
. "$ROOT/test/lib.sh"

BIN="$ROOT/apps/majordomus-cli/target/release/majordomus"
[ -x "$BIN" ] || BIN="${CARGO_TARGET_DIR:-$ROOT/apps/majordomus-cli/target}/debug/majordomus"
[ -n "${MAJORDOMUS_BIN:-}" ] && BIN="$MAJORDOMUS_BIN"
[ -x "$BIN" ] || { echo "    no built executable; skipping"; exit 0; }
CRATE="$ROOT/apps/majordomus-cli"
MANIFEST="$CRATE/Cargo.toml"
expect_file "$MANIFEST"
REGISTRY="$ROOT/docs/generated/registry.json"
expect_file "$REGISTRY"

running="$("$BIN" version 2>/dev/null | tr -d ' \n' | sed 's/^majordomus//')"
declared="$(sed -n '/^\[package\]/,/^\[/p' "$MANIFEST" | sed -n 's/^version = "\(.*\)"/\1/p' | head -1)"
[ -n "$declared" ] || { echo "    the manifest declares no version"; exit 1; }

# ---------------------------------------------------------------- 1. the ordinary path
# The tree this executable belongs to: it must not refuse it for any of the reasons below.
out="$(cd "$ROOT" && "$BIN" generate --check --repo . 2>&1)" || true
printf '%s' "$out" | grep -q 'this executable is' \
  && { echo "    generate refused the checkout it was built from ($running vs $declared)"; exit 1; }

# ---------------------------------------------------------------- 2. the version disagrees
# Still the cheapest question, and still asked first: the version is stamped into every
# artifact's provenance header, so a mismatch restamps all of them whatever the model says.
cp "$MANIFEST" "$T/Cargo.toml.bak"
restore() { cp "$T/Cargo.toml.bak" "$MANIFEST"; }
trap restore EXIT
sed -i.tmp "s/^version = \"$declared\"\$/version = \"9999.0.0\"/" "$MANIFEST" && rm -f "$MANIFEST.tmp"
grep -q '^version = "9999.0.0"' "$MANIFEST" || { echo "    could not stage the disagreement"; exit 1; }

set +e
(cd "$ROOT" && "$BIN" generate --check --repo . >"$T/out" 2>&1)
rc=$?
set -e
restore
trap - EXIT

[ "$rc" = 15 ] || { echo "    expected exit 15 (REFUSED) from a version mismatch, got $rc"; sed -n '1,5p' "$T/out"; exit 1; }
grep -q 'this executable is' "$T/out" || { echo "    the refusal does not say which executable is running"; exit 1; }
grep -q '9999.0.0' "$T/out" || { echo "    the refusal does not name the version the tree declares"; exit 1; }
grep -q 'scripts/derive' "$T/out" || { echo "    the refusal does not name the command that fixes it"; exit 1; }
grep -q 'The tree is not stale' "$T/out" || { echo "    the refusal does not say which side is out of date"; exit 1; }

# ---------------------------------------------------------------- a foreign tree, twice
# The remaining rules are asked of repositories built here rather than of this checkout: the
# guard reads the crate of whatever `--repo` names, so a fixture can hold a crate that
# declares this executable and is not the one it was built from. Nothing below writes into
# the checkout.
fixture() {                       # <dir> — a repository whose crate declares this executable
  mkdir -p "$1/apps/majordomus-cli/src" "$1/docs/generated"
  printf '[package]\nname = "majordomus-cli"\nversion = "%s"\nedition = "2021"\n' "$declared" \
    > "$1/apps/majordomus-cli/Cargo.toml"
  printf '// not the sources this executable was built from\npub fn f() {}\n' \
    > "$1/apps/majordomus-cli/src/lib.rs"
  # a real repository, because the guard is asked after the layer loads: a directory without
  # .ai/ never reaches it and would answer 12 (MISSING_ARTIFACT) to every question below
  (cd "$1" && git init -q . && git config user.email t@example.com && git config user.name t \
    && git commit -q --allow-empty -m init && "$MJ" init >/dev/null)
}
generate_in() {                   # <dir> -> exit code on stdout, output in $T/out
  gi_rc=0
  (cd "$1" && MAJORDOMUS_SHARE="$ROOT/share" "$BIN" generate --check --repo . >"$T/out" 2>&1) || gi_rc=$?
  printf '%s' "$gi_rc"
}

# --- 3. another crate, the same model: accepted
# A build from a sibling worktree whose crate differs by a comment produces the same
# artifacts, and refusing it would send the next session back to the ten-minute rebuild this
# guard exists to make safe. The registry the tree has committed is what says the models
# agree, so the fixture carries this checkout's.
fixture "$T/same-model"
cp "$REGISTRY" "$T/same-model/docs/generated/registry.json"
rc="$(generate_in "$T/same-model")"
[ "$rc" = 15 ] && {
  echo "    refused an executable that projects exactly the registry the tree committed:"
  sed 's/^/    | /' "$T/out"; exit 1; }
grep -q "not of this tree's generation" "$T/out" \
  && { echo "    a matching model was reported as a foreign generation"; exit 1; }

# --- 4. another crate and another model: refused
# One member of the model altered is enough. If the comparison were about the file being
# present, or about a version, this would pass and the derivation would proceed.
jq '.modules[0].title = "a model this executable does not have"' "$REGISTRY" \
  > "$T/altered.json" || { echo "    could not alter the committed registry"; exit 1; }
fixture "$T/other-model"
cp "$T/altered.json" "$T/other-model/docs/generated/registry.json"
rc="$(generate_in "$T/other-model")"
[ "$rc" = 15 ] || { echo "    expected exit 15 from a foreign generation, got $rc"; sed 's/^/    | /' "$T/out"; exit 1; }

grep -q "not of this tree's generation" "$T/out" \
  || { echo "    the refusal does not say the executable is of another generation"; exit 1; }
grep -q 'The tree is not stale' "$T/out" \
  || { echo "    the refusal does not say which side is out of date"; exit 1; }
grep -qF "$BIN" "$T/out" || { echo "    the refusal does not name the executable that was run"; exit 1; }
# both sides named: two generations, not one
[ "$(grep -c 'generation ' "$T/out")" -ge 2 ] \
  || { echo "    the refusal names fewer than two generations"; sed 's/^/    | /' "$T/out"; exit 1; }
grep -q 'cargo build' "$T/out" || { echo "    the refusal does not name the rebuild"; exit 1; }
grep -q 'scripts/derive' "$T/out" || { echo "    the refusal does not name scripts/derive"; exit 1; }
# and it must never print the remedy that destroys the artifacts as the thing to do
grep -q 'run: majordomus generate' "$T/out" \
  && { echo "    the refusal prints the destructive remedy"; exit 1; }
# nothing was written: a refusal that had already rewritten an artifact would be no guard
[ "$(sha256_of_file "$T/other-model/docs/generated/registry.json")" = "$(sha256_of_file "$T/altered.json")" ] \
  || { echo "    the refused run wrote into the tree anyway"; exit 1; }

# ---------------------------------------------------------------- the verdict is not "stale"
# scripts/derive-check is where the two verdicts are told apart, and where the wrong one
# would put `scripts/derive` in front of an operator whose executable is out of date.
grep -q '"\$grc" = 15' "$ROOT/scripts/derive-check" \
  || { echo "    scripts/derive-check does not tell a refusal from a stale artifact"; exit 1; }
grep -q '"\$rc" = 15' "$ROOT/scripts/derive" \
  || { echo "    scripts/derive says nothing about a refusal"; exit 1; }

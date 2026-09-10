# majordomus-exclusive: builds nothing, but reads and briefly edits this checkout's Cargo.toml
# majordomus-covers: none
# A stale executable may not restamp the tree it was not built from.
#
# `generate` writes a provenance header into every artifact naming the version of the
# executable that produced it. That version is compiled in, so a binary left in target/ from
# an older revision rewrites all of them with a version the tree does not have — and nothing
# else about the bytes changes, so the damage reads as an ordinary regeneration. It happened
# here: a leftover 0.2.0 build restamped seventy-five generated files in a 0.3.1 checkout.
#
# The guard asks the question only where the repository declares this executable, so a
# released binary generating a foreign repository is unaffected. This case proves both: the
# refusal when the versions disagree, and the silence when they agree.
. "$ROOT/test/lib.sh"

BIN="$ROOT/apps/majordomus-cli/target/release/majordomus"
[ -x "$BIN" ] || { echo "    the release executable is not built; skipping"; exit 0; }
MANIFEST="$ROOT/apps/majordomus-cli/Cargo.toml"
expect_file "$MANIFEST"

running="$("$BIN" version 2>/dev/null | tr -d ' \n' | sed 's/^majordomus//')"
declared="$(sed -n '/^\[package\]/,/^\[/p' "$MANIFEST" | sed -n 's/^version = "\(.*\)"/\1/p' | head -1)"
[ -n "$declared" ] || { echo "    the manifest declares no version"; exit 1; }

# --- they agree today, so generate --check must not refuse for this reason
out="$(cd "$ROOT" && "$BIN" generate --check --repo . 2>&1)" || true
printf '%s' "$out" | grep -q 'this executable is' \
  && { echo "    generate refused although the versions agree ($running vs $declared)"; exit 1; }

# --- disagree, and it must refuse with exit 10 and name the remedy
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

[ "$rc" = 10 ] || { echo "    expected exit 10 from a stale executable, got $rc"; sed -n '1,5p' "$T/out"; exit 1; }
grep -q 'this executable is' "$T/out" || { echo "    the refusal does not say which executable is running"; exit 1; }
grep -q '9999.0.0' "$T/out" || { echo "    the refusal does not name the version the tree declares"; exit 1; }
grep -q 'scripts/derive' "$T/out" || { echo "    the refusal does not name the command that fixes it"; exit 1; }

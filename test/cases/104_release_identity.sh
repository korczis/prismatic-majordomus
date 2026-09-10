# A release has one identity and its version is earned. This case proves the invariants
# `project.release-identity` states, and it proves them by mutation: a version below the
# minimum its change implies must be refused, a breaking change with no migration document
# must fail the check, two records with one identity must fail, and a hand-edited
# `share/version.txt` must fail `generate --check`.
#
# The mutations are made in a copy of the layer under $T, never in the checkout: the suite
# runs cases in parallel and a case that edited the tree would make its neighbours fail.
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

# --- the four versions are four facts, and the command says all four --------------------
expect_exit 0 "$MJB" release --repo "$ROOT" version
expect_grep '^source '
expect_grep '^published '
expect_grep '^running '

# the source version is the crate's, and nothing else states one
crate="$(awk '/^\[package\]/{p=1;next} /^\[/{p=0} p && /^version *=/{gsub(/[^0-9A-Za-z.+-]/,"",$3); print $3; exit}' "$ROOT/apps/majordomus-cli/Cargo.toml")"
expect_grep "^source +$crate\$"

# --- the version the shell tool prints is generated, not written ------------------------
expect_file "$ROOT/share/version.txt"
expect_grep 'GENERATED FILE' "$ROOT/share/version.txt"
expect_grep "^version=$crate\$" "$ROOT/share/version.txt"
# and bin/majordomus states no version of its own: that literal is the duplicate this
# subsystem removed, and its return would be a silent second authority
expect_no_grep '^MJ_VERSION="[0-9]' "$ROOT/bin/majordomus"
# the two writers agree, which is what the pre-existing gate has always asked
expect_exit 0 "$ROOT/scripts/release-version" --check
# and the tool prints it
expect_exit 0 "$ROOT/bin/majordomus" version
expect_grep "^majordomus $crate\$"

# --- the committed contract is a contract ------------------------------------------------
snapshot="$ROOT/docs/generated/contract.json"
expect_file "$snapshot"
expect_grep '"schema": "contract/v1"' "$snapshot"
# it covers the surfaces a verdict needs; a snapshot missing one narrows every future diff
expect_grep '"surface": "capability"' "$snapshot"
expect_grep '"surface": "command"' "$snapshot"
expect_grep '"surface": "document-kind"' "$snapshot"
# and it carries no workflow: the runner is not on every machine, and a contract that
# differed between two machines reading one commit could never be a baseline
expect_no_grep '"id": "workflow\.' "$snapshot"

# --- every projection of the release state is current ------------------------------------
expect_exit 0 "$MJB" generate --repo "$ROOT" release --check

# --- the changelog is generated from the records and from nothing else --------------------
expect_file "$ROOT/CHANGELOG.md"
expect_grep 'GENERATED FILE' "$ROOT/CHANGELOG.md"
expect_grep '^# Changelog' "$ROOT/CHANGELOG.md"
expect_exit 0 "$MJB" release --repo "$ROOT" changelog
# the command prints the same document the file carries, less the generated banner: two
# renderings of one model, which is the whole claim
"$MJB" release --repo "$ROOT" changelog > "$T/printed.md" 2>/dev/null
sed '1,/-->/d' "$ROOT/CHANGELOG.md" | sed '/./,$!d' > "$T/committed.md"
sed '/./,$!d' "$T/printed.md" > "$T/printed.trimmed"
diff -u "$T/committed.md" "$T/printed.trimmed" > "$T/changelog.diff" 2>&1 \
  || { echo "    the printed changelog and the committed one differ:"; sed 's/^/    | /' "$T/changelog.diff"; exit 1; }

# --- planning writes nothing ---------------------------------------------------------------
before="$(sha256_of_file "$ROOT/apps/majordomus-cli/Cargo.toml")"
expect_exit 0 "$MJB" release --repo "$ROOT" plan
"$MJB" release --repo "$ROOT" prepare --dry-run >/dev/null 2>&1 || true
after="$(sha256_of_file "$ROOT/apps/majordomus-cli/Cargo.toml")"
[ "$before" = "$after" ] || { echo "    a dry run wrote to the manifest"; exit 1; }

# --- --version and --bump are two ways to say one thing, and both is refused ---------------
expect_exit 15 "$MJB" release --repo "$ROOT" prepare --version 9.9.9 --bump major --dry-run
expect_grep 'give one'

# --- a version below the minimum its change implies is refused ------------------------------
# The minimum comes from the engine rather than from a number written here, so this case
# stays true as the repository's own state moves.
minimum="$("$MJB" release --repo "$ROOT" status --format json 2>/dev/null \
  | sed -n 's/.*"minimum_version": "\([^"]*\)".*/\1/p' | head -1)"
if [ -n "$minimum" ]; then
  expect_exit 15 "$MJB" release --repo "$ROOT" prepare --version 0.0.1 --dry-run
  expect_grep 'refused'
  expect_grep "requires at least $minimum"
  # and the version the policy asks for is accepted by the same check
  expect_exit 0 "$MJB" release --repo "$ROOT" prepare --version "$minimum" --dry-run
  expect_grep 'nothing was written'
fi

# --- the layer's own mutations, in a copy ----------------------------------------------------
# A disposable repository with this one's layer in it: `git init`, the layer, and the two
# files the engine reads beside it. Copying the whole checkout would cost a gigabyte and
# prove nothing more.
S="$T/layer"
mkdir -p "$S"
git -C "$S" init -q 2>/dev/null || { git init -q "$S"; }
cp -R "$ROOT/.ai" "$S/.ai"
mkdir -p "$S/apps/majordomus-cli" "$S/docs/generated"
cp "$ROOT/apps/majordomus-cli/Cargo.toml" "$S/apps/majordomus-cli/Cargo.toml"
cp "$snapshot" "$S/docs/generated/contract.json"
rm -rf "$S/.ai/local"
git -C "$S" add -A >/dev/null 2>&1
git -C "$S" -c user.email=t@t -c user.name=t commit -qm layer >/dev/null 2>&1

# a breaking change with no migration document is refused
cat > "$S/.ai/repo/changes/breaks-something.md" <<'CHANGE'
---
schema: change/v1
id: breaks-something
kind: change
title: something a caller relied on is gone
type: removed
impact: breaking
---

## Summary

A fixture. It removes something and says nothing about how to live without it.
CHANGE
git -C "$S" add .ai/repo/changes/breaks-something.md >/dev/null 2>&1
expect_exit 10 "$MJB" release --repo "$S" check
expect_grep 'MIGRATION_GUIDE_REQUIRED'

# naming a migration document satisfies exactly that finding
awk '{print} /^impact: breaking$/{print "migration: docs/RELEASE.md"}' \
  "$S/.ai/repo/changes/breaks-something.md" > "$S/fixed.md"
mv "$S/fixed.md" "$S/.ai/repo/changes/breaks-something.md"
git -C "$S" add .ai/repo/changes/breaks-something.md >/dev/null 2>&1
"$MJB" release --repo "$S" check > "$T/after.txt" 2>&1 || true
grep -q 'MIGRATION_GUIDE_REQUIRED' "$T/after.txt" \
  && { echo "    naming a migration document did not satisfy the check:"; sed 's/^/    | /' "$T/after.txt"; exit 1; }

# and a breaking record makes the release breaking, which below 1.0.0 moves the minor
"$MJB" release --repo "$S" status > "$T/status.txt" 2>&1 || true
grep -qE '^compatibility +breaking' "$T/status.txt" \
  || { echo "    a breaking record did not make the release breaking:"; sed 's/^/    | /' "$T/status.txt"; exit 1; }
grep -qE '^required bump +minor' "$T/status.txt" \
  || { echo "    below 1.0.0 a break must move the minor:"; sed 's/^/    | /' "$T/status.txt"; exit 1; }

# two records with one identity make the layer degraded, and a release decided over a
# layer that did not read is not a decision
sed 's/^title: .*/title: the same identity, said twice/' \
  "$S/.ai/repo/changes/breaks-something.md" > "$S/.ai/repo/changes/breaks-again.md"
git -C "$S" add .ai/repo/changes/breaks-again.md >/dev/null 2>&1
expect_exit 10 "$MJB" release --repo "$S" check
expect_grep 'LAYER_DEGRADED'
rm -f "$S/.ai/repo/changes/breaks-again.md" "$S/.ai/repo/changes/breaks-something.md"

# --- a hand-edited generated version is refused ------------------------------------------
cp -R "$ROOT/share" "$S/share"
printf 'version=9.9.9\n' > "$S/share/version.txt"
# The agreement gate reads the tree it lives in, so the copy is checked through the
# executable's own projection check instead, which is the gate CI actually runs.
expect_exit 10 "$MJB" generate --repo "$S" release --check
expect_grep 'version.txt'

# --- the gate runs all of it and says so ----------------------------------------------------
expect_exit 0 env MJ_ROOT="$ROOT" "$ROOT/scripts/ci/release-check"
expect_grep 'every release invariant holds'

# --- one answer, every surface ---------------------------------------------------------------
# The command line executes the capability rather than reimplementing it, so a difference
# between what the registry declares and what the command answers would mean the command had
# grown an opinion of its own.
"$MJB" capabilities --repo "$ROOT" list --format json > "$T/caps.json" 2>/dev/null
for id in release.version release.status release.explain release.diff release.changelog \
          release.manifest release.plan release.check; do
  grep -q "\"$id\"" "$T/caps.json" || { echo "    the registry does not carry $id"; exit 1; }
done
"$MJB" release --repo "$ROOT" status --format json > "$T/cli.json" 2>/dev/null
grep -q '"target_version"' "$T/cli.json" || { echo "    release status --format json is not the model"; exit 1; }

# every one of them is on all three surfaces, which is the parity the rule asks for
for id in release.version release.status release.changelog release.check; do
  "$MJB" capabilities --repo "$ROOT" describe "$id" --format json > "$T/cap.json" 2>/dev/null
  grep -q '"tool"' "$T/cap.json" || { echo "    $id has no MCP tool"; exit 1; }
  grep -q '/api/v1/release/' "$T/cap.json" || { echo "    $id has no HTTP route"; exit 1; }
  grep -q '"cli"' "$T/cap.json" || { echo "    $id has no command"; exit 1; }
done

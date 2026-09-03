. "$ROOT/test/lib.sh"
# The regression test for the whole derived-site architecture: change a canonical value
# in a throwaway copy of the repository, regenerate, and prove the output changed.
# If editing canonical content does not change the site, the site is not derived.
cp -R "$ROOT" "$T/repo" 2>/dev/null || true
rm -rf "$T/repo/_site" "$T/repo/.git"
GEN="$T/repo/scripts/generate-pages"

expect_exit 0 bash "$GEN" --out "$T/before" --no-css
before_sha="$(sed -n 's/.*"input_sha256": "\([0-9a-f]*\)".*/\1/p' "$T/before/site-manifest.json")"

# 1. a profile description reaches the profiles page
grep -q 'reproduce, isolate, fix, and prove a defect fixed' "$T/before/profiles/index.html"
sed -i.bak 's/^description: .*/description: CANARY-PROFILE-DESCRIPTION/' \
  "$T/repo/share/skeleton/profiles/debugging.yaml"
rm -f "$T/repo/share/skeleton/profiles/debugging.yaml.bak"

# 2. the version reaches every page
sed -i.bak 's/^MJ_VERSION=.*/MJ_VERSION="9.9.9-canary"/' "$T/repo/bin/majordomus"
rm -f "$T/repo/bin/majordomus.bak"

# 3. a policy value reaches the profiles page
sed -i.bak 's/^  checkpoint_interval_default: .*/  checkpoint_interval_default: 47m/' \
  "$T/repo/share/skeleton/policy.yaml"
rm -f "$T/repo/share/skeleton/policy.yaml.bak"

# 4. a claim reaches the guarantees page
sed -i.bak 's/^    claim: Every finding carries the command that reproduces it$/    claim: CANARY-CLAIM-TEXT/' \
  "$T/repo/docs/CLAIMS.yaml"
rm -f "$T/repo/docs/CLAIMS.yaml.bak"

expect_exit 0 bash "$GEN" --out "$T/after" --no-css

expect_grep 'CANARY-PROFILE-DESCRIPTION' "$T/after/profiles/index.html"
expect_grep '9\.9\.9-canary' "$T/after/index.html"
expect_grep '47m' "$T/after/profiles/index.html"
expect_grep 'CANARY-CLAIM-TEXT' "$T/after/guarantees/index.html"

# the stale text is gone, not merely joined by the new text
expect_no_grep 'reproduce, isolate, fix, and prove a defect fixed' "$T/after/profiles/index.html"

# and the manifest records that the inputs changed
after_sha="$(sed -n 's/.*"input_sha256": "\([0-9a-f]*\)".*/\1/p' "$T/after/site-manifest.json")"
[ "$before_sha" != "$after_sha" ] \
  || { echo "    input_sha256 did not change after editing canonical sources"; exit 1; }

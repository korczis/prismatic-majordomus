# The site says a test proves a guarantee only as far as recorded evidence supports it (I2965).
# The guarantees index said "What Majordomus actually does today, row by row, with ... the test
# that proves it", docs/SITE_CLAIMS.md "the test that proves it", the challenge page "the test
# that proves the guarantee" and the homepage "From first prompt to proven done", while the
# published evidence supported 0 of 229 guaranteed claims. Held here:
#   1. homepage.toml [promises] declares "proven" and "proves", so scripts/ci/homepage-check
#      refuses either word in the hand-written copy unless a guaranteed claim with a recorded
#      run permits it;
#   2. while the evidence supports fewer claims than are declared, the generated guarantees
#      index, docs/SITE_CLAIMS.md and the site's data say a test is "to prove" a claim, never
#      that it "proves" one, and the homepage copy says no "proven".
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
DECL="$ROOT/site/data/homepage.toml"
PRODUCT="$ROOT/site/data/registry/product.json"
expect_file "$DECL"; expect_file "$PRODUCT"

# ---------------------------------------------------------------- 1. the words are declared promises
promises="$(awk '/^\[promises\]/ { on = 1; next } /^\[/ { on = 0 } on' "$DECL")"
for word in proven proves; do
  printf '%s\n' "$promises" | grep -qE "^\"$word\" = " \
    || { echo "    homepage.toml [promises] does not declare \"$word\""; exit 1; }
done

# ---------------------------------------------------------------- 2. no proof is said that the evidence does not hold
declared="$(jq -r '.evidence.declared // empty' "$PRODUCT")"
supported="$(jq -r '.evidence.supported // empty' "$PRODUCT")"
case "$declared$supported" in *[!0-9]*|'') echo "    product.json carries no evidence.declared/supported: '$declared' '$supported'"; exit 1 ;; esac
if [ "$supported" -lt "$declared" ]; then
  bad=0
  for f in "$ROOT/site/content/guarantees/_index.md" "$ROOT/docs/SITE_CLAIMS.md" "$ROOT"/site/data/*.toml; do
    [ -f "$f" ] || continue
    hits="$(grep -nE 'the test that proves (it|the guarantee)' "$f" || true)"
    if [ -n "$hits" ]; then
      printf '    %s says a test proves a claim while %s of %s are supported:\n' "${f#"$ROOT"/}" "$supported" "$declared"
      printf '%s\n' "$hits" | sed 's/^/      /'
      bad=1
    fi
  done
  for f in "$ROOT/site/data/marketing.toml" "$ROOT/site/data/manifesto.toml"; do
    hits="$(grep -vE '^[[:space:]]*#' "$f" | grep -niwE 'proven|proves' || true)"
    if [ -n "$hits" ]; then
      printf '    %s says proven or proves while %s of %s are supported:\n' "${f#"$ROOT"/}" "$supported" "$declared"
      printf '%s\n' "$hits" | sed 's/^/      /'
      bad=1
    fi
  done
  [ "$bad" = 0 ] || exit 1
fi
exit 0

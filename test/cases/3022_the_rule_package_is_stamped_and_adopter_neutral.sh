# majordomus-covers: none
# The vendored rule package says nothing that is only true of this repository, and is
# stamped with the release that produced it (I1986, evidence neutral_rule_text and
# source_revision_stamped).
#
# An adopter reads the package's source_revision in `rules vendor status` to tell one package
# from another, and the label was wrong at every release it was asked of: the tag v0.23.0
# shipped a package labelled 0.6.0, and v0.24.1 and v0.25.0 one labelled 0.23.0, because
# scripts/rules-package stamped what bin/majordomus printed — share/version.txt, a projection
# that lags a bump — and nothing ran it at a release. And the package's rules told incidents
# of the Majordomus repository as "this repository", which in an adopter's vendored copy
# names the adopter. Held here:
#
#   1  `scripts/rules-package write` stamps the version the crate manifest declares, not the
#      projection: in a fixture whose projection still names an older release, the stamp is
#      the declared one
#   2  `check` refuses a stamp that is not the release the tree produces, naming both, and a
#      write makes it pass — so a bump that left the package alone is a finding
#   3  this checkout's package is stamped with the release this checkout produces
#   4  no rule in the package says "this repository", and every paragraph that tells a dated
#      or measured incident attributes it to the Majordomus repository
. "$ROOT/test/lib.sh"

# ---------------------------------------------------------------- the fixture
F="$T/dist"
mkdir -p "$F/scripts" "$F/apps/majordomus-cli"
cp -R "$ROOT/bin" "$ROOT/lib" "$ROOT/share" "$F/"
cp "$ROOT/scripts/rules-package" "$ROOT/scripts/release-version" "$F/scripts/"
cp "$ROOT/apps/majordomus-cli/Cargo.toml" "$F/apps/majordomus-cli/"
declared="$("$F/scripts/release-version")"
[ -n "$declared" ] || { echo "    the fixture declares no version"; exit 1; }
# the projection lags, as it does between `release bump` and the derive
printf 'version=0.0.1\n' > "$F/share/version.txt"
printed="$(env -u MAJORDOMUS_SHARE "$F/bin/majordomus" version)"
[ "$printed" = "majordomus 0.0.1" ] \
  || { echo "    the fixture's shell tool does not print its lagging projection: '$printed'"; exit 1; }
stamp() { sed -n 's/^source_revision: *//p' "$F/share/standard/majordomus/manifest.yaml"; }
pkg() { env -u MAJORDOMUS_SHARE "$F/scripts/rules-package" "$@"; }

# ---------------------------------------------------------------- 1. write stamps the declared release
expect_exit 0 pkg write || exit 1
[ "$(stamp)" = "$declared" ] \
  || { echo "    rules-package write stamped '$(stamp)'; the crate declares $declared (the projection said 0.0.1)"; exit 1; }
expect_exit 0 pkg check || { echo "    a freshly written package was refused"; exit 1; }
expect_grep "\\(1 \\($declared\\)\\)" || { echo "    check does not name the stamp it read"; exit 1; }

# ---------------------------------------------------------------- 2. a bump that left the package alone
sed -i.bak "s/^version = \"$declared\"/version = \"9.8.7\"/" "$F/apps/majordomus-cli/Cargo.toml"
[ "$("$F/scripts/release-version")" = 9.8.7 ] || { echo "    the fixture's bump did not take"; exit 1; }
expect_exit 10 pkg check || { echo "    a package stamped with the previous release was accepted"; exit 1; }
expect_grep "source_revision is $declared and this tree produces 9\\.8\\.7" \
  || { echo "    the refusal does not name the stamp and the release"; exit 1; }
expect_exit 0 pkg write || exit 1
[ "$(stamp)" = 9.8.7 ] || { echo "    the restamp wrote '$(stamp)'"; exit 1; }
expect_exit 0 pkg check || { echo "    the restamped package was refused"; exit 1; }

# ---------------------------------------------------------------- 3. this checkout's package
mine="$(sed -n 's/^source_revision: *//p' "$ROOT/share/standard/majordomus/manifest.yaml")"
want="$("$ROOT/scripts/release-version")"
[ "$mine" = "$want" ] \
  || { echo "    share/standard/majordomus is stamped $mine and this checkout produces $want: run scripts/rules-package write"; exit 1; }

# ---------------------------------------------------------------- 4. the text is the adopter's
RULES="$ROOT/share/standard/majordomus/rules"
if grep -n -i 'this repository' "$RULES"/*.md > "$T/this-repo.txt"; then
  echo "    a vendored rule says \"this repository\", which in an adopter's copy names the adopter:"
  sed 's/^/      /' "$T/this-repo.txt"; exit 1
fi
# paragraph by paragraph, below the front matter: a dated or measured incident names whose it
# was. One program for every file; a file's last paragraph is judged when the next one starts.
UNATTRIBUTED='
  function judge() {
    if (para ~ /20[0-9][0-9]-[0-1][0-9]-[0-3][0-9]/ || para ~ /[Mm]easured (in|on),? /)
      if (para !~ /Majordomus repository/) printf "%s:%d\n", file, start
    para = ""
  }
  FNR == 1 { judge(); fm = 0; file = FILENAME }
  /^---$/ && fm < 2 { fm++; next }
  fm < 2 { next }
  /^[[:space:]]*$/ { judge(); next }
  { if (para == "") start = FNR; para = para " " $0 }
  END { judge() }
'
awk "$UNATTRIBUTED" "$RULES"/*.md > "$T/unattributed.txt"
if [ -s "$T/unattributed.txt" ]; then
  echo "    a vendored rule tells a dated or measured incident without saying it was the Majordomus repository's:"
  sed 's/^/      /' "$T/unattributed.txt"; exit 1
fi
# and the program can see one: the wording the package carried before is found
printf -- '---\nid: x\n---\n\nMeasured in this repository on 2026-09-11: 974 records.\n' > "$T/probe.md"
[ -n "$(awk "$UNATTRIBUTED" "$T/probe.md")" ] \
  || { echo "    the paragraph check cannot see an unattributed incident"; exit 1; }

echo "    rules-package write stamps the declared release, not the lagging projection; check refuses a stale stamp; this checkout's package is stamped $want; no vendored rule calls an upstream incident \"this repository\""

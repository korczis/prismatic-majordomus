# majordomus-covers: none
# The version this page describes and the version the advertised install delivers are compared,
# and a disagreement between them is disclosed on the page rather than left silent.
#
# The site was generated from a repository at 0.5.0 while the newest published release was
# v0.3.1, so `curl … install.sh | sh` delivered a build with roughly half the interfaces the
# homepage counted, and the page it came from said nothing about it. Three gates already touched
# this area and each checked one direction — installer-live proves the installer works and
# reports whatever the metadata resolved, version-matches-surface proves the declared version is
# at least what the surface implies, release-check refuses a tree BEHIND the published release.
# A tree far ahead of its last release is exactly what all three are built to permit. The
# converse — that the version the page advertises is one that was ever published — was owned by
# nothing, which is the one-way-check class this repository keeps meeting.
#
# The gate does not refuse the gap: a repository is allowed to be ahead of its last release, and
# refusing that would block every commit until someone cut one. It refuses the silence, in both
# directions, because a notice left standing after the versions meet is the same defect wearing
# the other face.
. "$ROOT/test/lib.sh"

[ -f "$ROOT/site/public/index.html" ] || { echo "    site/public is not built; run scripts/site-build"; exit 0; }

dist="$ROOT/site/data/registry/distribution.json"
[ -f "$dist" ] || { echo "    no site/data/registry/distribution.json; the released version cannot be read"; exit 1; }

released="$(jq -r '.latest.version // ""' "$dist")"
described="$(jq -r '.source_version // ""' "$ROOT/site/data/build.json" 2>/dev/null)"
[ -n "$released" ] || { echo "    the distribution model names no released version"; exit 1; }
[ -n "$described" ] || { echo "    site/data/build.json does not say which version the site was built from"; exit 1; }

out="$(cd "$ROOT" && scripts/site-check --no-sync 2>&1)" || true
line="$(printf '%s' "$out" | grep -E '^(OK|FAIL)[[:space:]]+version[[:space:]]' | head -1)"
[ -n "$line" ] || { echo "    site-check reports no verdict on the version the install delivers"; exit 1; }
case "$line" in OK*) ;; *) echo "    $line"; exit 1 ;; esac

# the verdict names both versions, so a reader of the log knows which two were compared
printf '%s' "$line" | grep -qF "$released" \
  || { echo "    the verdict does not name the released version $released: $line"; exit 1; }

notice='id="version-gap"'
if [ "$released" = "$described" ]; then
  grep -q "$notice" "$ROOT/site/public/index.html" \
    && { echo "    the versions agree and the homepage still carries the version-gap notice"; exit 1; }
else
  grep -q "$notice" "$ROOT/site/public/index.html" \
    || { echo "    the page describes $described, the install delivers $released, and the homepage does not say so"; exit 1; }
  # and it names the version a reader would actually receive, not merely that a gap exists
  grep -q "v$released" "$ROOT/site/public/index.html" \
    || { echo "    the notice does not name the version the install delivers ($released)"; exit 1; }
fi

# the negative half: remove the disclosure while the gap stands, and the gate must refuse.
# Skipped when the versions agree, because then there is no gap to leave undisclosed and a
# check that quietly passes on a fixture it could not build would prove nothing.
if [ "$released" != "$described" ]; then
  T2="$(mktemp -d)"; trap 'rm -rf "$T2"' EXIT
  cp "$ROOT/site/public/index.html" "$T2/index.html.orig"
  sed 's/id="version-gap"/id="version-gap-removed"/' "$T2/index.html.orig" > "$T2/stripped.html"
  cmp -s "$T2/index.html.orig" "$T2/stripped.html" \
    && { echo "    could not strip the notice; the negative half proves nothing"; exit 1; }
  cp "$T2/stripped.html" "$ROOT/site/public/index.html"
  out2="$(cd "$ROOT" && scripts/site-check --no-sync 2>&1)" || true
  cp "$T2/index.html.orig" "$ROOT/site/public/index.html"
  printf '%s' "$out2" | grep -q '^FAIL[[:space:]]*version' \
    || { echo "    the gap went undisclosed and the gate passed anyway"; printf '%s\n' "$out2" | grep -E '(OK|FAIL)[[:space:]]+version' | sed 's/^/    /'; exit 1; }
fi

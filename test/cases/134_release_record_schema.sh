# majordomus-covers: none
# A release record carrying every field the release writer emits is accepted by the schema that
# governs it, and the index stays clean under --strict.
#
# scripts/release-record writes `required_targets:`. share/schemas/majordomus/release/release.v1
# did not declare it and is additionalProperties:false, so the first record the writer produced
# would have been refused: `key(s) not in schema 'majordomus.release/v1': required_targets`,
# index state Degraded, `generate --strict` exit 10. The publish job runs scripts/derive AFTER
# `gh release create` has already succeeded — so the failure would have landed a real GitHub
# release, with working archives and digests, that no record could ever be committed for and
# that the installer's latest.json would never learn about. Nothing tied the writer to the
# schema, and no record had exercised the field because the writer postdates the only release
# ever cut. This case is that tie.
. "$ROOT/test/lib.sh"

schema="$ROOT/share/schemas/majordomus/release/release.v1.schema.json"
writer="$ROOT/scripts/release-record"
[ -f "$schema" ] || { echo "    no release record schema at share/schemas/majordomus/release/"; exit 1; }
[ -f "$writer" ] || { echo "    no scripts/release-record; this case is measuring the wrong tree"; exit 1; }

# Every top-level key the writer emits is a key the schema declares. Derived from the writer
# rather than listed here: a field added to the writer tomorrow is checked the day it appears,
# which is the property whose absence caused the defect.
emitted="$(grep -oE '^[[:space:]]*(printf|echo)[^|]*"[a-z_]+:' "$writer" 2>/dev/null | grep -oE '[a-z_]+:' | tr -d ':' | LC_ALL=C sort -u)"
[ -n "$emitted" ] || emitted="$(grep -oE '^\s*[a-z_]+:' "$writer" | tr -d ' :' | LC_ALL=C sort -u)"
[ -n "$emitted" ] || { echo "    could not read the keys scripts/release-record writes; the check cannot reach its subject"; exit 1; }

declared="$(jq -r '.properties | keys[]' "$schema")"
missing=""
for k in $emitted; do
  case "$k" in schema|version|tag|channel|commit|published_at|notes_url|yanked|artifacts|required_targets|target|name|url|sha256|size) ;; *) continue ;; esac
  printf '%s\n' "$declared" | grep -qx "$k" || missing="$missing $k"
done
[ -z "$missing" ] \
  || { echo "    scripts/release-record writes key(s) the schema does not declare:$missing"; exit 1; }

# the specific field the defect was about
printf '%s\n' "$declared" | grep -qx required_targets \
  || { echo "    the schema does not declare required_targets, which scripts/release-record writes"; exit 1; }

# and the schema is still closed — declaring the field must not have been done by opening the
# door to every field, which would have removed the guarantee instead of satisfying it
[ "$(jq -r '.additionalProperties' "$schema")" = "false" ] \
  || { echo "    the release schema is no longer additionalProperties:false; unknown keys would pass unnoticed"; exit 1; }

# a record carrying the field indexes cleanly. Built in a fixture, never in the real layer.
command -v cargo >/dev/null 2>&1 || [ -n "${MAJORDOMUS_BIN:-}" ] || { echo "    no cargo and no MAJORDOMUS_BIN; cannot index"; exit 0; }
fixture_repo "$T/repo"
mkdir -p "$T/repo/.ai/repo/releases"
cat > "$T/repo/.ai/repo/releases/v9.9.9.yaml" <<'YAML'
schema: release/v1
version: 9.9.9
tag: v9.9.9
channel: stable
commit: 0000000000000000000000000000000000000000
published_at: '2026-01-01T00:00:00Z'
required_targets:
  - aarch64-apple-darwin
artifacts:
  - target: aarch64-apple-darwin
    name: majordomus-v9.9.9-aarch64-apple-darwin.tar.gz
    url: https://example.invalid/majordomus-v9.9.9-aarch64-apple-darwin.tar.gz
    sha256: 0000000000000000000000000000000000000000000000000000000000000000
    size: 1
YAML
out="$(rust_bin generate --repo "$T/repo" --check --strict 2>&1)" || true
printf '%s' "$out" | grep -q "not in schema 'majordomus.release/v1'" \
  && { echo "    a record carrying required_targets is still refused by the schema:"; printf '%s\n' "$out" | grep -i 'schema' | sed 's/^/    /'; exit 1; }
printf '%s' "$out" | grep -qi 'required_targets' \
  && { echo "    required_targets is still reported as a problem:"; printf '%s\n' "$out" | grep -i required_targets | sed 's/^/    /'; exit 1; }

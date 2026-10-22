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

# Every key the writer emits is a key the schema declares, at the level the writer emits it.
# Both sides are read rather than listed: a field added to the writer tomorrow is checked the
# day it appears, which is the property whose absence caused the defect.
#
# The two levels are distinguished by how the writer emits them, because the schema
# distinguishes them too. A record's own keys are written by `printf 'key: ...'` with the key
# at the start of the format; an artifact's keys are written indented, into the $body the
# artifacts list is built from. Checking an artifact key against the record's properties is
# how the first version of this case failed against a schema that was already correct.
top="$(sed -n "s/^[[:space:]]*printf '\([a-z_][a-z_]*\):.*/\1/p" "$writer" | LC_ALL=C sort -u)"
nested="$(sed -n 's/^[[:space:]]\{2,\}\(-[[:space:]]*\)\{0,1\}\([a-z_][a-z_]*\):[[:space:]].*/\2/p' "$writer" | LC_ALL=C sort -u)"
[ -n "$top" ] || { echo "    could not read the record keys scripts/release-record writes; the check cannot reach its subject"; exit 1; }
[ -n "$nested" ] || { echo "    could not read the artifact keys scripts/release-record writes; the check cannot reach its subject"; exit 1; }

declared="$(jq -r '.properties | keys[]' "$schema")"
declared_artifact="$(jq -r '.properties.artifacts.items.properties | keys[]' "$schema")"
[ -n "$declared_artifact" ] || { echo "    the schema declares no artifact properties; this case is reading the wrong shape"; exit 1; }

missing=""
for k in $top; do
  printf '%s\n' "$declared" | grep -qx "$k" || missing="$missing $k"
done
[ -z "$missing" ] \
  || { echo "    scripts/release-record writes record key(s) the schema does not declare:$missing"; exit 1; }

missing=""
for k in $nested; do
  printf '%s\n' "$declared_artifact" | grep -qx "$k" || missing="$missing $k"
done
[ -z "$missing" ] \
  || { echo "    scripts/release-record writes artifact key(s) the schema does not declare:$missing"; exit 1; }

# the specific field the defect was about
printf '%s\n' "$declared" | grep -qx required_targets \
  || { echo "    the schema does not declare required_targets, which scripts/release-record writes"; exit 1; }

# and the schema is still closed — declaring the field must not have been done by opening the
# door to every field, which would have removed the guarantee instead of satisfying it
[ "$(jq -r '.additionalProperties' "$schema")" = "false" ] \
  || { echo "    the release schema is no longer additionalProperties:false; unknown keys would pass unnoticed"; exit 1; }

# a record carrying the field indexes cleanly. Built in a fixture, never in the real layer.
#
# `rust_bin` prints the executable's path and does not run it, and the two greps below are
# written as `if` rather than `cmd && { ...; exit 1; }`: that chain returns non-zero exactly
# when the grep finds nothing, which is the passing case, and under `set -e` it ends the case
# with no assertion message at all. Both mistakes were in the first version of this case and
# both look like a failing test rather than a broken one.
RB="$(rust_bin)" || rust_bin_exit $?
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
out="$("$RB" generate --repo "$T/repo" --check --strict 2>&1)" || true
[ -n "$out" ] || { echo "    the executable said nothing at all; the check did not reach its subject"; exit 1; }

if printf '%s\n' "$out" | grep -q "not in schema 'majordomus.release/v1'"; then
  echo "    a record carrying required_targets is still refused by the schema:"
  printf '%s\n' "$out" | grep -i 'schema' | sed 's/^/    /'
  exit 1
fi
if printf '%s\n' "$out" | grep -qi 'required_targets'; then
  echo "    required_targets is still reported as a problem:"
  printf '%s\n' "$out" | grep -i required_targets | sed 's/^/    /'
  exit 1
fi
# and no diagnostic names the record at all. Scoped to the record rather than to the index's
# overall state: a fixture repository is deliberately minimal and carries its own unrelated
# complaints, so `state=Degraded` here would be true of the fixture and say nothing about the
# subject. What this case owns is that nothing is wrong with *this file*.
if printf '%s\n' "$out" | grep -q 'releases/v9.9.9.yaml'; then
  echo "    a diagnostic names the release record the schema is supposed to accept:"
  printf '%s\n' "$out" | grep 'releases/v9.9.9.yaml' | sed 's/^/    /'
  exit 1
fi

printf 'ok   release record: %s record key(s) and %s artifact key(s) the writer emits are declared; a record carrying required_targets indexes under --strict\n' \
  "$(printf '%s\n' $top | grep -c .)" "$(printf '%s\n' $nested | grep -c .)"

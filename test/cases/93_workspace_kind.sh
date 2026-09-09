# majordomus-covers: none
# A workspace is a kind of the layer, and its declaration carries no credential.
#
# ADR 0025 decided that the catalogue of external workspaces is the repository's own
# tracked statement while the content those workspaces hold is checkout state that no
# surface reaches. This case proves the tracked half: that the kind is declared, that its
# schema is where the identifier says it is, that the generated allow-list closes the keys,
# and — the part that matters most — that nothing in a declaration can be, or is, a secret.
#
# It reads; it writes nothing outside its own temporary repository.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }

KINDS="$ROOT/share/kinds.yaml"
SCHEMA="$ROOT/share/schemas/majordomus/workspace/workspace.v1.schema.json"
ALLOW="$ROOT/share/allow/workspace.txt"

# --- the kind is declared, and declares the schema whose identifier fixes the path
expect_file "$KINDS"
grep -q '^  workspace:' "$KINDS" \
  || { echo "    share/kinds.yaml declares no workspace kind"; exit 1; }
sed -n '/^  workspace:/,/^$/p' "$KINDS" | grep -q 'schema: majordomus.workspace/v1' \
  || { echo "    the workspace kind names no majordomus.workspace/v1 schema"; exit 1; }
sed -n '/^  workspace:/,/^$/p' "$KINDS" | grep -q 'identity: \[id\]' \
  || { echo "    the workspace kind takes its identity from something other than id"; exit 1; }

# --- the schema is at the path the identifier derives, and says what it is
expect_file "$SCHEMA"
jq -e . "$SCHEMA" >/dev/null || { echo "    $SCHEMA does not parse as JSON"; exit 1; }
[ "$(jq -r '."x-majordomus-schema"' "$SCHEMA")" = "majordomus.workspace/v1" ] \
  || { echo "    the schema does not declare its own identifier"; exit 1; }
[ "$(jq -r '."x-majordomus-allow"' "$SCHEMA")" = "workspace" ] \
  || { echo "    the schema does not name the allow-list generated from it"; exit 1; }
[ "$(jq -r '.additionalProperties' "$SCHEMA")" = "false" ] \
  || { echo "    the schema admits unknown keys, which the layer refuses"; exit 1; }

# --- the allow-list is generated from it and closes the keys
expect_file "$ALLOW"
head -1 "$ALLOW" | grep -q 'GENERATED FILE' \
  || { echo "    $ALLOW does not say it is generated"; exit 1; }
for k in '\^schema\$' '\^kind\$' '\^id\$' '\^vendor\\.origins' '\^authorisation\\.statement\$' '\^access\\.browser_profile\$'; do
  grep -q -- "$k" "$ALLOW" || { echo "    the allow-list is missing $k"; exit 1; }
done

# --- a browser profile is a label, so a path or an environment variable cannot be written there
pat="$(jq -r '.["$defs"].access.properties.browser_profile.pattern' "$SCHEMA")"
[ "$pat" = '^[a-z][a-z0-9-]*$' ] \
  || { echo "    access.browser_profile is not constrained to a label; it is /$pat/"; exit 1; }

# --- no declaration carries anything credential-shaped
found=0
for f in "$ROOT"/.ai/repo/workspaces/*.yaml; do
  [ -e "$f" ] || break
  found=$((found + 1))
  if grep -inE '(secret|passwd|password|api[_-]?key|bearer|authorization|cookie|token)[[:space:]]*:' "$f" >/dev/null; then
    echo "    $(basename "$f") carries a credential-shaped key"; exit 1
  fi
  # a token pasted as a value, rather than as a key
  if grep -oE '\b(sk-[A-Za-z0-9_-]{16,}|gh[pousr]_[A-Za-z0-9]{16,}|eyJ[A-Za-z0-9_-]{20,})' "$f" >/dev/null; then
    echo "    $(basename "$f") carries something shaped like a credential value"; exit 1
  fi
  # the profile is a label in the file too, not only in the schema
  prof="$(sed -n 's/^  browser_profile:[[:space:]]*//p' "$f")"
  case "$prof" in
    ""|*/*|*'$'*|*' '*) echo "    $(basename "$f") names a browser profile that is not a label: '$prof'"; exit 1 ;;
  esac
done
[ "$found" -gt 0 ] || { echo "    no workspace is declared, so nothing proves the kind is usable"; exit 1; }

# --- and the tool discovers it: the shell compiler classes the file as a workspace
out="$(cd "$ROOT" && bin/majordomus knowledge sources 2>&1)" || { echo "    knowledge sources failed"; exit 1; }
printf '%s' "$out" | grep -qE '^workspace[[:space:]]' \
  || { echo "    the knowledge compiler discovers no workspace source"; exit 1; }

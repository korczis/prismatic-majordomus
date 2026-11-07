# majordomus-covers: none
# An identity provider is a declarative object, and Google and Facebook are two of them (I1201).
#
# The provider set is expected to change, so a provider is a file under
# .ai/repo/identity/providers/ rather than a match arm in the executable. This case proves the
# declaration half: the kind is declared with a closed schema, the allow-list is the generated
# one, the schema has one flow shape that every object fills completely (a field only one
# provider fills is the escape hatch the issue forbids), credentials are named and never
# carried, the index reads both objects with no diagnostic, and the schema refuses a value
# where a name belongs and a key it does not declare.
#
# It reads the tree and writes nothing outside its own temporary fixture.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || skip "jq absent"

KINDS="$ROOT/share/kinds.yaml"
SCHEMA="$ROOT/share/schemas/majordomus/identity-provider/identity-provider.v1.schema.json"
ALLOW="$ROOT/share/allow/identity-provider.txt"
DIR="$ROOT/.ai/repo/identity/providers"

# --- the kind is declared, and names the schema whose identifier fixes the path
expect_file "$KINDS"
block="$(sed -n '/^  identity-provider:/,/^$/p' "$KINDS")"
[ -n "$block" ] || { echo "    share/kinds.yaml declares no identity-provider kind"; exit 1; }
printf '%s\n' "$block" | grep -q 'schema: majordomus.identity-provider/v1' \
  || { echo "    the identity-provider kind names no majordomus.identity-provider/v1 schema"; exit 1; }
printf '%s\n' "$block" | grep -q 'identity: \[id\]' \
  || { echo "    the identity-provider kind takes its identity from something other than id"; exit 1; }

# --- the schema is at that path, names itself and its allow-list, and is closed at every level
expect_file "$SCHEMA"
jq -e . "$SCHEMA" >/dev/null || { echo "    $SCHEMA does not parse as JSON"; exit 1; }
[ "$(jq -r '."x-majordomus-schema"' "$SCHEMA")" = "majordomus.identity-provider/v1" ] \
  || { echo "    the schema does not declare its own identifier"; exit 1; }
[ "$(jq -r '."x-majordomus-allow"' "$SCHEMA")" = "identity-provider" ] \
  || { echo "    the schema does not name the allow-list generated from it"; exit 1; }
open="$(jq -r '[.. | objects | select(has("properties")) | select(.additionalProperties != false)] | length' "$SCHEMA")"
[ "$open" = 0 ] || { echo "    $open object(s) of the schema admit unknown keys: a per-provider escape hatch"; exit 1; }
hatch="$(jq -r '[.. | objects | select(has("patternProperties") or has("oneOf") or has("anyOf") or has("if"))] | length' "$SCHEMA")"
[ "$hatch" = 0 ] \
  || { echo "    the schema branches ($hatch place(s) with patternProperties/oneOf/anyOf/if): more than one flow shape"; exit 1; }
[ "$(jq -r '.properties.flow.const' "$SCHEMA")" = "oidc-authorization-code-pkce" ] \
  || { echo "    the schema does not fix the one flow, the OpenID Connect code flow with PKCE"; exit 1; }
[ "$(jq -r '.properties.scopes.contains.const' "$SCHEMA")" = "openid" ] \
  || { echo "    the schema does not require the openid scope"; exit 1; }
[ "$(jq -r '."$defs".env.pattern' "$SCHEMA")" = '^[A-Z][A-Z0-9_]*$' ] \
  || { echo "    a credential field is not constrained to an environment variable name"; exit 1; }

# --- the allow-list is the generated one (generate --check proves its bytes)
expect_file "$ALLOW"
head -1 "$ALLOW" | grep -q 'GENERATED FILE' || { echo "    $ALLOW does not say it is generated"; exit 1; }
grep -q 'majordomus.identity-provider/v1' "$ALLOW" \
  || { echo "    $ALLOW does not name the schema it is generated from"; exit 1; }

# The dotted leaf paths an object carries, from the two-space block YAML these objects are
# written in: `a: v` is `a`, `  b: v` under `a:` is `a.b`, a `- item` under `a:` is `a`.
leaves() {
  awk '
    /^[[:space:]]*#/ || /^[[:space:]]*$/ { next }
    /^[a-z_]+:/ { k = $1; sub(/:.*/, "", k); parent = k; v = $0; sub(/^[^:]*:[[:space:]]*/, "", v)
                  if (v != "") print k; next }
    /^  [a-z_]+:/ { k = $1; sub(/:.*/, "", k); print parent "." k; next }
    /^  - / { print parent; next }
  ' "$1" | LC_ALL=C sort -u
}
schema_leaves="$(sed -n 's/^\^\(.*\)\$$/\1/p' "$ALLOW" | sed 's/(\\\.\[0-9\]+)?$//; s/\\\././g' | LC_ALL=C sort -u)"
[ -n "$schema_leaves" ] || { echo "    could not read the keys of $ALLOW; the check cannot reach its subject"; exit 1; }

# --- google and facebook exist, and every object fills every field the schema declares
for want in google facebook; do
  [ -f "$DIR/$want.yaml" ] || { echo "    no $want identity provider at .ai/repo/identity/providers/$want.yaml"; exit 1; }
done
found=0 orders="" names=""
for f in "$DIR"/*.yaml; do
  [ -e "$f" ] || break
  found=$((found + 1)); b="$(basename "$f" .yaml)"
  have="$(leaves "$f")"
  missing="$(LC_ALL=C comm -23 <(printf '%s\n' "$schema_leaves") <(printf '%s\n' "$have") | tr '\n' ' ')"
  [ -z "${missing// /}" ] \
    || { echo "    $b.yaml leaves field(s) of the one shape empty: $missing— a field one provider cannot fill"; exit 1; }
  extra="$(LC_ALL=C comm -13 <(printf '%s\n' "$schema_leaves") <(printf '%s\n' "$have") | tr '\n' ' ')"
  [ -z "${extra// /}" ] || { echo "    $b.yaml carries key(s) the schema does not declare: $extra"; exit 1; }
  [ "$(sed -n 's/^id:[[:space:]]*//p' "$f")" = "$b" ] \
    || { echo "    $b.yaml declares an id other than its file name"; exit 1; }
  orders="$orders$(sed -n 's/^order:[[:space:]]*//p' "$f")
"
  # credentials are environment variable names, distinct, and never a value
  for k in client_id_env client_secret_env; do
    v="$(sed -n "s/^  $k:[[:space:]]*//p" "$f")"
    printf '%s' "$v" | grep -qE '^[A-Z][A-Z0-9_]*$' \
      || { echo "    $b.yaml: credentials.$k is '$v', which is not an environment variable name"; exit 1; }
    names="$names$v
"
  done
  if grep -inE '^[[:space:]]*[a-z_]*(secret|password|api[_-]?key|token|cookie|bearer)[a-z_]*[[:space:]]*:' "$f" \
      | grep -vE ':[[:space:]]*client_secret_env:' | grep -vE '^[0-9]+:[[:space:]]*token:[[:space:]]*https://' >/dev/null; then
    echo "    $b.yaml carries a credential-shaped key"; exit 1
  fi
  if grep -oE '\b(GOCSPX-[A-Za-z0-9_-]{8,}|ya29\.[A-Za-z0-9_-]{8,}|EAA[A-Za-z0-9]{16,}|[0-9]{6,}-[a-z0-9]{16,}\.apps\.googleusercontent\.com|eyJ[A-Za-z0-9_-]{20,})' "$f" >/dev/null; then
    echo "    $b.yaml carries something shaped like a credential value"; exit 1
  fi
done
[ "$found" -ge 2 ] || { echo "    $found identity provider(s) declared; google and facebook are the first two"; exit 1; }
[ -z "$(printf '%s' "$orders" | LC_ALL=C sort | uniq -d)" ] \
  || { echo "    two providers share a place in the sign-in offer: $(printf '%s' "$orders" | sort | uniq -d | tr '\n' ' ')"; exit 1; }
[ -z "$(printf '%s' "$names" | LC_ALL=C sort | uniq -d)" ] \
  || { echo "    two credentials arrive in one environment variable: $(printf '%s' "$names" | sort | uniq -d | tr '\n' ' ')"; exit 1; }

# --- the README and front-matter doctrine holds at every level of the directory
for d in "$ROOT/.ai/repo/identity" "$DIR"; do
  [ -f "$d/README.md" ] || { echo "    ${d#"$ROOT"/} has no README.md"; exit 1; }
  if ! head -1 "$d/README.md" | grep -qx -- '---' \
      || ! sed -n '2,/^---$/p' "$d/README.md" | grep -q '^schema: context/v1$'; then
    echo "    ${d#"$ROOT"/}/README.md carries no context/v1 front matter"; exit 1
  fi
done

# --- the index reads both objects like any other kind, with no diagnostic
RB="$(rust_bin)" || rust_bin_exit $?
out="$(cd "$ROOT" && "$RB" entity kinds --repo "$ROOT" --share "$ROOT/share" 2>&1)" \
  || { echo "    entity kinds failed:"; printf '%s\n' "$out" | tail -5 | sed 's/^/    /'; exit 1; }
printf '%s\n' "$out" | grep -qE "^identity-provider[[:space:]]+${found}[[:space:]]" \
  || { echo "    the index does not report $found identity-provider object(s):"; printf '%s\n' "$out" | grep -i identity | sed 's/^/    /'; exit 1; }
if printf '%s\n' "$out" | grep -E 'WARN|ERROR' | grep -q '.ai/repo/identity/'; then
  echo "    a diagnostic names an identity provider:"
  printf '%s\n' "$out" | grep '.ai/repo/identity/' | sed 's/^/    /'
  exit 1
fi

# --- and the schema refuses a value where a name belongs, a missing openid scope and an
# unknown key. Built in a minimal fixture, never in the real layer.
F="$T/fx"
mkdir -p "$F/.ai/repo/knowledge"
cp "$ROOT/.ai/manifest.yaml" "$ROOT/.ai/README.md" "$F/.ai/"
cp "$ROOT/.ai/repo/README.md" "$ROOT/.ai/repo/scope.yaml" "$F/.ai/repo/"
cp "$ROOT/.ai/repo/knowledge/sources.yaml" "$ROOT/.ai/repo/knowledge/README.md" "$F/.ai/repo/knowledge/"
cp -R "$ROOT/.ai/repo/identity" "$F/.ai/repo/"
p="$F/.ai/repo/identity/providers"
sed 's/^  client_secret_env: .*/  client_secret_env: GOCSPX-not-a-name-but-a-value/' "$p/google.yaml" > "$p/g.tmp" && mv "$p/g.tmp" "$p/google.yaml"
printf 'tenant: common\n' >> "$p/facebook.yaml"
sed 's/^id: google$/id: nosub/; s/^  - openid$/  - profile/; s/^order: .*/order: 99/' "$ROOT/.ai/repo/identity/providers/google.yaml" > "$p/nosub.yaml"
bad="$(cd "$F" && "$RB" entity kinds --repo "$F" --share "$ROOT/share" --discovery filesystem 2>&1)" || true
[ -n "$bad" ] || { echo "    the executable said nothing about the fixture; the check did not reach its subject"; exit 1; }
printf '%s\n' "$bad" | grep 'providers/google.yaml' | grep -q 'credentials.client_secret_env' \
  || { echo "    a client secret written where its variable's name belongs was not refused:"; printf '%s\n' "$bad" | grep -v ' INFO ' | sed 's/^/    /'; exit 1; }
printf '%s\n' "$bad" | grep 'providers/facebook.yaml' | grep -q 'tenant' \
  || { echo "    a provider-specific key was not refused:"; printf '%s\n' "$bad" | grep -v ' INFO ' | sed 's/^/    /'; exit 1; }
printf '%s\n' "$bad" | grep -q 'providers/nosub.yaml' \
  || { echo "    a provider that does not ask for the openid scope was not refused:"; printf '%s\n' "$bad" | grep -v ' INFO ' | sed 's/^/    /'; exit 1; }

printf 'ok   identity providers: %s object(s) fill the one shape of %s field(s), name their credentials, index cleanly; a value, a stray key and a missing openid scope are refused\n' \
  "$found" "$(printf '%s\n' "$schema_leaves" | grep -c .)"

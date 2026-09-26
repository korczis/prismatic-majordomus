# majordomus-covers: none
# An entity names its public documentation page, and the page it names is the page the site
# publishes.
#
# Which kinds the site publishes, and where, is declared once in site/data/publication.toml.
# scripts/generate-site-data reads it to write the site's entity pages and
# site/data/generated/entities.json; the executable reads the same file at request time to
# answer `documentation` on `entity show`. Two readers of one declaration can still disagree
# about the function they apply to it, and that disagreement is what this case measures:
#
#   1. an ADR (a kind published one page per object) names https://majordomus.dev/adrs/<slug>/,
#      and that route is the kind route and slug entities.json carries, under the site's
#      base_url, with a generated page behind it;
#   2. a rule — the other entity kind — does the same, so the ADR is not a special case;
#   3. an object of a kind an editorial section publishes names the section, exactly the route
#      the generator's own `public()` gave that object in entities.json;
#   4. an object of a kind that is not published names no address and carries the reason the
#      declaration gives.
#
# Nothing here restates a route: every expected value is read from the derived data or the
# declaration.
. "$ROOT/test/lib.sh"

command -v jq >/dev/null 2>&1 || { echo "    jq absent; skipping"; exit 0; }
DATA="$ROOT/site/data/generated/entities.json"
PUB="$ROOT/site/data/publication.toml"
[ -f "$DATA" ] || { echo "    $DATA is missing; run scripts/derive"; exit 1; }
[ -f "$PUB" ] || { echo "    $PUB is missing"; exit 1; }

MJ="$(rust_bin)" || rust_bin_exit $?

fail=0
note() { echo "    $1"; fail=1; }

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-531.XXXXXX")"
trap 'rm -rf "$S"' EXIT

show() {
  local rc=0
  env -u MAJORDOMUS_SHARE "$MJ" entity show "$1" --repo "$ROOT" --format json > "$S/show.json" 2> "$S/show.err" || rc=$?
  if [ "$rc" != 0 ]; then
    note "majordomus entity show $1 exited $rc:"; sed 's/^/      | /' "$S/show.err"
    return 1
  fi
}

# the site's base_url, as site/config.toml declares it (a top-level key)
base="$(sed -n 's/^base_url[[:space:]]*=[[:space:]]*"\(.*\)"[[:space:]]*$/\1/p' "$ROOT/site/config.toml" | head -1)"
base="${base%/}"
[ -n "$base" ] || { echo "    site/config.toml declares no base_url"; exit 1; }

# ---------------------------------------------------------------- 1 and 2: the entity kinds
for kind in adr rule; do
  kroute="$(jq -r --arg k "$kind" '.kinds[] | select(.kind == $k) | .route' "$DATA")"
  kdir="$(jq -r --arg k "$kind" '.kinds[] | select(.kind == $k) | .dir' "$DATA")"
  slug="$(jq -r --arg k "$kind" '[.entities[] | select(.kind == $k) | .slug] | sort | first // empty' "$DATA")"
  if [ -z "$kroute" ] || [ -z "$slug" ]; then
    note "entities.json publishes no $kind page; nothing to compare against"; continue
  fi
  show "$kind/$slug" || continue
  want_route="$kroute$slug/"
  got_route="$(jq -r '.documentation.route // "null"' "$S/show.json")"
  got_url="$(jq -r '.documentation.url // "null"' "$S/show.json")"
  got_proj="$(jq -r '.documentation.projection // "null"' "$S/show.json")"
  [ "$got_proj" = entity ] || note "$kind/$slug: projection is '$got_proj', not entity"
  [ "$got_route" = "$want_route" ] \
    || note "$kind/$slug: entity show names route '$got_route'; the site publishes '$want_route'"
  [ "$got_url" = "$base$want_route" ] \
    || note "$kind/$slug: entity show names '$got_url'; the site publishes '$base$want_route'"
  # the generator wrote a page for exactly that route
  [ -f "$ROOT/site/content/$kdir/$slug.md" ] \
    || note "$kind/$slug: no generated page at site/content/$kdir/$slug.md behind $want_route"
  if [ "$kind" = adr ]; then
    case "$got_url" in
      "https://majordomus.dev/adrs/$slug/") ;;
      *) note "the ADR $slug names '$got_url', not https://majordomus.dev/adrs/$slug/" ;;
    esac
  fi
done

# ---------------------------------------------------------------- 3: a section kind
# an object entities.json links through the generator's public(): a kind whose route is its
# section, not a page of its own. Its label is its slug when it is already one segment.
jq -c '[.kinds[].kind] as $own
       | [.entities[] | (.references[], .referenced_by[])
          | select(.route != null and .external == false and (.kind | IN($own[]) | not)
                   and (.label | test("^[a-z0-9]+(-[a-z0-9]+)*$")))]
       | unique_by(.kind) | .[]' "$DATA" | LC_ALL=C sort > "$S/sections.jsonl"
[ -s "$S/sections.jsonl" ] || note "entities.json links no object of a section kind; nothing to compare against"
checked=0
while IFS= read -r side; do
  kind="$(printf '%s' "$side" | jq -r .kind)"
  label="$(printf '%s' "$side" | jq -r .label)"
  want="$(printf '%s' "$side" | jq -r .route)"
  show "$kind/$label" || continue
  got="$(jq -r '.documentation.route // "null"' "$S/show.json")"
  [ "$got" = "$want" ] || note "$kind/$label: entity show names '$got'; the generator linked it at '$want'"
  [ "$(jq -r '.documentation.projection' "$S/show.json")" = section ] \
    || note "$kind/$label: projection is not section"
  checked=$((checked + 1))
  if [ "$checked" -ge 2 ]; then break; fi
done < "$S/sections.jsonl"

# ---------------------------------------------------------------- 4: a kind that is not published
KINDS="$S/kinds.json"
rc=0
env -u MAJORDOMUS_SHARE "$MJ" entity kinds --repo "$ROOT" --format json > "$KINDS" 2> "$S/kinds.err" || rc=$?
[ "$rc" = 0 ] || { note "majordomus entity kinds exited $rc"; sed 's/^/      | /' "$S/kinds.err"; exit 1; }
# the first kind, in byte order, that the declaration does not publish and the index holds
none_kind=""
for k in $(awk '/^\[\[kinds\]\]/ { k = "" } /^kind = / { k = $3 } /^projection = "none"/ { print k }' "$PUB" \
           | tr -d '"' | LC_ALL=C sort); do
  if jq -e --arg k "$k" 'any(.kinds[]; .kind == $k and .example != null)' "$KINDS" >/dev/null; then
    none_kind="$k"; break
  fi
done
if [ -z "$none_kind" ]; then
  note "no unpublished kind of the declaration has an object in the index"
else
  uri="$(jq -r --arg k "$none_kind" '.kinds[] | select(.kind == $k) | .example.uri' "$KINDS")"
  if show "$uri"; then
    [ "$(jq -r '.documentation.projection' "$S/show.json")" = none ] \
      || note "$uri: an unpublished kind does not say projection none"
    [ "$(jq -r '.documentation | has("url") or has("route")' "$S/show.json")" = false ] \
      || note "$uri: an unpublished kind names an address"
    reason="$(jq -r '.documentation.reason // empty' "$S/show.json")"
    [ -n "$reason" ] && grep -qF "reason = \"$reason\"" "$PUB" \
      || note "$uri: the reason given is not the declaration's: '$reason'"
  fi
fi

exit "$fail"

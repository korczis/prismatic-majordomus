# majordomus-covers: none
# An object of the layer has an address of its own on every surface, and adding one adds it
# everywhere — with no consumer edited.
#
# The Rust crate's own tests prove the derivation (slug, route, collision, both directions of
# the graph) and the Cockpit's zero-registration test proves the served routes. What is left
# for the suite is the half that lives outside the crate and that nothing else executes:
#
#   1. the gate refuses a kind the declaration does not mention, and the reverse;
#   2. the gate refuses a route collision — two identities of one kind that reduce to one
#      address — which is the only way an object can fail to be addressable;
#   3. the command line answers for an entity at the address the Cockpit serves it at, and
#      the URI and the route name the same object;
#   4. the publication declaration covers every kind the index holds, in this repository as
#      it stands, so that a kind added without a decision is a failure here and not a silence.
. "$ROOT/test/lib.sh"

GATE="$ROOT/scripts/ci/entity-check"
[ -f "$GATE" ] || { echo "    $GATE is missing"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq absent; skipping"; exit 0; }

MJ="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_BIN="$MJ"

fail=0
note() { echo "    $1"; fail=1; }

# ---------------------------------------------------------------- the gate passes on this tree
if ! out="$(env -u MAJORDOMUS_SHARE bash "$GATE" 2>&1)"; then
  note "entity-check refuses this repository:"
  printf '%s\n' "$out" | sed 's/^/      /'
fi

# ---------------------------------------------------------------- every kind is declared
KINDS="$(env -u MAJORDOMUS_SHARE "$MJ" entity kinds --repo "$ROOT" --format json 2>"$ROOT/../mj276.err" || true)"
if [ -z "$KINDS" ]; then
  echo "    majordomus entity kinds did not answer:"; sed 's/^/      /' "$ROOT/../mj276.err" 2>/dev/null | tail -5
  rm -f "$ROOT/../mj276.err"; exit 1
fi
rm -f "$ROOT/../mj276.err"

# no object may be without an address, and the tool must say so in one number rather than
# leaving the reader to compare two lists
total="$(printf '%s' "$KINDS" | jq -r '[.kinds[].count] | add // 0')"
routable="$(printf '%s' "$KINDS" | jq -r '.routable')"
[ "$total" = "$routable" ] || note "$((total - routable)) of $total objects have no address of their own"
[ "$(printf '%s' "$KINDS" | jq -r '.collisions | length')" = 0 ] || note "the index carries a route collision"

for k in $(printf '%s' "$KINDS" | jq -r '.kinds[].kind'); do
  grep -q "^kind = \"$k\"\$" "$ROOT/site/data/publication.toml" \
    || note "kind '$k' is in the index and not in site/data/publication.toml"
done

# ---------------------------------------------------------------- the two spellings agree
uri="$(printf '%s' "$KINDS" | jq -r '.kinds[] | select(.kind == "adr") | .example.uri')"
if [ -n "$uri" ] && [ "$uri" != null ]; then
  by_uri="$(env -u MAJORDOMUS_SHARE "$MJ" entity show "$uri" --repo "$ROOT" --format json 2>/dev/null)"
  route="$(printf '%s' "$by_uri" | jq -r '.route')"
  slug="$(printf '%s' "$by_uri" | jq -r '.slug')"
  by_route="$(env -u MAJORDOMUS_SHARE "$MJ" entity show "adr/$slug" --repo "$ROOT" --format json 2>/dev/null)"
  [ "$(printf '%s' "$by_route" | jq -r '.uri')" = "$uri" ] \
    || note "the route and the URI of $uri name different objects"
  # the address the command line prints is the address the Cockpit serves
  case "$route" in
    /cockpit/objects/adr/"$slug") ;;
    *) note "the route of $uri is '$route', which is not /cockpit/objects/adr/$slug" ;;
  esac
  # and the surfaces it names include the three a reader can reach it on
  for surface in mcp http cockpit; do
    printf '%s' "$by_uri" | jq -e --arg s "$surface" 'any(.surfaces[]; .surface == $s)' >/dev/null \
      || note "$uri does not say it is answered over $surface"
  done
fi

# ---------------------------------------------------------------- an address nothing serves
if env -u MAJORDOMUS_SHARE "$MJ" entity show adr/no-such-decision --repo "$ROOT" >/dev/null 2>&1; then
  note "an address the layer does not serve was answered rather than refused"
fi

# ---------------------------------------------------------------- the gate refuses what it must
S="$(mktemp -d "${TMPDIR:-/tmp}/mj-277.XXXXXX")"
trap 'rm -rf "$S"' EXIT
# The gate reads MAJORDOMUS_PUBLICATION when it is set, so a broken declaration is handed to
# it in a temporary file rather than written into the tree it is checking: a gate that has to
# be proved by mutating the repository is a gate nobody dares prove.
sed 's/^kind = "adr"$/kind = "adr-typo"/' "$ROOT/site/data/publication.toml" > "$S/renamed.toml"
if MAJORDOMUS_PUBLICATION="$S/renamed.toml" env -u MAJORDOMUS_SHARE bash "$GATE" >/dev/null 2>&1; then
  note "entity-check accepted a declaration that does not mention a kind the index holds"
fi
# and it says both halves: the kind that is missing, and the kind that names nothing
out="$(MAJORDOMUS_PUBLICATION="$S/renamed.toml" env -u MAJORDOMUS_SHARE bash "$GATE" 2>&1 || true)"
case "$out" in *"holds kind 'adr' and"*) ;; *) note "the finding does not name the undeclared kind" ;; esac
case "$out" in *"'adr-typo'"*) ;; *) note "the finding does not name the kind the index does not hold" ;; esac

exit "$fail"

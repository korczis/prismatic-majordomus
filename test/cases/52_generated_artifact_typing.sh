# majordomus-exclusive: reads and briefly mutates docs/generated/ of this checkout
# majordomus-covers: none
# Every generated artifact of *this* checkout is typed, and the manifest is the tree.
#
# apps/majordomus-cli/tests/generated_documents.rs proves the property over a plan the
# executable builds in memory, and proves that each clause, broken, is refused. This case
# proves the same thing about the files that are actually committed here — a plan can be
# right while the committed tree is stale, which is exactly the failure `--check` exists
# for — and it proves the one claim Rust cannot make on its own: that a `.yaml` encoding
# is real YAML and decodes to the same document as its `.json` sibling, read by a YAML
# parser this repository does not own.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }
MAN="$ROOT/docs/generated/artifacts.json"
expect_file "$MAN"

# --- the manifest is a manifest
[ "$(jq -r '.schema' "$MAN")" = "majordomus/generated-artifacts/v1" ] \
  || { echo "    $MAN is not a majordomus/generated-artifacts/v1 document"; exit 1; }
jq -e '.generated | startswith("GENERATED FILE")' "$MAN" >/dev/null \
  || { echo "    the manifest does not say it is generated"; exit 1; }
n="$(jq '.artifacts | length' "$MAN")"
[ "$n" -gt 20 ] || { echo "    the manifest lists only $n artifacts"; exit 1; }

# --- every file it names exists, and its bytes and hash are what the manifest recorded
jq -r '.artifacts[] | [.path, (.bytes // "-"), (.sha256 // "-"), .format, .document] | @tsv' "$MAN" \
| while IFS="$(printf '\t')" read -r path bytes sha format document; do
  f="$path"; case "$path" in /*) ;; *) f="$ROOT/$path" ;; esac
  [ -f "$f" ] || { echo "    the manifest names $path and no such file exists"; exit 1; }
  [ -n "$document" ] || { echo "    $path projects no document"; exit 1; }
  if [ "$sha" = "-" ]; then
    # the manifest's own encodings: `generate --check` compares them byte for byte
    jq -e --arg p "$path" '.artifacts[] | select(.path==$p) | .describes_itself == true' "$MAN" >/dev/null \
      || { echo "    $path carries no hash and does not say it describes itself"; exit 1; }
    continue
  fi
  got="$(wc -c < "$f" | tr -d ' ')"
  [ "$got" = "$bytes" ] || { echo "    $path is $got bytes and the manifest records $bytes"; exit 1; }
  gh="$(shasum -a 256 "$f" | cut -d' ' -f1)"
  [ "$gh" = "$sha" ] || { echo "    $path hashes to $gh and the manifest records $sha"; exit 1; }
  # --- and it carries a provenance header in the form its encoding allows
  case "$document" in providers/*) continue ;; esac
  case "$format" in
    markdown) head -n 2 "$f" | grep -q '^<!-- GENERATED FILE' || { echo "    $path carries no banner"; exit 1; } ;;
    # A `#`-commented encoding carries the banner on its first line — unless the file is
    # something a kernel reads before anything else does, in which case the shebang is line
    # one and the banner is line two. site/static/install.sh is generated *and* executable;
    # demanding the banner first would demand a script that cannot run.
    yaml|text) head -n 2 "$f" | grep -q '^# GENERATED FILE' \
                 && { [ "$(head -n 1 "$f" | grep -c '^\(#!\|# GENERATED FILE\)')" = 1 ] \
                      || { echo "    $path buries its banner below something that is not a shebang"; exit 1; }; } \
                 || { echo "    $path carries no banner"; exit 1; } ;;
    json) jq -e '(.generated // .["x-majordomus-generated"] // "") | startswith("GENERATED FILE")' "$f" >/dev/null \
            || { echo "    $path says nothing about being generated"; exit 1; } ;;
    *) echo "    $path declares the unknown encoding $format"; exit 1 ;;
  esac
done || exit 1

# --- every generated file under docs/generated is in the manifest: a file the index does
#     not name is a file nothing regenerates
for f in "$ROOT"/docs/generated/*.* "$ROOT"/docs/generated/modules/*.*; do
  [ -f "$f" ] || continue
  rel="${f#"$ROOT"/}"
  jq -e --arg p "$rel" 'any(.artifacts[]; .path == $p)' "$MAN" >/dev/null \
    || { echo "    $rel exists and the manifest does not name it"; exit 1; }
done

# --- where a document is committed in both JSON and YAML, the two are one value
# The rule is that a document is written in every encoding this repository commits it in,
# from one value — not that a JSON document must also be committed as YAML. A dataset with
# a single machine reader (the site's registry, the release build matrix a workflow reads
# with fromJSON) is committed as JSON alone and is not in breach; what would be a breach is
# two encodings that disagree. Paths come from the manifest rather than being composed from
# the document id, so a document that lives outside docs/generated needs no exception.
YAML_READER=""
if command -v ruby >/dev/null 2>&1 && ruby -ryaml -rjson -e '' 2>/dev/null; then YAML_READER=ruby
elif python3 -c 'import yaml' 2>/dev/null; then YAML_READER=python; fi
pairs=0
for id in $(jq -r '.documents[] | select((.formats | index("json")) and (.formats | index("yaml"))) | .id' "$MAN"); do
  case "$id" in providers/*) continue ;; esac
  j="$ROOT/$(jq -r --arg id "$id" '.artifacts[] | select(.document==$id and .format=="json") | .path' "$MAN" | head -n 1)"
  y="$ROOT/$(jq -r --arg id "$id" '.artifacts[] | select(.document==$id and .format=="yaml") | .path' "$MAN" | head -n 1)"
  expect_file "$j"; expect_file "$y"
  pairs=$((pairs + 1))
  case "$YAML_READER" in
    ruby)
      ruby -ryaml -rjson -e '
        y = YAML.safe_load(File.read(ARGV[1]), permitted_classes: [], aliases: false)
        j = JSON.parse(File.read(ARGV[0]))
        abort("the YAML and the JSON of #{ARGV[2]} are different documents") unless y == j
      ' "$j" "$y" "$id" || exit 1 ;;
    python)
      python3 -c '
import json, sys, yaml
j = json.load(open(sys.argv[1])); y = yaml.safe_load(open(sys.argv[2]))
if j != y: sys.exit("the YAML and the JSON of %s are different documents" % sys.argv[3])
' "$j" "$y" "$id" || exit 1 ;;
  esac
done
[ "$pairs" -gt 0 ] || { echo "    no document is committed in two encodings; this check has stopped checking anything"; exit 1; }
[ -n "$YAML_READER" ] || echo "    note: no YAML parser (ruby/psych or python3/PyYAML); the encodings were compared by declaration only"

# --- every schema a document names is published and pins that document
for s in $(jq -r '.documents[] | select(.schema) | .schema' "$MAN" | sort -u); do
  grep -rl "\"$s\"" "$ROOT/share/schemas/generated"/*.schema.json >/dev/null 2>&1 \
    || { echo "    the document schema $s has no published contract under share/schemas/generated/"; exit 1; }
done
for f in "$ROOT"/share/schemas/generated/*.schema.json; do
  jq -e '.properties.schema.const' "$f" >/dev/null \
    || { echo "    ${f#"$ROOT"/} pins no document (properties.schema.const)"; exit 1; }
done

# --- and the whole tree is what the executable would write right now
RB="$(rust_bin)" || rust_bin_exit $?
expect_exit 0 "$RB" generate --check --repo "$ROOT"
expect_grep 'in sync'

# --- the mutation: a hand edit to any encoding is refused, and the encodings are restored
#     together. Done in a copy, because this checkout is the repository under test.
orig="$(mktemp "${TMPDIR:-/tmp}/mj-registry-yaml.XXXXXX")"
cp "$ROOT/docs/generated/registry.yaml" "$orig"
printf '\n' >> "$ROOT/docs/generated/registry.yaml"
rc=0; "$RB" generate --check --repo "$ROOT" >/dev/null 2>&1 || rc=$?
cp "$orig" "$ROOT/docs/generated/registry.yaml"; rm -f "$orig"
[ "$rc" = 10 ] || { echo "    a hand edit to docs/generated/registry.yaml was not refused (exit $rc)"; exit 1; }
expect_exit 0 "$RB" generate --check --repo "$ROOT"

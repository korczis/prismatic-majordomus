# majordomus-covers: none
# A doc comment reaches a projection as CommonMark, and the site never publishes its source.
#
# schemars copies a Rust doc comment into `description` verbatim, and every consumer of that
# field reads it as CommonMark: OpenAPI says so, MCP hands it to a client, and the site hands
# it to a Markdown renderer. Rustdoc is not CommonMark. It adds an intra-doc link,
# [`Type::Variant`], which is a shortcut reference with no definition anywhere, and reference
# definitions pointing at paths relative to the source file. Neither resolves outside rustdoc,
# so both reached the reader as source: /docs/api/ published ``[`WaiverReason::…`]:`` as body
# text, and that 37-character unbreakable token made the page scroll sideways at 320px.
#
# Three parts, and none is sufficient alone. The first is the invariant at the boundary
# where a doc comment becomes an artifact: no description any projection reads may carry a
# link only rustdoc can follow. That is what `majordomus generate` now guarantees, and it
# holds for every consumer, not only the one that was measured.
#
# The second is that the API reference renders what the artifact carries: a template slot that
# interpolates a description as text publishes rustdoc's *emphasis* and brackets regardless.
#
# The third is that the page detector still detects. A guard whose subject moved reports clean
# for the wrong reason, so site-check's intra-doc-link check is run against markup whose answer
# is known: the form a renderer leaves must be named, and correctly rendered prose must not be.
. "$ROOT/test/lib.sh"

# ---------------------------------------------------------------- the artifact boundary
# Over the committed artifacts, because they are what every reader outside this repository
# gets. jq walks to the descriptions rather than grepping the file: a `[` elsewhere in the
# document — an array in an example, a character class in a pattern — is not prose.
for artifact in registry openapi; do
  doc="$ROOT/docs/generated/$artifact.json"
  expect_file "$doc"
  jq -r '[paths(type == "string") as $p | select(($p | last) == "description") | getpath($p)]
         | .[]' "$doc" > "$T/$artifact.desc"
  # a walk that finds nothing proves nothing: the artifact carries thousands of descriptions
  [ "$(wc -l < "$T/$artifact.desc")" -gt 1000 ] || {
    echo "    $artifact.json: fewer than 1000 description lines read; the walk lost its subject"; exit 1; }
  # every description of the document, with its fenced code blocks removed: a doc comment's
  # Rust example is not prose, and its brackets are array literals and character classes
  awk '/^[ \t]*(```|~~~)/ { fence = !fence; next } !fence' "$T/$artifact.desc" > "$T/$artifact.prose"
  if grep -nE '\[`[^`]+`\]' "$T/$artifact.prose"; then
    echo "    $artifact.json: a description carries a rustdoc intra-doc link, which no"
    echo "    CommonMark reader can resolve (fix: apps/majordomus-cli/src/capability/rustdoc.rs)"
    exit 1
  fi
  # a reference definition whose destination is a path relative to the Rust source file
  if grep -nE '^[ \t]*\[[^]]+\]:[ \t]*\.\.?/' "$T/$artifact.prose"; then
    echo "    $artifact.json: a description carries a link definition relative to the source"
    echo "    file, which resolves for rustdoc and for no other reader"
    exit 1
  fi
done

# ---------------------------------------------------------------- the templates render it
# The artifact carrying CommonMark is half the fix. /docs/api/ printed *published* with its
# asterisks and [`WaiverReason::TransientState`] with its brackets because one slot, an enum
# value's description, interpolated the text instead of rendering it. Every description a
# schema slot of the API reference shows goes through the renderer, or its source is published.
schema_table="$ROOT/site/templates/partials/schema-table.html"
api_schema="$ROOT/site/templates/partials/api-schema.html"
expect_file "$schema_table"
expect_file "$api_schema"
# the API reference's schema article, which is where a doc comment arrives: since the reference
# became a page per tag it is one partial every page includes; the operation, tag and server
# descriptions around it are declared prose, not doc comments
cp "$api_schema" "$T/api-schemas.html"
[ -s "$T/api-schemas.html" ] || { echo "    partials/api-schema.html is empty"; exit 1; }
: > "$T/raw-slots"
grep -nE '\{\{ *[a-z]+\.description *\}\}' "$schema_table" >> "$T/raw-slots" || true
grep -nE '\{\{ *[a-z]+\.description *\}\}' "$T/api-schemas.html" >> "$T/raw-slots" || true
if [ -s "$T/raw-slots" ]; then
  echo "    a schema description is interpolated as text, not rendered as the CommonMark it is:"
  cat "$T/raw-slots"; exit 1
fi
rendered="$(grep -cE 'description \| markdown' "$T/api-schemas.html" || true)"
[ "${rendered:-0}" -ge 4 ] || {
  echo "    the API reference's schema partial renders ${rendered:-0} description slot(s) through"
  echo "    markdown; it has four (type, enum value, variant, variant property)"; exit 1; }

# ---------------------------------------------------------------- the detector detects
# The page check of site-check, run over markup whose answer is known. A guard whose subject
# moved reports clean for the wrong reason, so the intra-doc link as a renderer leaves it — a
# bracket, a <code>, a bracket — must be named, and the same identifier rendered correctly,
# or inside a listing where brackets are Rust, must not be.
awk_lib="$ROOT/scripts/lib/site-pages.awk"
expect_file "$awk_lib"
mkdir -p "$T/pub"
cat > "$T/pub/link.html" <<'HTML'
<html><body><main><p>Distinct from [<code>WaiverReason::TransientState</code>]: durable.</p></main></body></html>
HTML
cat > "$T/pub/clean.html" <<'HTML'
<html><body><main><p>Distinct from <code>WaiverReason::TransientState</code>: <em>durable</em>.</p>
<pre><code>let v = [<code>Type</code>]; // brackets are Rust in a listing</code></pre></main></body></html>
HTML

awk -v pub="$T/pub" -f "$awk_lib" "$T/pub/link.html" "$T/pub/clean.html" > "$T/rows" 2>&1

grep -E '^link\.html	rustdoc	' "$T/rows" > /dev/null || {
  echo "    a bracket around a code span is an unresolved intra-doc link, and went unnamed"; cat "$T/rows"; exit 1; }
if grep -E '^clean\.html	rustdoc' "$T/rows" > /dev/null; then
  echo "    correctly rendered prose was reported as an unresolved link; the check would refuse a"
  echo "    healthy page, which is how a gate gets turned into a warning"; cat "$T/rows"; exit 1
fi

# and site-check reports that row rather than collecting it silently
expect_grep 'rows rustdoc' "$ROOT/scripts/site-check"
expect_grep 'no unresolved intra-doc links' "$ROOT/scripts/site-check"

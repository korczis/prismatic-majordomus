# majordomus-covers: generate site-check
# majordomus-negative: unrendered markup
# A doc comment reaches a projection as CommonMark, and the site never publishes its source.
#
# schemars copies a Rust doc comment into `description` verbatim, and every consumer of that
# field reads it as CommonMark: OpenAPI says so, MCP hands it to a client, and the site hands
# it to a Markdown renderer. Rustdoc is not CommonMark. It adds an intra-doc link,
# [`Type::Variant`], which is a shortcut reference with no definition anywhere, and reference
# definitions pointing at paths relative to the source file. Neither resolves outside rustdoc,
# so both reached the reader as source: /docs/api/ published ``[`WaiverReason::…`]:`` and an
# eighty-seven character `../../../../.ai/repo/adrs/…` path as body text, and the second was
# wide enough on its own to make a phone's page scroll sideways.
#
# Two halves, and neither is sufficient alone. The first is the invariant at the boundary
# where a doc comment becomes an artifact: no description any projection reads may carry a
# link only rustdoc can follow. That is what `majordomus generate` now guarantees, and it
# holds for every consumer, not only the one that was measured.
#
# The second is that the detector still detects. A guard whose subject moved reports clean
# for the wrong reason, so the page check is run against prose whose answer is known: three
# ways markup arrives unrendered must each be named, and correctly rendered prose carrying
# the same characters inside a code span must not be.
. "$ROOT/test/lib.sh"

# ---------------------------------------------------------------- the artifact boundary
# Over the committed artifacts, because they are what every reader outside this repository
# gets. jq walks to the descriptions rather than grepping the file: a `[` elsewhere in the
# document — an array in an example, a character class in a pattern — is not prose.
for artifact in registry openapi; do
  doc="$ROOT/docs/generated/$artifact.json"
  expect_file "$doc"
  jq -r '[paths(type == "string")] as $p
         | reduce $p[] as $q ([]; if ($q | last) == "description" then . + [getpath($q)] else . end)
         | .[]' "$doc" > "$T/$artifact.desc"
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

# ---------------------------------------------------------------- the detector detects
# The page check of site-check, run over prose whose answer is known. Without this the guard
# above is the only thing standing between a new template slot and the defect returning, and
# a template slot is exactly how it arrived.
awk_lib="$ROOT/scripts/lib/site-pages.awk"
expect_file "$awk_lib"
mkdir -p "$T/pub"

# each form on its own page, so a detector that has stopped seeing one is not covered by
# another that still works
cat > "$T/pub/backtick.html" <<'HTML'
<html><body><main><p>The value is `not_executable` here.</p></main></body></html>
HTML
cat > "$T/pub/link.html" <<'HTML'
<html><body><main><p>Distinct from [<code>WaiverReason::TransientState</code>]: durable.</p></main></body></html>
HTML
cat > "$T/pub/emphasis.html" <<'HTML'
<html><body><main><p>what this repository has *published*, so its input is a release</p></main></body></html>
HTML
# rendered correctly: the same characters, inside the elements a renderer produces
cat > "$T/pub/clean.html" <<'HTML'
<html><body><main><p>Distinct from <code>WaiverReason::TransientState</code>: <em>durable</em>.</p>
<pre><code>let v = ["capability"]; // `raw` *markup* is source here</code></pre></main></body></html>
HTML

awk -v pub="$T/pub" -f "$awk_lib" "$T/pub/backtick.html" "$T/pub/link.html" \
  "$T/pub/emphasis.html" "$T/pub/clean.html" > "$T/rows" 2>&1

grep -E '^backtick\.html\trustdoc\t' "$T/rows" > /dev/null || {
  echo "    a literal backtick in prose is markdown no renderer touched, and went unnamed"; cat "$T/rows"; exit 1; }
grep -E '^link\.html\trustdoc\t' "$T/rows" > /dev/null || {
  echo "    a bracket around a code span is an unresolved intra-doc link, and went unnamed"; cat "$T/rows"; exit 1; }
grep -E '^emphasis\.html\trustdoc\t' "$T/rows" > /dev/null || {
  echo "    an emphasis still wearing its asterisks went unnamed"; cat "$T/rows"; exit 1; }
grep -E '^clean\.html\t' "$T/rows" > /dev/null && {
  echo "    correctly rendered prose was reported as unrendered; the check would refuse a"
  echo "    healthy page, which is how a gate gets turned into a warning"; cat "$T/rows"; exit 1; }

# and site-check reports that row rather than collecting it silently
expect_grep 'rows rustdoc' "$ROOT/scripts/site-check"
expect_grep 'no unrendered markup' "$ROOT/scripts/site-check"

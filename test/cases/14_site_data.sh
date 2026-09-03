. "$ROOT/test/lib.sh"
# The site data is a projection of canonical repository files. The generator validates
# before it writes, and refuses to generate from canonical data that has gone inconsistent.

expect_exit 0 "$ROOT/scripts/generate-site-data" --check
expect_grep 'OK   responsibility'
expect_grep 'OK   claim'
expect_grep 'OK   concepts'
expect_grep 'OK   lifecycle'

expect_exit 0 "$ROOT/scripts/generate-site-data"
for f in project responsibilities concepts profiles policy capabilities lifecycle source; do
  [ -s "$ROOT/site/data/generated/$f.json" ] || { echo "    $f.json was not written"; exit 1; }
done

# the version on the site is the version in the code, not a number typed twice
ver="$(sed -n 's/^MJ_VERSION="\([^"]*\)"$/\1/p' "$ROOT/bin/majordomus")"
expect_grep "\"version\": \"$ver\"" "$ROOT/site/data/generated/project.json"

# every responsibility carries the README's own prose, its command and its claims
for id in policy projection state scope profiles handover finish doctor watch; do
  expect_grep "\"id\": \"$id\"" "$ROOT/site/data/generated/responsibilities.json"
done
expect_grep '"index": \{' "$ROOT/site/data/generated/responsibilities.json"

# the outcome vocabulary is read from the code that enforces it
for o in completed partial blocked no_match failed; do
  expect_grep "\"$o\"" "$ROOT/site/data/generated/lifecycle.json"
done

# a profile file is a profile page; nothing is hardcoded
for p in routine implementation debugging deep-work; do
  expect_grep "\"slug\": \"$p\"" "$ROOT/site/data/generated/profiles.json"
done

# one routable page per vocabulary term, plus the section
[ -f "$ROOT/site/content/concepts/_index.md" ] || { echo "    no concepts section"; exit 1; }
for t in policy projection scope handover outcome drift wired; do
  [ -f "$ROOT/site/content/concepts/$t.md" ] || { echo "    no page for concept $t"; exit 1; }
done
# and one per responsibility
for id in policy projection state scope profiles handover finish doctor watch; do
  [ -f "$ROOT/site/content/supervises/$id.md" ] || { echo "    no page for responsibility $id"; exit 1; }
done

# canonical Markdown is imported wrapped so Zola never evaluates it as a template
expect_grep '^\{% raw %\}$' "$ROOT/site/content/docs/cli.md"
expect_grep '^\{% endraw %\}$' "$ROOT/site/content/docs/cli.md"

# the manifest names every input it read, with a hash
expect_grep '"path": "README.md"' "$ROOT/site/data/generated/source.json"
expect_grep '"path": "docs/CLAIMS.yaml"' "$ROOT/site/data/generated/source.json"
expect_grep '"source_hash"' "$ROOT/site/data/generated/source.json"

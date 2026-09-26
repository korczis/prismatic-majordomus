# majordomus-exclusive: builds the site (site-build writes site/static and site/data/build.json of this checkout)
# majordomus-covers: none
# The MCP reference and the schema reference link what they list, on the built pages.
#
#   1. /registry/mcp/ has one row per tool the registry projects (site/data/registry/registry.json
#      .mcp.tools), no more and no fewer, and each row links to a /registry/capabilities/<slug>/
#      page that was built and that is the page of that tool's capability (it names the id);
#   2. /docs/schemas/ has one row per schema file git tracks under share/schemas/ (the README
#      that describes the directory is not a schema), each linking to that file on the forge
#      (project.json repository_url), every kind schema the registry declares is named there,
#      and every kind a row links to is a page the build produced.
#
# The site is built from the committed data (--no-data): what this case holds is that the
# templates render a link for every row of that data. That the data is current is
# scripts/derive-check's verdict, not this case's. --no-css: the styling is irrelevant to a
# link. Skips itself without zola, as the other site cases do.
. "$ROOT/test/lib.sh"
command -v zola >/dev/null 2>&1 || { echo "    zola absent; skipping"; exit 0; }
command -v jq >/dev/null 2>&1 || { echo "    jq absent; skipping"; exit 0; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj532.XXXXXX")"; trap 'rm -rf "$S"' EXIT
P="$S/public"
rc=0; "$ROOT/scripts/site-build" --no-data --no-css --base-url "https://site.invalid" --output-dir "$P" > "$S/build.log" 2>&1 || rc=$?
[ "$rc" = 0 ] || { echo "    site-build exited $rc:"; tail -20 "$S/build.log" | sed 's/^/      /'; exit 1; }

fail=0
note() { echo "    $1"; fail=1; }
REG="$ROOT/site/data/registry/registry.json"
REPO="$(jq -r '.repository_url' "$ROOT/site/data/generated/project.json")"

# one "<name>\t<every href of the row, space-separated>" line per table row; the name is the
# row's first code span, or with `schema` its data-schema-file (and rows without one are skipped)
rows() {
  awk -v mode="$2" 'BEGIN { RS = "<tr" } NR > 1 {
    rec = $0; name = ""; hrefs = ""
    if (mode == "schema") {
      if (!match(rec, /data-schema-file="[^"]*"/)) next
      name = substr(rec, RSTART + 18, RLENGTH - 19)
    } else if (match(rec, /<code[^>]*>[^<]*<\/code>/)) { c = substr(rec, RSTART, RLENGTH); sub(/^<code[^>]*>/, "", c); sub(/<\/code>$/, "", c); name = c }
    while (match(rec, /href="[^"]*"/)) { hrefs = hrefs " " substr(rec, RSTART + 6, RLENGTH - 7); rec = substr(rec, RSTART + RLENGTH) }
    print name "\t" hrefs
  }' "$1"
}
local_path() { printf '%s' "$1" | sed -E 's#^https://site\.invalid##'; }

# ---------------------------------------------------------------- 1. the MCP reference
MCP="$P/registry/mcp/index.html"
if [ ! -f "$MCP" ]; then note "the build has no /registry/mcp/"; else
  # the tools table is the first table; the resources are a list, so every <tr> is a tool
  rows "$MCP" tool | awk -F '\t' '$1 != ""' > "$S/mcp.rows"
  jq -r '.mcp.tools[].name' "$REG" | LC_ALL=C sort > "$S/tools.want"
  cut -f1 "$S/mcp.rows" | LC_ALL=C sort > "$S/tools.got"
  [ -s "$S/tools.want" ] || note "the registry projects no MCP tool; the check is vacuous"
  cmp -s "$S/tools.want" "$S/tools.got" || { note "the rows of /registry/mcp/ are not the registry's tools:"; diff "$S/tools.want" "$S/tools.got" | sed 's/^/      /' | head -10; }
  while IFS="$(printf '\t')" read -r name hrefs; do
    id="$(jq -r --arg n "$name" '.mcp.tools[] | select(.name == $n) | .id' "$REG")"
    cap=""
    for h in $hrefs; do case "$(local_path "$h")" in /registry/capabilities/?*/) cap="$(local_path "$h")" ;; esac; done
    if [ -z "$cap" ]; then note "tool $name: its row links to no /registry/capabilities/<id>/ page"; continue; fi
    if [ ! -f "$P${cap}index.html" ]; then note "tool $name links to $cap, which the build did not produce"; continue; fi
    grep -qF "$id" "$P${cap}index.html" || note "tool $name links to $cap, which is not the page of capability $id"
  done < "$S/mcp.rows"
fi

# ---------------------------------------------------------------- 2. the schema reference
SCH="$P/docs/schemas/index.html"
if [ ! -f "$SCH" ]; then note "the build has no /docs/schemas/"; else
  rows "$SCH" schema > "$S/schema.rows"
  git -C "$ROOT" ls-files share/schemas | grep -v '/README\.md$' | LC_ALL=C sort > "$S/schemas.want"
  cut -f1 "$S/schema.rows" | LC_ALL=C sort > "$S/schemas.got"
  [ -s "$S/schemas.want" ] || note "git tracks no schema file; the check is vacuous"
  cmp -s "$S/schemas.want" "$S/schemas.got" || { note "the rows of /docs/schemas/ are not the tracked schema files:"; diff "$S/schemas.want" "$S/schemas.got" | sed 's/^/      /' | head -10; }
  while IFS="$(printf '\t')" read -r path hrefs; do
    found=0
    for h in $hrefs; do
      if [ "$h" = "$REPO/blob/master/$path" ]; then found=1; continue; fi
      case "$h" in https://site.invalid/*)
        [ -f "$P$(local_path "$h")index.html" ] || note "schema $path links a kind to $(local_path "$h"), which the build did not produce" ;;
      esac
    done
    [ "$found" = 1 ] || note "schema $path: its row does not link to $REPO/blob/master/$path"
    git -C "$ROOT" ls-files --error-unmatch "$path" >/dev/null 2>&1 || note "schema $path: its row names a file git does not track"
  done < "$S/schema.rows"
  for id in $(jq -r '[.kinds[].schema | select(. != null)] | unique[]' "$REG"); do
    grep -qF ">$id<" "$SCH" || note "the kind schema $id is on no row of /docs/schemas/"
  done
fi
[ "$fail" = 0 ]

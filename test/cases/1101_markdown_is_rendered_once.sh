# majordomus-covers: none
# A Markdown body is drawn by one renderer, wherever it is shown
# (project.markdown-is-rendered-once).
#
# Until 2026-10-12 the Cockpit showed a rule or a decision record as the characters it is
# written in, and the UI drew a milestone's prose with a parser of its own. What a record may
# write into a page, where its links lead and whether a table brings an inline style are
# answered by whichever renderer drew it, so there is one. Held here, with no build and no
# server — the served behaviour is apps/majordomus-cli/tests/markdown_bodies.rs:
#
#   1. the parser is named by one source file of the crate, and declared once
#   2. the surfaces that show a body call the renderer: the Cockpit's object page for a body,
#      the UI's prose and lead sentences for a field
#   3. the renderer writes no inline style and never fetches a picture
#   4. every class the renderer writes is a class of the shared primitives, so the look is one
#   5. the diagram library is vendored, not committed, and loaded only for a marked diagram
. "$ROOT/test/lib.sh"

SRC="$ROOT/apps/majordomus-cli/src"
MD="$SRC/markdown.rs"
[ -f "$MD" ] || { echo "    $MD does not exist: there is no renderer"; exit 1; }

# --- 1. one module names the parser
named="$(grep -rl 'pulldown_cmark' "$SRC" | sed "s|$ROOT/||" | LC_ALL=C sort | tr '\n' ' ')"
[ "$named" = "apps/majordomus-cli/src/markdown.rs " ] \
  || { echo "    the Markdown parser is named outside the renderer: $named"; exit 1; }
[ "$(grep -c '^pulldown-cmark' "$ROOT/apps/majordomus-cli/Cargo.toml")" = 1 ] \
  || { echo "    the crate manifest does not declare the parser exactly once"; exit 1; }
grep -q '^pulldown-cmark = { version = "[0-9.]*", default-features = false }' "$ROOT/apps/majordomus-cli/Cargo.toml" \
  || { echo "    the parser is declared with default features: only the parser is wanted"; exit 1; }

# --- 2. the surfaces call it
grep -q 'crate::markdown::render' "$SRC/cockpit/pages.rs" \
  || { echo "    the Cockpit's object page does not draw a body through the renderer"; exit 1; }
grep -q 'markdown::inline' "$SRC/ui/prose.rs" \
  || { echo "    the UI's prose does not go through the renderer"; exit 1; }
grep -q 'crate::markdown::inline' "$SRC/ui/view.rs" \
  || { echo "    the UI's lead sentences do not go through the renderer"; exit 1; }
# and no surface draws a Markdown object's content as a bare code block any more
if grep -n 'card("Content", pre(content))' "$SRC/cockpit/pages.rs" | grep -v 'media_type != "text/markdown"' | grep -qv 'return card'; then
  echo "    the Cockpit still draws an object's content as text whatever its media type"; exit 1
fi

# --- 3. what the renderer never writes (its tests aside, which name these to refuse them)
body="$(awk '/^#\[cfg\(test\)\]/{exit} {print}' "$MD" | grep -v '^ *//')"
for forbidden in '"style"' 'el("img")' 'el("script")' 'el("iframe")' '.raw('; do
  if printf '%s\n' "$body" | grep -qF -- "$forbidden"; then
    echo "    the renderer writes $forbidden"; exit 1
  fi
done

# --- 4. every class it writes is a shared primitive
CSS="$ROOT/share/design/primitives.css"
classes="$(printf '%s\n' "$body" | grep -oE 'mj-(md|table|pre)[a-z0-9-]*' | LC_ALL=C sort -u)"
[ -n "$classes" ] || { echo "    the renderer writes no class at all"; exit 1; }
for class in $classes; do
  case "$class" in
    # a state prefix completed at run time (`mj-md-quote--note`), declared by its full names
    mj-md-quote--) for kind in note tip important warning caution; do
        grep -qF ".mj-md-quote--$kind" "$CSS" || { echo "    $CSS does not style .mj-md-quote--$kind"; exit 1; }
      done ;;
    *) grep -qE "\.$class([ ,:{.>\[]|\$)" "$CSS" \
         || { echo "    the renderer writes .$class and $CSS does not style it"; exit 1; } ;;
  esac
done
for sheet in share/cockpit/cockpit.css share/ui/ui.css; do
  grep -q 'mj-md-heading' "$ROOT/$sheet" \
    || { echo "    $sheet was not rebuilt with the Markdown primitives (scripts/cockpit-assets)"; exit 1; }
done

# --- 5. the diagram library: vendored by the asset build, ignored, asked for by name once
grep -q 'mermaid/dist/mermaid.min.js' "$ROOT/scripts/cockpit-assets" \
  || { echo "    scripts/cockpit-assets does not vendor the diagram library"; exit 1; }
( cd "$ROOT" && git check-ignore -q share/cockpit/vendor/mermaid.min.js ) \
  || { echo "    the vendored diagram library is not ignored: it would be committed"; exit 1; }
grep -q '"mermaid.min.js"' "$SRC/cockpit/mod.rs" \
  || { echo "    the Cockpit does not publish the diagram library's URL"; exit 1; }
grep -q "data-mj-diagram=\"mermaid\"" "$ROOT/share/cockpit/cockpit.js" \
  || { echo "    the Cockpit's script does not look for a marked diagram"; exit 1; }
grep -q "securityLevel: 'strict'" "$ROOT/share/cockpit/cockpit.js" \
  || { echo "    the diagram library is not told to trust nothing"; exit 1; }
exit 0

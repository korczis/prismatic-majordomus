# majordomus-covers: none
# A graph drawing is a drawing, and the choice of layout is the graph's own shape.
#
# The Cockpit draws eight graphs with one layout name — `breadthfirst`, for all of them.
# Breadthfirst puts an entire breadth-first level on one row. The composed graph has 1442
# nodes and one of its levels held almost every one of them, so the drawing was a row 1400
# nodes wide and `cy.fit()` zoomed out until that row fitted the frame: measured in Chrome
# at 1110x630, a horizontal line about one pixel tall. `why` was the same. Nothing failed,
# because the only assertion anyone had written was that a `<canvas>` element existed, and
# Cytoscape makes three of those whatever it draws.
#
# This case holds the three things that stopped that being possible, and it holds them
# where a browser is not needed, because a browser test that needs a server, Chrome and
# 400 kB of vendored library is a test that gets skipped:
#
#   1. the layout is a function of the measured shape, not a constant — proved by running
#      `choose()` and `measure()` out of share/cockpit/graph.js over graph shapes that
#      stand for the real ones, and asserting that the composed graph's shape does not
#      choose breadthfirst while a shallow tree's shape does;
#   2. no colour is decided in the viewer — proved by the design gate, with graph.js no
#      longer exempted from it by path;
#   3. a kind's colour is stable when another kind is declared, and does not depend on the
#      order the kinds arrive in.
#
# The browser half — that the drawing actually occupies the frame — is asserted by
# scripts/lib/cockpit-probe.mjs against a running server (`graph` and `graph-scale`), which
# is where a real renderer exists to ask.
. "$ROOT/test/lib.sh"

command -v node >/dev/null 2>&1 || {
  echo "    node is not available; this case runs the viewer's own functions"
  exit 1
}

VIEWER="$ROOT/share/cockpit/graph.js"
expect_file "$VIEWER"

S="$(mktemp -d)"
trap 'rm -rf "$S"' EXIT

# The viewer is an ES module that imports ./cockpit.js and touches `document` at load, so
# it cannot simply be imported here. Its decision functions are pure and self-contained;
# they are lifted out by name and run. Lifting by name is deliberate: rename or delete
# `choose` and this case fails loudly rather than passing over a file it no longer reads.
node - "$VIEWER" > "$S/out.txt" 2> "$S/err.txt" <<'NODE' || { echo "    the viewer's decision functions did not run:"; sed 's/^/    | /' "$S/err.txt"; exit 1; }
const fs = require('fs');
const source = fs.readFileSync(process.argv[2], 'utf8');

// lift the named function declarations this case is about, with their bodies
function lift(name) {
  const at = source.indexOf(`function ${name}(`);
  if (at < 0) throw new Error(`share/cockpit/graph.js declares no function ${name}`);
  let depth = 0;
  let started = false;
  for (let i = at; i < source.length; i += 1) {
    if (source[i] === '{') { depth += 1; started = true; }
    else if (source[i] === '}') { depth -= 1; if (started && depth === 0) return source.slice(at, i + 1); }
  }
  throw new Error(`could not read the body of ${name}`);
}

// eslint-disable-next-line no-new-func
const g = new Function(`${lift('choose')}\n${lift('measure')}\n${lift('degrees')}\n${lift('slot')}\n${lift('size')}\nreturn { choose, measure, degrees, slot, size };`)();

// A graph of the given shape: `levels` layers of `across` nodes each, wired downward, plus
// `extra` edges wired across so density can be dialled independently of the layering.
function shaped(levels, across, extra) {
  const nodes = [];
  const edges = [];
  for (let l = 0; l < levels; l += 1) {
    for (let i = 0; i < across; i += 1) nodes.push({ id: `n${l}_${i}`, kind: 'thing' });
  }
  for (let l = 1; l < levels; l += 1) {
    for (let i = 0; i < across; i += 1) {
      edges.push({ source: `n${l - 1}_${i % across}`, target: `n${l}_${i}`, kind: 'to' });
    }
  }
  for (let e = 0; e < extra; e += 1) {
    const a = nodes[e % nodes.length].id;
    const b = nodes[(e * 7 + 3) % nodes.length].id;
    if (a !== b) edges.push({ source: a, target: b, kind: 'also' });
  }
  return { nodes, edges, node_kinds: { thing: 'a thing' } };
}

function layoutOf(graph) {
  const shape = g.measure(graph, g.degrees(graph));
  const chosen = g.choose(shape);
  return { name: chosen.name, shape };
}

const said = [];

// 1. the eight-node graph: a ring, because everything fits on one at a readable size
said.push(`tiny ${layoutOf(shaped(2, 4, 0)).name}`);

// 2. a shallow tree, 6 levels of 8: breadthfirst, which is what breadthfirst is for
said.push(`tree ${layoutOf(shaped(6, 8, 0)).name}`);

// 3. the shape that broke: one level about 1400 nodes wide. Whatever this chooses, it is
//    not breadthfirst; that is the defect, stated as a test.
const huge = shaped(2, 720, 2000);
const big = layoutOf(huge);
said.push(`huge ${big.name} widest=${big.shape.widest} nodes=${big.shape.nodes}`);

// 4. a hub-shaped graph of a few hundred nodes: concentric, which ranks by degree. No graph
//    this repository derives currently lands here — every one is either under 400 nodes or
//    over 900 — so this is the only thing that exercises that branch, and a branch nothing
//    exercises is a branch nobody notices breaking.
const hub = shaped(2, 300, 0);
for (let i = 0; i < 600; i += 1) hub.edges.push({ source: 'n0_0', target: `n1_${i % 300}`, kind: 'hub' });
said.push(`hub ${layoutOf(hub).name}`);

// 5. a kind's colour does not move when another kind is declared, and does not depend on
//    the order the kinds arrive in
const before = ['capability', 'rule', 'adr'];
const after = ['adr', 'capability', 'brand-new-kind', 'rule'];
const slots = (kinds) => kinds.map((k) => `${k}=${g.slot(k, 8)}`).join(' ');
said.push(`slots-before ${slots(before)}`);
said.push(`slots-after ${slots(after)}`);

// 6. a node's size follows its degree
said.push(`size ${g.size(0, 40)} ${g.size(10, 40)} ${g.size(40, 40)}`);

console.log(said.join('\n'));
NODE
LAST_OUT="$(cat "$S/out.txt")"
sed 's/^/    /' "$S/out.txt"

echo "  a graph's shape chooses its layout"
expect_grep '^tiny circle$'
expect_grep '^tree breadthfirst$'
grep -qE '^huge breadthfirst' "$S/out.txt" && {
  echo '    a graph whose widest breadth-first level is hundreds of nodes still chose'
  echo '    breadthfirst. That layout puts one level on one row, so fit() zooms out until'
  echo '    the row fits the frame and the drawing is a horizontal line one pixel tall.'
  exit 1
}
# grid, at this size: `concentric` was tried and measured — 1452 nodes gave a disc about
# 19000px across, `fit()` hit minZoom, and a node rendered under one pixel, which is the
# same defect in a different shape and would have passed an area check
expect_grep '^huge (grid|concentric|cose|circle) '
# and the branch no real graph reaches is reached by this one
expect_grep '^hub concentric$'

echo "  a node's diameter follows its degree"
# smallest < middling < largest, and none of them the old constant 12 for every node
size_line="$(sed -n 's/^size //p' "$S/out.txt")"
set -- $size_line
[ "$1" -lt "$2" ] && [ "$2" -lt "$3" ] || {
  printf '    degree did not order the sizes: %s\n' "$size_line"
  exit 1
}

echo "  a kind's colour is stable when another kind is declared"
for kind in capability rule adr; do
  b="$(sed -n 's/^slots-before //p' "$S/out.txt" | tr ' ' '\n' | sed -n "s/^$kind=//p")"
  a="$(sed -n 's/^slots-after //p' "$S/out.txt" | tr ' ' '\n' | sed -n "s/^$kind=//p")"
  [ -n "$b" ] && [ "$a" = "$b" ] || {
    printf '    %s took series slot %s before a new kind was declared and %s after.\n' "$kind" "$b" "$a"
    echo '    A kind is coloured by a hash of its own name for exactly this reason: indexing'
    echo '    into the kind list recoloured every kind after the one that was added.'
    exit 1
  }
done

echo "  the viewer decides no colour of its own"
# the same question the design gate asks, asked here of this one file, so the case fails on
# the defect itself and not only through a gate that could be exempted again
expect_no_grep '#([0-9a-fA-F]{8}|[0-9a-fA-F]{6})\b|[:,][[:space:]]*#[0-9a-fA-F]{3}\b|\b(rgba?|hsla?|oklch|oklab|lch)\(' "$VIEWER"
expect_grep '[-][-]mj-series-' "$VIEWER"

echo "  and the gate that asks it of every surface no longer exempts this file by path"
# the exemption was a `-e` filter in first_party(); the prose above it may still discuss
# why it is gone, and should. What must not come back is the filter.
expect_no_grep "\-e '\^share/cockpit/graph" "$ROOT/scripts/ci/design-check"

echo "  the declaration owns the series the viewer reads"
expect_grep '^  series-1:' "$ROOT/share/design/tokens.yaml"
expect_grep '[-][-]mj-series-1:' "$ROOT/share/design/surface.css"

echo "  no token reaches the drawing library in a syntax it cannot read"
# Cytoscape's parser does not know oklch, which is what the declaration's palette is: handed
# one it logs that the property is invalid and falls back to black. Every token this file read
# was landing that way, unnoticed, because the node fill was the one colour the old code
# computed itself. The conversion is a measurement, not a choice, and it must stay.
expect_grep 'function channels\(' "$VIEWER"
expect_grep 'getImageData' "$VIEWER"

echo "  the type size is the declared scale, not a raw number"
expect_no_grep "'font-size': [0-9]" "$VIEWER"
expect_grep '[-][-]text-meta' "$VIEWER"

echo "  the probe can tell a drawing from a hairline"
# the assertion that let this live: three canvas layers reported a healthy drawing over a
# one-pixel line. What replaces it must measure an area.
PROBE="$ROOT/scripts/lib/cockpit-probe.mjs"
expect_grep 'MIN_DRAWN' "$PROBE"
expect_grep 'mjGraph.extent\(\)' "$PROBE"
# and it asks the question of the largest graph, which is the only one that ever had
# the defect: a probe that only looks at the small graph cannot see it
expect_grep 'graphAtScale' "$PROBE"

echo "    every claim held"

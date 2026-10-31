# majordomus-covers: none
# A graph, a chart or a diagram is something a reader can act on, never a picture
# (project.a-visualisation-is-interactive).
#
# The UI's and the Cockpit's relationship graphs take a press on a node to its record; the
# overview's activity field and the 3-D topology draw the same records and bind nothing, and
# nothing said which kind the next drawing had to be. Held here, on copies, with no build:
#
#   1. the gate finds this repository's drawings, passes on them, and names the two pictures
#      the baseline accepts
#   2. a new script that draws and binds nothing fails by name
#   3. a baseline line whose drawing became interactive, or is gone, fails until it is removed
#   4. a tree with no drawing at all cannot pass: the gate says it measured nothing
#   5. a diagram drawn from a Markdown body carries links the server derived and keeps its
#      source one press away; the script holds no route of its own
#   6. the gate is declared in the CI model, in every plan
. "$ROOT/test/lib.sh"

GATE="$ROOT/scripts/ci/visualisation-check"
BASE=".ai/repo/visualisation-baseline.txt"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj1102.XXXXXX")"; trap 'rm -rf "$S"' EXIT

# --- 1. this repository
out="$("$GATE" 2>&1)" || { echo "    the gate fails on this tree:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
listed="$("$GATE" --list)"
for script in share/ui/ui.js share/cockpit/graph.js share/cockpit/cockpit.js; do
  printf '%s\n' "$listed" | grep -qE "^interactive +$script\$" \
    || { echo "    $script is not found as an interactive drawing:"; printf '%s\n' "$listed" | sed 's/^/      /'; exit 1; }
done
pictures="$(printf '%s\n' "$listed" | awk '$1 == "picture" {print $2}' | LC_ALL=C sort)"
accepted="$(grep -vE '^[[:space:]]*(#|$)' "$ROOT/$BASE" | LC_ALL=C sort)"
[ "$pictures" = "$accepted" ] \
  || { echo "    the baseline is not the pictures the gate finds:"; echo "      found:    $pictures"; echo "      accepted: $accepted"; exit 1; }

# a fixture: the served scripts and the baseline, and nothing else the gate reads
fixture() {
  rm -rf "$S/t"; mkdir -p "$S/t/share/ui" "$S/t/share/cockpit" "$S/t/.ai/repo"
  cp "$ROOT"/share/ui/*.js "$S/t/share/ui/"
  cp "$ROOT"/share/cockpit/*.js "$S/t/share/cockpit/"
  cp "$ROOT/$BASE" "$S/t/$BASE"
}
judge() { MJ_ROOT="$S/t" "$GATE" 2>&1; }

# --- 2. a new picture
fixture
printf '%s\n' "import { vendor } from './cockpit.js';" "vendor('chart.min.js', 'Chart').then((Chart) => new Chart(document.querySelector('canvas'), {}));" > "$S/t/share/cockpit/burndown.js"
if out="$(judge)"; then echo "    a new drawing that binds nothing passed:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; fi
printf '%s\n' "$out" | grep -q '^FAIL visualisation share/cockpit/burndown.js' \
  || { echo "    the new picture is not named:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
# the same drawing with a way from its marks to their records is accepted
printf '%s\n' "canvas.addEventListener('click', (event) => window.location.assign(hrefAt(event)));" >> "$S/t/share/cockpit/burndown.js"
judge >/dev/null || { echo "    a drawing that binds a way through is refused:"; judge | sed 's/^/      /'; exit 1; }
# a script that only talks about loading a library draws nothing
fixture
printf '%s\n' "// once this calls vendor('chart.min.js') it will need a handler" "export const nothing = 1;" > "$S/t/share/cockpit/notes.js"
judge >/dev/null || { echo "    a comment about a library is read as a drawing:"; judge | sed 's/^/      /'; exit 1; }

# --- 3. the baseline is a ratchet
fixture
printf '%s\n' "canvas.addEventListener('click', pick);" >> "$S/t/share/cockpit/topology.js"
if out="$(judge)"; then echo "    a baseline line for a drawing that became interactive passed"; exit 1; fi
printf '%s\n' "$out" | grep -q "lists share/cockpit/topology.js, which is interactive now or gone" \
  || { echo "    the paid debt is not asked to leave the baseline:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
fixture
rm "$S/t/share/cockpit/activity.js"
if judge >/dev/null; then echo "    a baseline line for a drawing that is gone passed"; exit 1; fi
fixture
grep -v 'share/cockpit/activity.js$' "$S/t/$BASE" > "$S/t/$BASE.new" && mv "$S/t/$BASE.new" "$S/t/$BASE"
if out="$(judge)"; then echo "    a picture removed from the baseline but still a picture passed"; exit 1; fi
printf '%s\n' "$out" | grep -q '^FAIL visualisation share/cockpit/activity.js' \
  || { echo "    the unlisted picture is not named"; exit 1; }

# --- 4. nothing found is not a pass
rm -rf "$S/t"; mkdir -p "$S/t/share/ui" "$S/t/share/cockpit" "$S/t/.ai/repo"
: > "$S/t/$BASE"
printf '%s\n' "export const quiet = 1;" > "$S/t/share/ui/ui.js"
set +e; judge >/dev/null; rc=$?; set -e
[ "$rc" = 12 ] || { echo "    a tree with no drawing answered $rc, not 12: the gate would pass by finding nothing"; exit 1; }
rm "$S/t/$BASE"
set +e; judge >/dev/null; rc=$?; set -e
[ "$rc" = 12 ] || { echo "    a missing baseline answered $rc, not 12"; exit 1; }

# --- 5. a diagram from a Markdown body: derived links, and the source kept
MD="$ROOT/apps/majordomus-cli/src/markdown.rs"
JS="$ROOT/share/cockpit/cockpit.js"
grep -q '"data-mj-links"' "$MD" || { echo "    the renderer derives no links for a diagram's nodes"; exit 1; }
grep -q 'dataset.mjLinks' "$JS" || { echo "    the script does not read the links the server derived"; exit 1; }
grep -q 'sourceToggle(holder, figure, source)' "$JS" || { echo "    a drawn diagram does not keep its source"; exit 1; }
if awk '/^async function drawDiagrams/,/^drawDiagrams\(\);/' "$JS" | grep -qE "['\"]/(ui|cockpit)/"; then
  echo "    the diagram script writes a route of its own instead of reading the derived links"; exit 1
fi

# --- 6. declared, in every plan
awk '/^  - id: visualisation-check$/{f=1; next} f && /^  - id: /{f=0} f' "$ROOT/.ai/repo/ci/gates.yaml" > "$S/gate"
grep -q '^    runs: scripts/ci/visualisation-check$' "$S/gate" || { echo "    the CI model does not run the gate"; exit 1; }
grep -q '^    always: true$' "$S/gate" || { echo "    the gate is not in every plan"; exit 1; }
exit 0

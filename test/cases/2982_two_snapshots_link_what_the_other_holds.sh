# majordomus-covers: none
# The UI's snapshot and the Cockpit's link each other by the pages each holds (I2495, ADR 0147):
#
#   before   with no Cockpit snapshot, a link from the UI into the Cockpit leads to the UI's
#            page about the live surface, carrying the path it meant
#   after    once the Cockpit's snapshot is written, the UI's files lead those links to the
#            Cockpit's pages, and every such link names a page the Cockpit's snapshot holds
#   first    the capabilities the UI cites are in the Cockpit's snapshot although its bound
#            is far below the registry: what the UI links to is read first
#   model    the publication model declares both snapshots, so the deploy, the public probe
#            and the live gate hold both
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is not installed"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install
U=target/web/ui-snapshot
C=target/web/cockpit-snapshot
into() { grep -rhoE "$1" "$U" --include=index.html | sort -u; }

# ---------------------------------------------------------------- before
"$RB" web report ui-snapshot --limit 30 --budget-seconds 0 >/dev/null 2> err.txt \
  || { echo "    the UI's snapshot was not written:"; tail -5 err.txt | sed 's/^/    | /'; exit 1; }
[ -z "$(into 'href="/cockpit[^"]*"')" ] || { echo "    the UI links the Cockpit's mount before any snapshot of it exists:"; into 'href="/cockpit[^"]*"' | head -3 | sed 's/^/    | /'; exit 1; }
into 'live/index.html#/cockpit/capabilities/[^"]*' > wanted.txt
[ -s wanted.txt ] || { echo "    the UI's source lines lead to no capability of the Cockpit"; exit 1; }

# ---------------------------------------------------------------- after
"$RB" web report cockpit-snapshot --limit 120 >/dev/null 2> err.txt \
  || { echo "    the Cockpit's snapshot was not written:"; tail -5 err.txt | sed 's/^/    | /'; exit 1; }
into 'href="/cockpit[^"]*"' | sed 's/^href="//; s/"$//' > led.txt
[ -s led.txt ] || { echo "    writing the Cockpit's snapshot led none of the UI's links to it"; exit 1; }
while IFS= read -r link; do
  path="${link%%[#?]*}"; rest="${path#/cockpit}"; rest="${rest#/}"
  [ -f "$C/${rest}index.html" ] || [ -f "$C/$rest/index.html" ] \
    || { echo "    the UI links $link, which the Cockpit's snapshot does not hold"; exit 1; }
done < led.txt

# ---------------------------------------------------------------- first
while IFS= read -r want; do
  target="${want#live/index.html#}"
  jq -e --arg t "$target" '.pages | has($t)' "$C/snapshot.json" >/dev/null \
    || { echo "    the UI cites $target, and the Cockpit's snapshot of 120 pages left it out"; exit 1; }
done < wanted.txt
[ -z "$(into 'live/index.html#/cockpit/capabilities/[^"]*')" ] \
  || { echo "    a capability the Cockpit's snapshot holds is still led to the live page:"; into 'live/index.html#/cockpit/capabilities/[^"]*' | head -3 | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- model
for surface in ui cockpit; do
  grep -qE "^    - surface: $surface\$" "$ROOT/.ai/repo/ci/pages.yaml" \
    || { echo "    the publication model declares no snapshot of the $surface surface"; exit 1; }
done
grep -q 'verify-ui --surface cockpit --commit' "$ROOT/.github/workflows/pages.yml" \
  || { echo "    the pages workflow does not ask the public site for the Cockpit after a deploy"; exit 1; }

echo "    the UI's links into the Cockpit lead to the live page until the Cockpit has a snapshot, and to its pages after"

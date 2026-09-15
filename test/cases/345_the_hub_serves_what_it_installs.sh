# majordomus-covers: none
# A hub that cannot draw a graph is not installed.
#
# The Cockpit's graph pages are drawn by four files that this repository deliberately does not
# commit: share/cockpit/vendor/{cytoscape,three.module,three.core,p5}.min.js are copied from
# pinned npm packages by scripts/cockpit-assets, which says so in its own header. scripts/mesh-hub
# compiled the executable, wrote the unit, started it and waited for the mesh to answer — and
# never ran `npm ci` or scripts/cockpit-assets. So a hub installed from a clean checkout reported
# itself healthy while /cockpit/graphs/* returned 404 for every page.
#
# That is not hypothetical: on 2026-09-14 the lundra hub was in exactly that state, 44 browser
# findings, and it was repaired by copying four files onto the machine by hand — untracked, so the
# next reinstall loses them and nobody would know why.
#
# The install path cannot be driven here (it needs a user systemd, and the script refuses on
# macOS by design), so what this case drives is the half that decides: the verification, against a
# tree with the files and a tree without them, plus the wiring that makes install and deploy ask.
. "$ROOT/test/lib.sh"

HUB="$ROOT/scripts/mesh-hub"
[ -x "$HUB" ] || { echo "    scripts/mesh-hub is missing"; exit 1; }

# ---------------------------------------------------------------- 1. the premise: not committed
# The whole defect rests on these files being absent from a fresh clone. If someone commits them
# this case must change meaning deliberately rather than keep passing for a reason that has gone.
for f in cytoscape.min.js three.module.min.js three.core.min.js p5.min.js; do
  git -C "$ROOT" ls-files --error-unmatch "share/cockpit/vendor/$f" >/dev/null 2>&1 \
    && { echo "    share/cockpit/vendor/$f is tracked now; this case was written for files a clone does not get"; exit 1; }
done
echo "    the graph assets are not in the clone: a hub must build them"

# ---------------------------------------------------------------- 2. a tree without them is refused
R="$T/hub"; mkdir -p "$R/scripts" "$R/share/cockpit/vendor" "$R/apps/majordomus-cli"
cp "$HUB" "$R/scripts/mesh-hub"; chmod +x "$R/scripts/mesh-hub"
rc=0; ( cd "$R" && ./scripts/mesh-hub assets --verify ) > "$T/out.txt" 2>&1 || rc=$?
[ "$rc" = 10 ] || { echo "    a hub with no graph assets answered $rc, not 10 (a refusal)"; cat "$T/out.txt"; exit 1; }
for f in cytoscape.min.js three.module.min.js three.core.min.js p5.min.js; do
  grep -q "$f" "$T/out.txt" || { echo "    the refusal does not name $f; 'assets missing' sends somebody hunting"; cat "$T/out.txt"; exit 1; }
done
echo "    a hub without the graph assets is refused, and every missing file is named"

# ---------------------------------------------------------------- 3. a tree with them passes
for f in cytoscape.min.js three.module.min.js three.core.min.js p5.min.js; do
  printf '// a vendored file, enough of one for this case\n' > "$R/share/cockpit/vendor/$f"
done
rc=0; ( cd "$R" && ./scripts/mesh-hub assets --verify ) > "$T/out2.txt" 2>&1 || rc=$?
[ "$rc" = 0 ] || { echo "    a hub holding every asset was still refused ($rc)"; cat "$T/out2.txt"; exit 1; }
echo "    a hub that has them is accepted"

# ---------------------------------------------------------------- 4. install and deploy ask
# The two paths that put a hub on a machine. Read from the script because systemd is not here to
# run them: what is asserted is that neither reaches its service without the assets being settled.
awk '/^  install\)/{f=1} f{print} /^    ;;/{if(f) exit}' "$HUB" | grep -q 'assets' \
  || { echo "    install does not build or verify the Cockpit assets: a fresh hub would serve 404 graph pages"; exit 1; }
awk '/^  deploy\)/{f=1} f{print} /^    ;;/{if(f) exit}' "$HUB" | grep -q 'assets_build' \
  || { echo "    deploy does not rebuild the Cockpit assets: an update would leave yesterday's files"; exit 1; }
echo "    install and deploy both settle the assets before the service is asked to serve"

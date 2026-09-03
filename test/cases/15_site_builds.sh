. "$ROOT/test/lib.sh"
# The site builds and passes its own checks. Skipped when Zola is absent, because the
# CLI has no business requiring a static site generator to be installed.
command -v zola >/dev/null 2>&1 || { echo "    zola absent; skipping"; exit 0; }
[ -d "$ROOT/node_modules/flowbite" ] || { echo "    node_modules absent; skipping"; exit 0; }

expect_exit 0 "$ROOT/scripts/site-build"
expect_grep 'site-build: ok'

# the routes the information architecture promises
for r in / /getting-started/ /supervises/ /commands/ /concepts/ /profiles/ /policy/ /guarantees/ /architecture/ /docs/; do
  [ -f "$ROOT/public${r}index.html" ] || { echo "    route $r was not generated"; exit 1; }
done
for id in policy projection state scope profiles handover finish doctor watch; do
  [ -f "$ROOT/public/supervises/$id/index.html" ] || { echo "    no page for responsibility $id"; exit 1; }
done
n="$(find "$ROOT/public/concepts" -name index.html | wc -l | tr -d ' ')"
[ "$n" -ge 15 ] || { echo "    only $n concept routes"; exit 1; }
# one page per subcommand, one per profile
for c in init doctor start check watch update handover finish version; do
  [ -f "$ROOT/public/commands/$c/index.html" ] || { echo "    no page for command $c"; exit 1; }
  grep -q 'Tested in CI' "$ROOT/public/commands/$c/index.html" \
    || { echo "    the $c page does not show its CI evidence"; exit 1; }
done
for p in routine implementation debugging deep-work; do
  [ -f "$ROOT/public/profiles/$p/index.html" ] || { echo "    no page for profile $p"; exit 1; }
  grep -q 'Tested in CI' "$ROOT/public/profiles/$p/index.html" \
    || { echo "    the $p page does not show its CI evidence"; exit 1; }
done

expect_exit 0 "$ROOT/scripts/site-check"
expect_grep 'site-check: ok'

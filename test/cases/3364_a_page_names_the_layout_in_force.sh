# The site's own pages name the layout in force, `.ai/`, never the retired `.majordomus/`
# (I2967). The published /policy page said "`.majordomus/policy.yaml` is the source of truth",
# a layout every command now refuses (README: the old layout is refused with the command that
# migrates it). The retired path is history on the migration and plan pages, which live in
# sections; it is a falsehood on a top-level page, in the site's hand-written data or in a
# template, which describe the product as it is.
. "$ROOT/test/lib.sh"
files="$(ls "$ROOT"/site/content/*.md "$ROOT"/site/data/*.toml 2>/dev/null; find "$ROOT/site/templates" -type f -name '*.html' 2>/dev/null)"
[ -n "$files" ] || { echo "    no site page, data file or template was found: the read is wrong"; exit 1; }
# shellcheck disable=SC2086
hits="$(grep -nF '.majordomus/' $files 2>/dev/null || true)"
if [ -n "$hits" ]; then
  echo "    a page that describes the product as it is names the retired .majordomus/ layout:"
  printf '%s\n' "$hits" | sed "s|$ROOT/||; s/^/      /"
  exit 1
fi
# the page this case was written for names the layout in force
grep -qF '`.ai/repo/policy.yaml` is the source of truth' "$ROOT/site/content/policy.md" \
  || { echo "    site/content/policy.md does not name .ai/repo/policy.yaml as the source of truth"; exit 1; }
exit 0

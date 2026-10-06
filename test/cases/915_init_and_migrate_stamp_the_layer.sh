# majordomus-covers: init migrate
# The commands that write a layer stamp it with the tool version they wrote it for (I1980):
# init on a new layer, migrate when it moves a pre-.ai layout to .ai/. The skeleton they copy
# carries no version, because a version is written by hand in one place only; the stamp is
# this executable's, beside the schema line, and an existing manifest is never overwritten.
. "$ROOT/test/lib.sh"

tool="$("$MJ" version | awk '{print $2}')"
written() { sed -n 's/^written_for: *"\{0,1\}\([^"]*\)"\{0,1\} *$/\1/p' "$1"; }

echo "    the skeleton carries no version of its own"
expect_no_grep '^written_for:' "$ROOT/share/skeleton/ai/manifest.yaml"

echo "    init stamps the layer it creates"
"$MJ" init >/dev/null
[ "$(written .ai/manifest.yaml)" = "$tool" ] || { echo "    init wrote '$(written .ai/manifest.yaml)'"; exit 1; }
[ "$(sed -n '/^schema:/{n;p;}' .ai/manifest.yaml)" = "written_for: \"$tool\"" ] \
  || { echo "    the stamp is not beside the schema:"; cat .ai/manifest.yaml; exit 1; }

echo "    init --extend on an existing layer leaves its manifest alone"
sed -i.bak 's/^written_for: .*/written_for: "0.0.0"/' .ai/manifest.yaml && rm -f .ai/manifest.yaml.bak
"$MJ" init --extend >/dev/null
[ "$(written .ai/manifest.yaml)" = "0.0.0" ] || { echo "    init --extend rewrote the manifest"; exit 1; }

echo "    migrate stamps the layer it moves a pre-.ai layout into"
d="$(mktemp -d "${TMPDIR:-/tmp}/mj-915.XXXXXX")"
(
  cd "$d" && git init -q . && git config user.email t@example.com && git config user.name t \
    && git commit -q --allow-empty -m init
  "$MJ" init >/dev/null
  mkdir -p .majordomus
  for s in policy.yaml profiles prompts project; do [ -e ".ai/repo/$s" ] && mv ".ai/repo/$s" ".majordomus/$s"; done
  rm -rf .ai
  git add -A && git commit -qm legacy
  "$MJ" migrate >/dev/null
  [ "$(written .ai/manifest.yaml)" = "$tool" ] || { echo "    migrate wrote '$(written .ai/manifest.yaml)'"; exit 1; }
)
rm -rf "$d"

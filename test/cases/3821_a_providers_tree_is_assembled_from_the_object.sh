# majordomus-covers: capability:deploy.slug
# The tree a provider receives is assembled from the deployment object at one commit (I3821,
# ADR 0219 decisions 3 and 4).
#
# The first Gigalixir deployment was a slug assembled by hand: a file list, a Procfile and an
# executable that restated what the deployment object already said, and an executable built
# at another commit than the tree it served. `majordomus deploy slug` assembles it instead,
# outside the checkout: the object's build inputs except the package the executable is
# compiled from, read from git at the commit; the provider's generated files at the slug's
# root with the hooks executable; the buildpack line; the executable; and a record naming the
# commit. This case assembles a fixture slug and proves its exact top-level list, its modes,
# its buildpack line and its recorded commit; that an edit nobody committed never reaches it;
# and that an out directory inside the checkout, and a deployment that names no provider, are
# refused with nothing written.
. "$ROOT/test/lib.sh"
. "$ROOT/lib/sha256.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

# the slug is written outside the checkout, so its directory is not the fixture's
ELSEWHERE="$(mktemp -d "${TMPDIR:-/tmp}/mj-slug.XXXXXX")"
trap 'rm -rf "$ELSEWHERE"' EXIT

"$MJ" init >/dev/null
printf '[toolchain]\nchannel = "1.98.1"\n' > rust-toolchain.toml

write_deployment() {   # provider block (may be empty)
  cat > .ai/repo/deployments/hosted.yaml <<MD
schema: deployment/v1
kind: deployment
id: hosted
title: The hosted deployment
application: hosted
build:
  package: majordomus-cli
  binary: majordomus
  profile: release
  inputs:
    - apps/majordomus-cli
    - share
    - .ai
    - bin
    - README.md
listen:
  port: 8080
  interface: all
health:
  liveness: /api/v1/live
  readiness: /api/v1/ready
resources:
  cpu_kind: shared
  cpus: 1
  memory_mb: 256
machines:
  count: 1
  min_running: 0
  autostart: true
  autostop: true
region: fra
$1
MD
}

mkdir -p apps/majordomus-cli/src share bin
printf '// compiled into the executable, never shipped beside it\n' > apps/majordomus-cli/src/lib.rs
printf 'shared\n' > share/data.txt
printf '#!/bin/sh\necho tool\n' > bin/tool; chmod 755 bin/tool
printf 'readme\n' > README.md
write_deployment 'provider:
  name: gigalixir
  app: hosted-app
  stack: gigalixir-24
  url: https://hosted-app.gigalixirapp.com'
expect_exit 0 "$RB" generate --repo . deployment
git add -A >/dev/null; git commit -qm "a hosted deployment and its provider files" >/dev/null
HEAD="$(git rev-parse HEAD)"

# the executable built at that commit; a script stands in for it
printf '#!/bin/sh\necho majordomus\n' > "$ELSEWHERE/exe"

# an edit nobody committed must not reach the slug
printf 'edited, not committed\n' > share/data.txt

# ---------------------------------------------------------------- the slug is exactly the object
SLUG="$ELSEWHERE/slug"
run_quiet "$ELSEWHERE/slug.err" "$RB" deploy slug --repo . --deployment hosted --executable "$ELSEWHERE/exe" \
  --out "$SLUG" --format json > "$ELSEWHERE/slug.json"
want='.ai .buildpacks .majordomus Procfile README.md bin share'
names() { find "$1" -mindepth 1 -maxdepth 1 | sed 's|.*/||' | LC_ALL=C sort | tr '\n' ' ' | sed 's/ $//'; }
have="$(names "$SLUG")"
[ "$have" = "$want" ] || { echo "    the slug's top level is '$have', expected '$want'"; exit 1; }
[ ! -e "$SLUG/apps" ] || { echo "    the package the executable is compiled from was shipped"; exit 1; }
[ ! -e "$SLUG/deploy" ] || { echo "    the provider files were not moved to the slug's root"; exit 1; }
[ "$(cat "$SLUG/share/data.txt")" = shared ] || { echo "    the slug carries the working tree, not the commit"; exit 1; }
[ "$(names "$SLUG/bin")" = 'compile detect release tool' ] \
  || { echo "    the slug's bin/ is not the hooks and the tracked tool:"; ls -l "$SLUG/bin"; exit 1; }

# ---------------------------------------------------------------- the hooks and the executable run
for f in bin/detect bin/compile bin/release bin/tool .majordomus/bin/majordomus; do
  [ "$(file_mode "$SLUG/$f")" = 755 ] || { echo "    $f is mode $(file_mode "$SLUG/$f"), not 755"; exit 1; }
done
for f in Procfile .buildpacks share/data.txt; do
  [ "$(file_mode "$SLUG/$f")" = 644 ] || { echo "    $f is mode $(file_mode "$SLUG/$f"), not 644"; exit 1; }
done
cmp -s "$ELSEWHERE/exe" "$SLUG/.majordomus/bin/majordomus" || { echo "    the executable in the slug is not the one named"; exit 1; }
cmp -s deploy/gigalixir/Procfile "$SLUG/Procfile" || { echo "    the slug's Procfile is not the generated one"; exit 1; }

# ---------------------------------------------------------------- the buildpack line is the pinned one
pin="$(sed -n 's|^    "\(https://github.com/kr/heroku-buildpack-inline#[0-9a-f]*\)";$|\1|p' "$ROOT/apps/majordomus-cli/src/deploy/render.rs")"
[ -n "$pin" ] || { echo "    the pinned buildpack could not be read from render.rs"; exit 1; }
[ "$(cat "$SLUG/.buildpacks")" = "$pin" ] && [ "$(wc -l < "$SLUG/.buildpacks" | tr -d ' ')" = 1 ] \
  || { echo "    .buildpacks is not exactly the pinned buildpack line:"; cat "$SLUG/.buildpacks"; exit 1; }

# ---------------------------------------------------------------- the commit is recorded
src="$SLUG/.majordomus/SOURCE.json"
[ "$(jq -r .commit "$src")" = "$HEAD" ] || { echo "    SOURCE.json does not record $HEAD:"; cat "$src"; exit 1; }
[ "$(jq -r .deployment "$src")" = hosted ] && [ "$(jq -r .provider "$src")" = gigalixir ] \
  || { echo "    SOURCE.json does not name the deployment and the provider:"; cat "$src"; exit 1; }
[ "$(jq -r .executable_sha256 "$src")" = "$(mj_sha256_hex "$ELSEWHERE/exe")" ] \
  || { echo "    SOURCE.json does not carry the executable's digest"; exit 1; }
# what a publisher reads to push it and to ask it what it serves
[ "$(jq -r '.provider.app' "$ELSEWHERE/slug.json")" = hosted-app ] \
  && [ "$(jq -r '.provider.url + .liveness' "$ELSEWHERE/slug.json")" = https://hosted-app.gigalixirapp.com/api/v1/live ] \
  || { echo "    the answer does not carry the provider's application and URL:"; cat "$ELSEWHERE/slug.json"; exit 1; }

# ---------------------------------------------------------------- refused, and nothing written
expect_exit 15 "$RB" deploy slug --repo . --deployment hosted --executable "$ELSEWHERE/exe" --out "$PWD/slug"
expect_grep 'inside the checkout'
[ ! -e slug ] || { echo "    a refused slug was written into the checkout"; exit 1; }
expect_exit 15 "$RB" deploy slug --repo . --deployment hosted --executable "$ELSEWHERE/exe" --out "$SLUG"
expect_grep 'not empty'

printf 'shared\n' > share/data.txt
write_deployment ''
git add -A >/dev/null; git commit -qm "the deployment names no provider" >/dev/null
expect_exit 15 "$RB" deploy slug --repo . --deployment hosted --executable "$ELSEWHERE/exe" --out "$ELSEWHERE/none"
expect_grep 'names no provider'
[ ! -e "$ELSEWHERE/none" ] || { echo "    a deployment with no provider produced a slug"; exit 1; }
exit 0

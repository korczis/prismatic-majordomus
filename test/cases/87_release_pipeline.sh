# The release workflow is an adapter over .ai/repo/ci/release.yaml and the generated build
# matrix. It may not carry a platform, a runner, an artifact name or a URL of its own, and
# only the job that publishes may write. This case holds it to all of that without running
# a release.
. "$ROOT/test/lib.sh"
MODEL="$ROOT/.ai/repo/ci/release.yaml"
WF="$ROOT/.github/workflows/release.yml"
MJB="$(rust_bin)" || rust_bin_exit $?
expect_file "$MODEL"
expect_file "$WF"

# --- the trigger the model declares is the trigger the workflow has ---------------------
tags="$(sed -n 's/^  tags: *//p' "$MODEL" | tr -d "'\"" | head -n 1)"
[ -n "$tags" ] || { echo "    the release model declares no tag pattern"; exit 1; }
grep -qF -- "- '$tags'" "$WF" \
  || { echo "    the workflow does not trigger on the pattern the model declares ($tags)"; exit 1; }

# --- the matrix comes from the file the model names, and from nothing else --------------
source_file="$(sed -n 's/^  source: *//p' "$MODEL" | head -n 1)"
expect_file "$ROOT/$source_file"
grep -q "$source_file" "$WF" \
  || { echo "    the workflow does not read $source_file"; exit 1; }

# --- no platform is written in the workflow ----------------------------------------------
# every Rust target triple the model declares, and every runner it names, must be absent:
# the workflow reads them from the matrix or it is carrying a copy
"$MJB" distribution --repo "$ROOT" targets 2>/dev/null | awk 'NR>1{print $2}' | while read -r triple; do
  grep -qF -- "$triple" "$WF" && { echo "    the workflow names the target $triple"; exit 1; }
  :
done
runners="$(sed -n 's/.*"runner": "\([^"]*\)".*/\1/p' "$ROOT/$source_file" | LC_ALL=C sort -u)"
for runner in $runners; do
  case "$runner" in
    ubuntu-24.04) continue ;;   # the coordination jobs run there too; it is not a target's runner
  esac
  grep -q "runs-on: $runner" "$WF" && { echo "    the workflow pins $runner instead of reading it from the matrix"; exit 1; }
done
grep -q 'runs-on: ${{ matrix.runner }}' "$WF" \
  || { echo "    no job takes its runner from the matrix"; exit 1; }

# --- no artifact name and no installation URL is written in the workflow ------------------
first="$("$MJB" distribution --repo "$ROOT" targets 2>/dev/null | awk 'NR>1 && $3=="supported"{print $1; exit}')"
name="$("$MJB" distribution --repo "$ROOT" artifact --target "$first" --tag v9.9.9 --format text 2>/dev/null | sed -n 1p)"
stem="${name%-v9.9.9-*}"
grep -qE "$stem-v[0-9]" "$WF" && { echo "    the workflow composes an artifact name"; exit 1; }
installer="$("$MJB" distribution --repo "$ROOT" show --format json 2>/dev/null \
             | sed -n 's/.*"installer_url": "\([^"]*\)".*/\1/p' | head -n 1)"
grep -qF "$installer" "$WF" && { echo "    the workflow writes the installer URL instead of reading it"; exit 1; }

# --- every phase the model declares is a job of the workflow -------------------------------
awk '/^  - id: /{id=$3} /^    job: /{print id "\t" $2}' "$MODEL" > "$T/phases"
[ -s "$T/phases" ] || { echo "    the release model declares no phases"; exit 1; }
while IFS="$(printf '\t')" read -r phase job; do
  grep -q "^  $job:" "$WF" \
    || { echo "    the phase '$phase' names the job '$job' and the workflow has none"; exit 1; }
done < "$T/phases"
for job in plan build publish smoke; do
  grep -q "^  $job:" "$WF" || { echo "    the workflow has no $job job"; exit 1; }
done

# --- least privilege: only the publication job may write -------------------------------------
grep -q '^permissions:' "$WF" || { echo "    the workflow declares no default permissions"; exit 1; }
grep -A1 '^permissions:$' "$WF" | grep -q 'contents: read' \
  || { echo "    the workflow's default permission is not read-only"; exit 1; }
writes="$(grep -c 'contents: write' "$WF")"
[ "$writes" = 1 ] || { echo "    $writes jobs ask for write permission; exactly one may"; exit 1; }
# and it is the publication job: the write appears after `publish:` and before `smoke:`
awk '/^  publish:/{p=1} /^  smoke:/{p=0} p && /contents: write/{found=1} END{exit !found}' "$WF" \
  || { echo "    the job with write permission is not the publication job"; exit 1; }

# --- a release is all of its supported targets or it is not a release -------------------------
grep -q 'require_every_supported_target: true' "$MODEL" \
  || { echo "    the model no longer requires every supported target"; exit 1; }
awk '/^  build:/{p=1} /^  publish:/{p=0} p && /fail-fast: true/{found=1} END{exit !found}' "$WF" \
  || { echo "    the build matrix does not stop at the first failure"; exit 1; }
# the recorder is what enforces it: a record missing a supported target is refused
mkdir -p "$T/empty-dist"
expect_exit 10 env MAJORDOMUS_DIST_BIN="$MJB" "$ROOT/scripts/release-record" \
  --tag v9.9.9 --dir "$T/empty-dist" --commit "$(printf '0%.0s' $(seq 40))"

# --- the version gate refuses a tag that disagrees with the tree -------------------------------
expect_exit 10 "$ROOT/scripts/release-version" --check --tag v99.99.99
expect_exit 0 "$ROOT/scripts/release-version" --check --tag "v$("$ROOT/scripts/release-version")"

# --- publication regenerates every projection of the record, not only its own metadata ---------
# A release moves docs/INSTALL.md, which is a canonical input of the site's derived data. A
# publication that commits the guide without regenerating what is derived from it leaves the
# Pages build refusing the tree, and the metadata the installer reads is never deployed — the
# release exists on GitHub and cannot be installed. The derivation graph is what prevents it.
awk '/^  publish:/{p=1} /^  smoke:/{p=0} p && /scripts\/derive/{found=1} END{exit !found}' "$WF" \
  || { echo "    the publication job does not run scripts/derive; it would commit a guide the site's data no longer matches"; exit 1; }

# --- the publication phase is the only one that runs gh ------------------------------------------
awk '/^  plan:/{p=1} /^  publish:/{p=0} p && /gh release/{found=1} END{exit found}' "$WF" \
  || { echo "    a job before publication calls gh release"; exit 1; }
exit 0

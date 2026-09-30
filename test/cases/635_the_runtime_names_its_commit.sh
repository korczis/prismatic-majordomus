# majordomus-covers: none
# The runtime names the commit it was built from.
#
# The site already proves which revision it serves: /build.json names the commit and
# `scripts/pages verify --commit` waits for it. The process did not. `GET /api/v1/live` and
# `GET /api/v1/ready` answered a version, and a version is what a build calls itself — every
# revision between two bumps shares it — so a deployed runtime could not be told apart from
# the one before it. The executable had the commit compiled in all along (`crate::COMMIT`,
# which `distribution build` reports); nothing the health routes served said so, and the
# image build never handed it in, so a container would have said `unknown` even if asked.
#
# Asserted here:
#   0. the build named its commit: when the executable was built in a checkout of this
#      repository — by this suite (MAJORDOMUS_BIN unset) or by CI, whose MAJORDOMUS_BIN is
#      built from the checkout it tests — `distribution build` names a full commit of this
#      repository and not `unknown`, and on CI exactly the checkout's HEAD; only an
#      executable handed in from elsewhere may say `unknown`;
#   1. live, ready and the index route answer, under `commit`, that same commit, and
#      `dirty` on all three — true or false for a build made in a checkout, which git could
#      ask, and null only for one handed in from elsewhere;
#   2. the declared output schema requires `commit`, so OpenAPI, MCP and the reference
#      derive it and an unknown commit is the word `unknown`, never an absent field;
#   3. the image projection hands the commit to the builder as a build argument, declared
#      before the build that reads it.
#
# How the build decides the commit and the flag — a declared commit without a declared flag
# is unknown dirtiness, `1`/`0` are accepted, a declared `unknown` falls back to git — is a
# table, and it is unit-tested where it is defined (src/build_identity.rs, which build.rs
# compiles in through #[path]); this case measures what an executable actually built says.
#
# The executable is the suite's (rust_bin, or CI's MAJORDOMUS_BIN), built without
# MAJORDOMUS_BUILD_COMMIT: setting it here would rebuild the build directory every other
# Rust case shares. Locally the expectation is a commit of this repository rather than
# this checkout's HEAD, because build.rs reruns on the crate's sources and not on HEAD, so a
# commit that touched no crate source leaves the earlier one compiled in. CI builds fresh
# (its cache keeps no workspace crate), so there the commit is the checkout's HEAD.
. "$ROOT/test/lib.sh"
command -v curl >/dev/null 2>&1 || skip "no curl"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj635.XXXXXX")"
SRV=""; trap 'rm -rf "$S"; [ -n "$SRV" ] && kill "$SRV" 2>/dev/null' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm layer >/dev/null

# What the executable says it was built from, asked without a server.
rc=0; "$RB" distribution build --format json > "$S/build.json" 2> "$S/build.err" || rc=$?
[ "$rc" = 0 ] || { echo "    distribution build --format json exited $rc:"; cat "$S/build.err"; exit 1; }
BUILT="$(jq -r '.commit' "$S/build.json")"

# --- 0. a build made in a checkout names its commit
# built_here: the executable was built from a checkout of this repository, where build.rs
# could ask git — so `unknown` there is a build that failed to record what it knew
built_here=0
if [ -z "${MAJORDOMUS_BIN:-}" ] || [ "${GITHUB_ACTIONS:-}" = true ]; then built_here=1; fi
case "$BUILT" in
  unknown)
    [ "$built_here" = 0 ] \
      || { echo "    the executable was built in this checkout and names no commit (unknown)"
           exit 1; }
    ;;
  *)
    printf '%s' "$BUILT" | grep -Eq '^[0-9a-f]{40}$' \
      || { echo "    the executable names '$BUILT', neither a full commit nor unknown"; exit 1; }
    git -C "$ROOT" --no-pager cat-file -e "$BUILT^{commit}" 2>/dev/null \
      || { echo "    the executable names $BUILT, which is no commit of this repository"
           exit 1; }
    ;;
esac
if [ "${GITHUB_ACTIONS:-}" = true ]; then
  HEAD_SHA="$(git -C "$ROOT" --no-pager rev-parse HEAD)"
  [ "$BUILT" = "$HEAD_SHA" ] \
    || { echo "    CI built the executable from $HEAD_SHA and it names $BUILT"; exit 1; }
fi
echo "    the executable names commit $(printf '%s' "$BUILT" | cut -c1-12)" \
  "(built in a checkout: $built_here)"

serve_up "$S/out.txt" "$S/err.txt" || exit 1

# --- 1. every answer that names the build names the commit
for route in /api/v1/live /api/v1/ready /; do
  # --max-time, because a request without a bound is a case that hangs instead of failing:
  # the rule is project.liveness, and scripts/liveness-check refuses an unbounded one
  curl -fsS --max-time 30 -H 'Accept: application/json' "$U$route" > "$S/answer.json" \
    || { echo "    GET $route did not answer"; exit 1; }
  jq -e 'has("commit")' "$S/answer.json" >/dev/null \
    || { echo "    GET $route carries no commit:"; cat "$S/answer.json"; exit 1; }
  jq -e 'has("dirty") and (.dirty == null or .dirty == true or .dirty == false)' \
    "$S/answer.json" >/dev/null \
    || { echo "    GET $route does not say whether the build was dirty (null when unknown):"
         cat "$S/answer.json"; exit 1; }
  # built in a checkout without a declared commit, build.rs asked git: it knew
  [ "$built_here" = 0 ] \
    || jq -e '.dirty == true or .dirty == false' "$S/answer.json" >/dev/null \
    || { echo "    GET $route: built in a checkout, the build did not know if it was dirty:"
         cat "$S/answer.json"; exit 1; }
  served="$(jq -r '.commit' "$S/answer.json")"
  [ "$served" = "$BUILT" ] \
    || { echo "    GET $route names commit '$served'; the executable was built from $BUILT"
         exit 1; }
done
echo "    live, ready and / name commit $(printf '%s' "$BUILT" | cut -c1-12) in full, with dirty"

# --- 2. the declared schema requires it, so every projection derives it
for f in live ready; do
  rc=0
  "$RB" capabilities schema "health.$f" --side output > "$S/$f.schema.json" 2> "$S/schema.err" \
    || rc=$?
  [ "$rc" = 0 ] \
    || { echo "    capabilities schema health.$f --side output exited $rc:"; cat "$S/schema.err"
         exit 1; }
  jq -e '.required | index("commit")' "$S/$f.schema.json" >/dev/null \
    || { echo "    health.$f's declared output does not require commit"; exit 1; }
  jq -e '.properties.dirty' "$S/$f.schema.json" >/dev/null \
    || { echo "    health.$f's declared output does not declare dirty"; exit 1; }
done
echo "    health.live and health.ready declare commit as a required output field"

# --- 3. the image build is handed the commit
grep -q '^ARG MAJORDOMUS_BUILD_COMMIT=unknown$' "$ROOT/deploy/Dockerfile" \
  || { echo "    deploy/Dockerfile does not pass MAJORDOMUS_BUILD_COMMIT to the builder"; exit 1; }
awk '/^ARG MAJORDOMUS_BUILD_COMMIT/{a=NR} /^RUN cargo build/{r=NR} END{exit !(a && r && a < r)}' \
  "$ROOT/deploy/Dockerfile" \
  || { echo "    the build argument is declared after the build that should read it"; exit 1; }
echo "    the image projection passes MAJORDOMUS_BUILD_COMMIT into the build"


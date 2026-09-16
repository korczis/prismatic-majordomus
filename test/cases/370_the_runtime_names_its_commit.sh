# majordomus-covers: none
# The runtime names the commit it was built from.
#
# The site already proves which revision it serves: /build.json names the commit and
# `scripts/pages verify --commit` waits for it. The process did not. `GET /api/v1/live` and
# `GET /api/v1/ready` answered a version, and a version is what a build calls itself — every
# revision between two bumps shares it — so a deployed runtime could not be told apart from
# the one before it. The executable had the commit compiled in all along (`crate::COMMIT`);
# nothing it served said so, and the image build never handed it in, so a container would
# have said `unknown` even if asked.
#
# Asserted here:
#   1. an executable built with MAJORDOMUS_BUILD_COMMIT set answers that commit — this
#      checkout's HEAD — on live, on ready and on the index route, in full, under `commit`;
#      and `dirty` is present on all three, `null` when the build was not told;
#   2. the declared output schema requires `commit`, so OpenAPI, MCP and the reference
#      derive it and an unknown commit is the word `unknown`, never an absent field;
#   3. the image projection hands the commit to the builder as a build argument;
#   4. the mutation: an expected commit that is not the one served is detected as a
#      mismatch, not accepted. There is no runtime analogue of `scripts/pages verify` yet;
#      the comparison below is the one such a verifier must make, and the verifier itself
#      is follow-up work.
. "$ROOT/test/lib.sh"
command -v curl >/dev/null 2>&1 || { echo "    skip: no curl"; exit 0; }
command -v jq >/dev/null 2>&1 || { echo "    skip: no jq"; exit 0; }
HEAD_SHA="$(git -C "$ROOT" rev-parse HEAD)"

# An executable built by the suite from this checkout names HEAD through git; one built here
# is told it, the way a release pipeline and the image build tell it. A CI job hands the
# suite its own build (MAJORDOMUS_BIN) of the same checkout, so the expectation is the same.
if [ -z "${MAJORDOMUS_BIN:-}" ]; then
  MAJORDOMUS_BUILD_COMMIT="$HEAD_SHA"; export MAJORDOMUS_BUILD_COMMIT
  unset MAJORDOMUS_BUILD_DIRTY
fi
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj370.XXXXXX")"
SRV=""; trap 'rm -rf "$S"; [ -n "$SRV" ] && kill "$SRV" 2>/dev/null' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm layer >/dev/null

"$RB" serve --repo "$PWD" --port 0 --idle 60 </dev/null > "$S/out.txt" 2> "$S/err.txt" & SRV=$!
i=0
until grep -q 'listening on http://' "$S/err.txt" 2>/dev/null; do
  i=$((i+1)); [ "$i" -lt 300 ] || { echo "    the server never listened"; cat "$S/err.txt"; exit 1; }
  kill -0 "$SRV" 2>/dev/null || { echo "    the server exited before listening"; cat "$S/err.txt"; exit 1; }
  sleep 0.1
done
U="$(sed -n 's#.*listening on \(http://127\.0\.0\.1:[0-9]*\).*#\1#p' "$S/err.txt" | head -n 1)"
[ -n "$U" ] || { echo "    no URL on the listening line"; cat "$S/err.txt"; exit 1; }

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
  served="$(jq -r '.commit' "$S/answer.json")"
  [ "$served" = "$HEAD_SHA" ] \
    || { echo "    GET $route names commit '$served'; the executable was built from $HEAD_SHA"
         exit 1; }
  if [ -n "${MAJORDOMUS_BUILD_COMMIT:-}" ]; then
    # a commit handed in says nothing about the tree it came from
    jq -e '.dirty == null' "$S/answer.json" >/dev/null \
      || { echo "    GET $route claims a dirty flag the build was never told:"
           cat "$S/answer.json"; exit 1; }
  fi
  cp "$S/answer.json" "$S/answer$(printf '%s' "$route" | tr '/' '_').json"
done
echo "    live, ready and / name commit ${HEAD_SHA:0:12} in full, with dirty present"

# --- 2. the declared schema requires it, so every projection derives it
for f in live ready; do
  "$RB" capabilities schema "health.$f" --side output > "$S/$f.schema.json" 2> "$S/schema.err" \
    || { echo "    capabilities schema health.$f --side output failed:"; cat "$S/schema.err"; exit 1; }
  jq -e '.required | index("commit")' "$S/$f.schema.json" >/dev/null \
    || { echo "    health.$f's declared output does not require commit"; exit 1; }
  jq -e '.properties.dirty' "$S/$f.schema.json" >/dev/null \
    || { echo "    health.$f's declared output does not declare dirty"; exit 1; }
done
grep -q '"commit"' "$ROOT/docs/generated/registry.json" \
  || { echo "    the generated registry does not carry the commit field"; exit 1; }
echo "    health.live and health.ready declare commit as a required output field"

# --- 3. the image build is handed the commit
grep -q '^ARG MAJORDOMUS_BUILD_COMMIT=unknown$' "$ROOT/deploy/Dockerfile" \
  || { echo "    deploy/Dockerfile does not pass MAJORDOMUS_BUILD_COMMIT to the builder"; exit 1; }
awk '/^ARG MAJORDOMUS_BUILD_COMMIT/{a=NR} /^RUN cargo build/{r=NR} END{exit !(a && r && a < r)}' \
  "$ROOT/deploy/Dockerfile" \
  || { echo "    the build argument is declared after the build that should read it"; exit 1; }
echo "    the image projection passes MAJORDOMUS_BUILD_COMMIT into the build"

# --- 4. the mutation: a runtime serving another commit is detected
# The comparison a deployment verifier makes: the commit it expects against the commit the
# runtime names. A deliberately wrong expectation must be refused.
serves_commit() {   # answer-file expected-sha
  [ "$(jq -r '.commit' "$1")" = "$2" ]
}
wrong="$(printf '%s' "$HEAD_SHA" | tr '0-9a-f' '1-9a-f0')"
[ "$wrong" != "$HEAD_SHA" ] || { echo "    the mutation did not change the commit"; exit 1; }
serves_commit "$S/answer_api_v1_live.json" "$HEAD_SHA" \
  || { echo "    the verifier refused the commit that is served"; exit 1; }
if serves_commit "$S/answer_api_v1_live.json" "$wrong"; then
  echo "    a runtime expected at ${wrong:0:12} was accepted while serving ${HEAD_SHA:0:12}"
  exit 1
fi
if serves_commit "$S/answer_api_v1_live.json" unknown; then
  echo "    a runtime that names its commit was accepted as unknown"; exit 1
fi
echo "    an expected commit that is not the served one is detected as a mismatch"

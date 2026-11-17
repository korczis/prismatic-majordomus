# majordomus-covers: none
# The Evidence screen's claim inspector (I2477), through a real server over a fixture claims
# matrix of two claims one test settles:
#
#   a claim         ?claim= opens it with what it says, where, what settles it, its last run
#                   (none yet) and the claim the same test also proves, a link to its own
#   the way in      the Evidence screen's claim rows lead to the inspector
#   unknown or bad  a claim the matrix does not declare is said as the refusal it is; a value
#                   that is no id is never asked
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# the fixture's server listens on loopback whatever the machine's own default host is
unset MAJORDOMUS_HTTP_HOST

"$MJ" init >/dev/null
mkdir -p docs lib test/cases
cat > docs/CLAIMS.yaml <<'YAML'
version: 1
claims:
  - id: alpha-holds
    claim: Alpha holds
    source: docs/ALPHA.md
    implementation: lib/alpha.sh
    test: test/cases/901_alpha.sh
    status: guaranteed
  - id: alpha-also
    claim: Alpha also says why
    source: docs/ALPHA.md
    implementation: lib/alpha.sh
    test: test/cases/901_alpha.sh
    status: guaranteed
YAML
printf '# Alpha\n' > docs/ALPHA.md
printf '#!/usr/bin/env bash\n# alpha\n' > lib/alpha.sh
printf '. "$ROOT/test/lib.sh"\ntrue\n' > test/cases/901_alpha.sh
git add -A >/dev/null && git commit -qm "a claims matrix of two claims one test settles"

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
get() { curl -s --max-time 60 "$U$1"; }

# ---------------------------------------------------------------- a claim
page="$(get '/ui/evidence?claim=alpha-holds')"
printf '%s' "$page" | grep -q 'id="claim"' \
  || { echo "    ?claim=alpha-holds opened no inspector:"; printf '%s\n' "$page" | grep -o 'mj-ui-empty">[^<]*' | sed 's/^/    | /'; echo; exit 1; }
for want in 'Alpha holds' 'test/cases/901_alpha.sh' 'docs/ALPHA.md' 'href="/ui/evidence?claim=alpha-also#claim"'; do
  printf '%s' "$page" | grep -qF "$want" || { echo "    the inspector has no $want"; exit 1; }
done

# ---------------------------------------------------------------- the way in
get '/ui/evidence' | grep -qF 'href="/ui/evidence?claim=alpha-holds#claim"' \
  || { echo "    the Evidence screen's rows do not lead to the inspector"; exit 1; }

# ---------------------------------------------------------------- unknown or bad
page="$(get '/ui/evidence?claim=no-such-claim')"
printf '%s' "$page" | grep -q 'id="claim"' && { echo "    an undeclared claim opened an inspector"; exit 1; }
printf '%s' "$page" | grep -q 'no-such-claim' \
  || { echo "    an undeclared claim is not said as its refusal"; exit 1; }
page="$(get '/ui/evidence?claim=alpha%22%20onclick%3D%22x')"
printf '%s' "$page" | grep -q 'onclick' && { echo "    the page echoes a handler a link carried"; exit 1; }

echo "    a claim opens with what settles it, and leads to the claim its test also proves"

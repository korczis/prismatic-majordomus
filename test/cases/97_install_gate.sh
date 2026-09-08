# The gate that measures the published install command, measured itself. It is the only
# check in this repository whose subject is outside the repository, so what a case can hold
# it to is: that it states no address of its own, that it goes green against an origin that
# serves a complete release, and that against an origin that serves none it fails with the
# sentence a person needs — not a stack trace, and not silence.
#
# Nothing here reaches the internet: the fixture release and the origin are local, and the
# gate is pointed at them with --base.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/install-check"
expect_file "$GATE"
MJB="$(rust_bin)" || rust_bin_exit $?
command -v curl >/dev/null 2>&1 || { echo "    skip: no curl"; exit 0; }

# --- it states no address of its own ------------------------------------------------------
# The projection it reads holds every URL this project publishes at; a literal here would be
# a second statement of the same fact, and the two would diverge the day one of them moved.
MODEL="$ROOT/site/data/registry/distribution.json"
expect_file "$MODEL"
base="$(sed -n 's/.*"installer_url": "\(https\{0,1\}:\/\/[^/]*\).*/\1/p' "$MODEL" | head -n 1)"
[ -n "$base" ] || { echo "    the distribution projection names no installer origin"; exit 1; }
grep -q "$base" "$GATE" && { echo "    the gate writes the origin $base instead of reading it"; exit 1; }
expect_no_grep 'https://[a-z0-9.-]*github' "$GATE"

# --- it is a script this machine can run --------------------------------------------------
bash -n "$GATE" || { echo "    the gate does not parse"; exit 1; }
if command -v shellcheck >/dev/null 2>&1; then
  shellcheck -S warning "$GATE" || { echo "    shellcheck refuses the gate"; exit 1; }
fi

# --- an origin that serves a complete release: the gate is green ---------------------------
FIX="$T/fixture"; mkdir -p "$FIX/releases"
start_http "$FIX" || { echo "    skip: no python3 or node to serve a fixture release"; exit 0; }
trap 'stop_http' EXIT INT TERM HUP

VERSION="$("$ROOT/scripts/release-version")"
MAJORDOMUS_DIST_BIN="$MJB" "$ROOT/scripts/release-fixture" --out "$FIX" --base-url "$HTTP_BASE" > "$T/fixture.log" 2>&1 \
  || { rc=$?; echo "    the fixture release could not be built (exit $rc)"; tail -20 "$T/fixture.log" | sed 's/^/      /'; exit 1; }
expect_file "$FIX/releases/latest.json"

out="$(env HOME="$T/home-green" "$GATE" --base "$HTTP_BASE" 2>&1)" || {
  echo "    the gate failed against an origin that serves a complete release"
  printf '%s\n' "$out" | sed 's/^/      /'
  exit 1
}
printf '%s\n' "$out" | grep -q "the metadata names v$VERSION" || { echo "    the gate did not resolve the fixture release"; exit 1; }
printf '%s\n' "$out" | grep -q "the installed tool reports $VERSION" || { echo "    the gate did not check the installed version"; exit 1; }
printf '%s\n' "$out" | grep -q "MCP launcher runs with no toolchain" || { echo "    the gate did not exercise the MCP launcher"; exit 1; }
printf '%s\n' "$out" | grep -q "initialises a repository" || { echo "    the gate did not initialise a repository"; exit 1; }

# --- and it wrote nothing outside its own temporary tree -----------------------------------
# The gate was handed a home directory and installed into neither it nor the caller's: it
# makes one of its own under TMPDIR and removes it. A gate that installs into the machine it
# runs on would pass on a runner and be unusable anywhere else.
[ ! -e "$T/home-green" ] || { echo "    the gate installed into the home it was given"; exit 1; }

# --- an origin that serves no release: the gate fails, and says what a reader would see -----
mv "$FIX/releases/latest.json" "$FIX/releases/latest.json.away"
out="$(env HOME="$T/home-red" "$GATE" --base "$HTTP_BASE" 2>&1)" && {
  echo "    the gate passed against an origin that publishes no release"
  exit 1
}
printf '%s\n' "$out" | grep -q "the metadata the installer resolves is not served" \
  || { echo "    the gate did not name the metadata as the missing thing: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "README" \
  || { echo "    the gate did not say what a reader of the README gets"; exit 1; }
mv "$FIX/releases/latest.json.away" "$FIX/releases/latest.json"

# --- an origin that serves metadata for an artifact it does not have ------------------------
# The other half of the same failure: the pointer exists, the download behind it does not.
rm -f "$FIX"/majordomus-*.tar.gz
out="$(env HOME="$T/home-gone" "$GATE" --base "$HTTP_BASE" 2>&1)" && {
  echo "    the gate passed against an origin whose artifacts are missing"
  exit 1
}
printf '%s\n' "$out" | grep -q "the advertised command failed on this machine" \
  || { echo "    the gate did not report the advertised command failing: $out"; exit 1; }

# --- the CI model declares it, and the workflow runs it --------------------------------------
MODEL_CI="$ROOT/.ai/repo/ci/gates.yaml"
grep -q '^  - id: installer-live$' "$MODEL_CI" || { echo "    the CI model declares no installer-live gate"; exit 1; }
job="$(awk '/^  - id: installer-live$/{f=1;next} f&&/^    job: /{print $2;exit}' "$MODEL_CI")"
[ -n "$job" ] || { echo "    the installer-live gate names no job"; exit 1; }
W="$ROOT/.github/workflows/validate.yml"
grep -q "^  $job:" "$W" || { echo "    validate.yml has no $job job for the installer-live gate"; exit 1; }
grep -q "needs.plan.outputs.installer_live" "$W" || { echo "    the $job job is not gated on the plan's decision"; exit 1; }
grep -q "scripts/ci/install-check" "$W" || { echo "    the $job job does not run the gate"; exit 1; }
# no class may select it: it measures a deployment, and a tree cannot change the answer
awk '/^classes:/{c=1} c' "$MODEL_CI" | grep -q 'installer-live' \
  && { echo "    a path class selects installer-live; no change to a tree can affect it"; exit 1; }

echo "    the published install command has a gate, and the gate tells the truth in both directions"

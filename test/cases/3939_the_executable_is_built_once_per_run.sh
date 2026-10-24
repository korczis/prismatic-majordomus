# majordomus-covers: none
# The executable and the crate's rustdoc surface are built once per run (I3939), in the `build`
# and `rustdoc-surface` jobs right after the plan, and the jobs that drive them download them
# through .github/actions/majordomus-bin. Before it, eight jobs built the same debug executable
# (about 4.5 minutes each) and seven the same surface (about 2.3). A producer that is skipped
# while a consumer runs would not be a slow job but a missing file, and one that runs for a plan
# that selects no consumer would be a build nobody reads, so this case holds the wiring:
#
#   one build     outside the macos job (another target), the crate's `cargo build ...
#                 --manifest-path apps/majordomus-cli/Cargo.toml` appears in the build job and
#                 nowhere else of validate.yml; the composite action carries the one fallback
#   covered       for each producer, every job that waits for it (directly or through a job it
#                 needs) and every job that uses the action for what it produces runs only when
#                 the producer's `if:` does: each plan output in the consumer's `if:` is in the
#                 producer's; and the producer's `if:` is those outputs alone, never always(),
#                 so that a plan that selects no consumer skips both
#   one artifact  `majordomus-exe` is uploaded once, by the build job, with the provenance
#                 fields scripts/rust-check --artifact writes, and is downloaded in exactly
#                 one place, the action, which compares the provenance with its checkout and
#                 builds when it cannot trust it
#   the verdict   the ci job needs both producers
#
# Mutations: a consumer whose condition the producer does not cover, an `always()` in a
# producer, and a second build step in a consumer each turn the case red.
. "$ROOT/test/lib.sh"
W="$ROOT/.github/workflows/validate.yml"
A="$ROOT/.github/actions/majordomus-bin/action.yml"
[ -f "$W" ] || { echo "    .github/workflows/validate.yml is missing"; exit 1; }
[ -f "$A" ] || { echo "    .github/actions/majordomus-bin/action.yml is missing"; exit 1; }

job_of() { # <workflow> <job> -> the job's lines
  awk -v j="  $2:" '$0 == j {f=1; next} f && /^  [a-z][a-z0-9-]*:$/ {exit} f' "$1"
}
jobs_of() { awk '/^jobs:/ {f=1; next} f && /^  [a-z][a-z0-9-]*:$/ {gsub(/[ :]/, ""); print}' "$1"; }
outputs_of() { # <workflow> <job> -> the plan outputs the job's own if: reads, one per line
  job_of "$1" "$2" | sed -n 's/^    if: //p' | head -n 1 | grep -oE 'needs\.plan\.outputs\.[a-z_]+' | LC_ALL=C sort -u || true
}
needs_of() { # <workflow> <job> -> the jobs it names in needs, one per line
  job_of "$1" "$2" | sed -n 's/^    needs: //p' | head -n 1 | tr -d '[] ' | tr ',' '\n' | grep -v '^$' || true
}
waits_for() { # <workflow> <job> <ancestor>: whether <ancestor> is a direct or indirect need of <job>
  local n
  for n in $(needs_of "$1" "$2"); do
    [ "$n" = "$3" ] && return 0
    [ "$n" = plan ] && continue
    waits_for "$1" "$n" "$3" && return 0
  done
  return 1
}
uses_action_for() { # <workflow> <job> <input>: whether the job passes <input>: "true" to the action
  job_of "$1" "$2" | awk -v want="$3" '
    /uses: \.\/\.github\/actions\/majordomus-bin/ {f=1; next}
    f && /^      - / {f=0}
    f && $1 == want ":" && $2 == "\"true\"" {found=1}
    END {exit !found}'
}

judge() { # <workflow> -> findings, one per line; nothing when the wiring holds
  local wf="$1" j p o pout in uses direct
  for p in build rustdoc-surface; do
    job_of "$wf" "$p" | grep -q . || { echo "no job $p"; continue; }
    pout="$(outputs_of "$wf" "$p")"
    [ -n "$pout" ] || echo "job $p has no plan output in its if:"
    job_of "$wf" "$p" | sed -n 's/^    if: //p' | head -n 1 | grep -q 'always()' \
      && echo "job $p runs on always(): a plan that selects none of its consumers would build it"
    printf '%s\n' "$(needs_of "$wf" "$p")" | grep -qx plan || echo "job $p does not wait for the plan"
    needs_of "$wf" ci | grep -qx "$p" || echo "the ci job does not need $p"
    in=exe; [ "$p" = rustdoc-surface ] && in=rustdoc
    for j in $(jobs_of "$wf"); do
      [ "$j" = "$p" ] && continue
      uses=0; uses_action_for "$wf" "$j" "$in" && uses=1
      direct=0; needs_of "$wf" "$j" | grep -qx "$p" && direct=1
      # a wait that downloads nothing is a wait for nothing (the ci job waits for the verdict)
      [ "$direct" = 1 ] && [ "$uses" = 0 ] && [ "$j" != ci ] && echo "job $j waits for $p and never downloads from it"
      [ "$uses" = 1 ] || continue
      # a download of an artifact nothing orders before the job is a race the fallback hides
      waits_for "$wf" "$j" "$p" || echo "job $j downloads from $p and nothing orders it after $p"
      [ -n "$(outputs_of "$wf" "$j")" ] || echo "job $j uses $p and has no plan output in its if: the case cannot cover it"
      for o in $(outputs_of "$wf" "$j"); do
        printf '%s\n' "$pout" | grep -qx "$o" \
          || echo "job $j runs on ${o#needs.plan.outputs.} but $p, which it uses, does not: the artifact would be missing"
      done
    done
  done
  # one build of the crate outside macos, in the producer
  awk '
    /^  [a-z][a-z0-9-]*:$/ { job = $0; gsub(/[ :]/, "", job) }
    /^ *#/ { next }
    /cargo build[^#]*apps\/majordomus-cli\/Cargo\.toml/ { print job }' "$wf" | LC_ALL=C sort | uniq -c \
    | while read -r n job; do
        case "$job" in
          build) [ "$n" = 1 ] || echo "the build job builds the crate $n times" ;;
          macos) ;;
          *) echo "job $job builds the crate itself; it downloads the build job's executable" ;;
        esac
      done
  [ "$(job_of "$wf" build | grep -cE '^\s+run: .*cargo build[^#]*apps/majordomus-cli/Cargo\.toml')" = 1 ] \
    || echo "the build job does not build the crate exactly once"
  # one artifact
  [ "$(grep -c 'name: majordomus-exe$' "$wf")" = 1 ] || echo "majordomus-exe is named other than once in the workflow"
  job_of "$wf" build | grep -q 'name: majordomus-exe$' || echo "the build job does not upload majordomus-exe"
  job_of "$wf" build | grep -q 'retention-days: 1$' || echo "the executable artifact is kept longer than the run needs"
  job_of "$wf" build | grep -q 'if-no-files-found: error' || echo "the executable artifact may upload nothing"
  job_of "$wf" rustdoc-surface | grep -q 'name: rustdoc-surface$' || echo "the rustdoc-surface job does not upload rustdoc-surface"
  for k in schema name profile commit target rustc cargo_lock_sha256 built_at; do
    job_of "$wf" build | grep -q "$k" || echo "the provenance lacks the field $k of scripts/rust-check --artifact"
  done
}

out="$(judge "$W")"
[ -z "$out" ] || { printf '    %s\n' "the executable is not built once per run:"; printf '      %s\n' "$out"; exit 1; }

# the consumers the case is about are the ones it found: a reader that finds none proves nothing
n=0
for j in $(jobs_of "$W"); do
  if waits_for "$W" "$j" build || uses_action_for "$W" "$j" exe; then n=$((n + 1)); fi
done
[ "$n" -ge 6 ] || { echo "    only $n job(s) use the build job; the reader has lost the consumers"; exit 1; }
for j in suite-shard ui-shard ui-checks; do
  uses_action_for "$W" "$j" rustdoc || { echo "    $j does not download the rustdoc surface"; exit 1; }
done

# the action: the download, the provenance against HEAD, the export, the fallback
grep -q 'name: majordomus-exe$' "$A" || { echo "    the action does not download majordomus-exe"; exit 1; }
grep -q 'name: rustdoc-surface$' "$A" || { echo "    the action does not download rustdoc-surface"; exit 1; }
[ "$(grep -c 'uses: actions/download-artifact@v4' "$A")" = 2 ] || { echo "    the action downloads other than the two artifacts"; exit 1; }
[ "$(grep -c 'continue-on-error: true' "$A")" = 2 ] || { echo "    a download of the action does not fall back on failure"; exit 1; }
grep -q 'git rev-parse HEAD' "$A" && grep -q "\.commit" "$A" && grep -q "\.built_from" "$A" \
  || { echo "    the action does not compare the provenance with HEAD"; exit 1; }
grep -q 'chmod +x' "$A" || { echo "    the action does not restore the executable's mode"; exit 1; }
grep -q 'echo "MAJORDOMUS_BIN=' "$A" || { echo "    the action does not export MAJORDOMUS_BIN"; exit 1; }
[ "$(grep -cE '^\s+RUSTFLAGS=.* cargo build .*apps/majordomus-cli/Cargo\.toml' "$A")" = 1 ] \
  || { echo "    the action does not carry exactly one cargo build fallback"; exit 1; }
grep -q 'uses: ./.github/actions/rustdoc' "$A" || { echo "    the action's rustdoc fallback is not the rustdoc action"; exit 1; }
echo "    ok: one build of the crate outside macos, every consumer covered by its producer's if:, one artifact downloaded through the action"

# --- mutations: the reader refuses what it is meant to refuse
mutant() { # <name> <awk program over the workflow> <expected finding (ERE)>
  awk "$2" "$W" > "$T/$1.yml"
  cmp -s "$W" "$T/$1.yml" && { echo "    mutation $1 changed nothing"; exit 1; }
  out="$(judge "$T/$1.yml")"
  printf '%s\n' "$out" | grep -qE -- "$3" || { echo "    mutation $1 was not refused as /$3/; findings:"; printf '      %s\n' "$out"; exit 1; }
}
mutant uncovered '/^  build:$/ {f=1} f && /^    if: / { sub(/ \|\| needs\.plan\.outputs\.mesh_lab == .true./, ""); f=0 } 1' \
  'job mesh-lab runs on mesh_lab but build'
mutant always '/^  rustdoc-surface:$/ {f=1} f && /^    if: / { sub(/^    if: /, "    if: always() \\&\\& "); f=0 } 1' \
  'rustdoc-surface runs on always'
mutant second-build '/^  mesh-lab:$/ {f=1} f && /^    steps:$/ { print; print "      - run: RUSTFLAGS=\"\" cargo build --quiet --locked --manifest-path apps/majordomus-cli/Cargo.toml"; f=0; next } 1' \
  'job mesh-lab builds the crate itself'
mutant unneeded '{ sub(/needs: \[plan, build, rustdoc-surface, structure,/, "needs: [plan, build, structure,") } 1' \
  'the ci job does not need rustdoc-surface'
echo "    ok: an uncovered consumer, an always() producer, a second build and a verdict that skips a producer are each refused"

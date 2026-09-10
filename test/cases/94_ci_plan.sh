# majordomus-covers: none
# The CI planner, the verdict and the parallel runner: the pieces validate.yml is an adapter
# over. The planner is driven with listed paths, never with this checkout's diff, so the
# case is the same on every machine; the verdict with fixture plans and needs contexts; the
# runner with a private harness of throwaway cases.
. "$ROOT/test/lib.sh"
PLAN="$ROOT/scripts/ci-plan"; VERDICT="$ROOT/scripts/ci/verdict"; MODEL="$ROOT/.ai/repo/ci/gates.yaml"
command -v jq >/dev/null 2>&1 || { echo "    jq absent; skipping"; exit 0; }
plan() { printf '%s\n' "$@" | "$PLAN" --files - ; }
selected() { plan "$@" | jq -r '.selected | join(" ")'; }
# the same plan, asked for the gates whose runner is not available on demand (the macOS ones):
# what the nightly schedule, a dispatch and a ci:full pull request ask for
plan_od() { printf '%s\n' "$@" | "$PLAN" --files - --on-demand ; }
selected_od() { plan_od "$@" | jq -r '.selected | join(" ")'; }
# read from the model, never listed here: a gate that gains or loses `on-demand` should not
# have to be spelled in a case to be believed
ondemand="$(awk '/^  - id: /{id=$3} /^    on-demand: true$/{print id}' "$MODEL" | tr '\n' ' ')"
has() { case " $1 " in *" $2 "*) return 0 ;; *) return 1 ;; esac; }
lacks() { ! has "$1" "$2"; }

# --- the model resolves, and a model that does not is refused by name
expect_exit 0 "$PLAN" --check
# the rust class's gate list, whatever it currently holds: a literal here goes stale the
# day a gate is added to that class, and the mutation then silently does nothing
awk '/^  - id: rust$/{r=1} r && /^    gates: /{print "    gates: [rust-check, no-such-gate]"; r=0; next} {print}' "$MODEL" > broken.yaml
grep -q no-such-gate broken.yaml || { echo "    the mutation did not take"; exit 1; }
expect_exit 10 "$PLAN" --model broken.yaml --check
expect_grep 'names a gate that does not exist: no-such-gate'

# --- the usage a person guesses. `ci-plan <path> <path>` is the obvious first try and the
# one thing the parser cannot take, so the refusal names the spelling it wanted rather than
# reporting a path as an option; a misspelt option is a different mistake and still says so.
# Both exit 2.
expect_exit 2 "$PLAN" docs/DESIGN.md docs/CLI.md
expect_grep 'paths are not given as arguments'
expect_grep '--files -'
expect_no_grep 'unknown option'
expect_exit 2 "$PLAN" --fils docs/DESIGN.md
expect_grep 'unknown option --fils'

# --- the gates that always run are in every plan, and an empty change selects nothing else.
# Which gates those are comes from the model, not from a list here: a gate that gains
# `always: true` should not have to be spelled in a case to be believed.
s="$(selected)"
always="$(awk '/^  - id: /{id=$3} /^    always: true/{print id}' "$ROOT/.ai/repo/ci/gates.yaml")"
[ -n "$always" ] || { echo "    the model declares no always-gate; the plan cannot be checked against it"; exit 1; }
for g in $always; do has "$s" "$g" || { echo "    the always gate $g is missing from the empty plan: $s"; exit 1; }; done
n="$(printf '%s\n' "$always" | wc -w | tr -d ' ')"
[ "$(printf '%s\n' "$s" | wc -w | tr -d ' ')" = "$n" ] || { echo "    an empty change selected more than the $n always gate(s): $s"; exit 1; }

# --- a Rust change selects the Rust gates and the suite, never the site
s="$(selected apps/majordomus-cli/src/lib.rs)"
for g in rust-check rust-integration rust-coverage shell-suite; do has "$s" "$g" || { echo "    a Rust change did not select $g: $s"; exit 1; }; done
for g in site-build site-probe; do lacks "$s" "$g" || { echo "    a Rust change selected $g, which reads nothing from the crate: $s"; exit 1; }; done
# the class still names the macOS gates; what changed is when they are planned, so the class
# map is asserted over the plan that asks for them
s="$(selected_od apps/majordomus-cli/src/lib.rs)"
for g in rust-bench macos; do has "$s" "$g" || { echo "    a Rust change did not select $g when the plan asked for the on-demand gates: $s"; exit 1; }; done

# --- a site change selects the site gates and the suite (two cases build the site), never
#     a Rust gate
s="$(selected site/templates/base.html site/tailwind.css)"
for g in site-build site-probe shell-suite; do has "$s" "$g" || { echo "    a site change did not select $g: $s"; exit 1; }; done
# the on-demand plan, so that "a site change selects no Rust gate" stays a statement about
# the class map rather than one about the cadence
s="$(selected_od site/templates/base.html site/tailwind.css)"
for g in rust-check rust-integration rust-coverage rust-bench macos; do lacks "$s" "$g" || { echo "    a site change selected $g: $s"; exit 1; }; done

# --- a document is an input of the site and an object of the index: the site gates and the
#     registry checks, not the crate's own gates nor coverage
s="$(selected docs/DESIGN.md)"
for g in site-build site-probe shell-suite rust-integration; do has "$s" "$g" || { echo "    a docs change did not select $g: $s"; exit 1; }; done
s="$(selected_od docs/DESIGN.md)"
for g in rust-check rust-coverage rust-bench macos; do lacks "$s" "$g" || { echo "    a docs change selected $g: $s"; exit 1; }; done

# --- the distribution is read by both implementations and by the site: every gate but the
#     always ones comes from the class, none from escalation
p="$(plan_od share/schemas/majordomus/policy/policy.v1.schema.json)"
[ "$(printf '%s' "$p" | jq -r .mode)" = affected ] || { echo "    a share change escalated instead of selecting by class"; exit 1; }
s="$(printf '%s' "$p" | jq -r '.selected | join(" ")')"
for g in rust-check rust-coverage rust-bench shell-suite site-build site-probe macos; do has "$s" "$g" || { echo "    a share change did not select $g: $s"; exit 1; }; done

# --- the pipeline itself, and a path no class knows, escalate to the full plan and say why
p="$(plan_od .github/workflows/validate.yml)"
[ "$(printf '%s' "$p" | jq -r .mode)" = full ] || { echo "    a workflow change did not escalate"; exit 1; }
printf '%s' "$p" | jq -r .reason | grep -q 'escalates: .github/workflows/validate.yml' || { echo "    the escalation does not name the path"; exit 1; }
[ "$(printf '%s' "$p" | jq -r '[.gates[] | select(.selected)] | length')" = "$(printf '%s' "$p" | jq -r '.gates | length')" ] || { echo "    the full plan left a gate out"; exit 1; }
p="$(plan some/new/thing.txt)"
[ "$(printf '%s' "$p" | jq -r .mode)" = full ] || { echo "    an unclassified path did not escalate"; exit 1; }
printf '%s' "$p" | jq -r .unclassified[0] | grep -qx 'some/new/thing.txt' || { echo "    the plan does not name the unclassified path"; exit 1; }
p="$(plan .gitignore)"
# an inert path selects the always-gates and nothing beyond them; how many that is comes
# from the model, for the same reason as above
[ "$(printf '%s' "$p" | jq -r .mode)" = affected ] && [ "$(printf '%s' "$p" | jq -r '.selected | length')" = "$n" ] || { echo "    an inert path selected a gate beyond the $n always gate(s)"; exit 1; }

# --- the gates whose runner is not available on demand. A gate the model marks `on-demand`
#     is planned only when the plan is asked for it: the nightly schedule, a dispatch and a
#     pull request labelled ci:full ask, a routine push does not. This is the rule that makes
#     the `ci` verdict arrive at all — on 2026-09-10 four master runs had every Linux job
#     finished, five of them red, and their verdict job had never started, because it needs
#     three macOS jobs that had been queued for between three and seven hours.
[ -n "$ondemand" ] || { echo "    the model marks no gate on-demand; the rule cannot be checked against it"; exit 1; }
p="$(plan .github/workflows/validate.yml)"    # a full plan that did not ask for them
for g in $ondemand; do
  [ "$(printf '%s' "$p" | jq -r --arg g "$g" '.gates[] | select(.id == $g) | .selected')" = false ] \
    || { echo "    the full plan of a routine push selected the on-demand gate $g"; exit 1; }
  printf '%s' "$p" | jq -r --arg g "$g" '.gates[] | select(.id == $g) | .reason' | grep -q '^on demand only' \
    || { echo "    the plan does not say why $g was not planned"; exit 1; }
done
printf '%s' "$p" | jq -r .reason | grep -q 'without the on-demand gates' \
  || { echo "    a full plan that leaves the on-demand gates out does not say so in its reason"; exit 1; }
p="$(plan_od .github/workflows/validate.yml)"
for g in $ondemand; do
  [ "$(printf '%s' "$p" | jq -r --arg g "$g" '.gates[] | select(.id == $g) | .selected')" = true ] \
    || { echo "    --on-demand did not plan $g"; exit 1; }
done
# the two formats the callers read. The text one takes run/skip from the selection and not
# from the presence of a reason: an on-demand gate carries a reason and did not run, and
# printing it as `run` is how a reader is told a gate ran that never did.
printf '.github/workflows/validate.yml\n' | "$PLAN" --files - --format text > od.txt
printf '.github/workflows/validate.yml\n' | "$PLAN" --files - --format github > od-gh.txt
for g in $ondemand; do
  grep -qE "^  skip  $g +on demand only" od.txt || { cat od.txt; echo "    the text plan does not report $g as skipped"; exit 1; }
  grep -qx "$(printf '%s' "$g" | tr '-' '_')=false" od-gh.txt || { cat od-gh.txt; echo "    the github plan does not report $g as false, so its job would run"; exit 1; }
done
# nothing implies its way past the cadence: a selected gate that names an on-demand one still
# does not bring it into a plan that did not ask
od1="${ondemand%% *}"
awk -v g="$od1" '/^  - id: core-check$/{print; print "    implies: [" g "]"; next} {print}' "$MODEL" > implied.yaml
grep -q "implies: \[$od1\]" implied.yaml || { echo "    the mutation did not take"; exit 1; }
s="$(: | "$PLAN" --model implied.yaml --files - | jq -r '.selected | join(" ")')"
lacks "$s" "$od1" || { echo "    $od1 was implied into a plan that did not ask for it: $s"; exit 1; }
s="$(: | "$PLAN" --model implied.yaml --files - --on-demand | jq -r '.selected | join(" ")')"
has "$s" "$od1" || { echo "    the mutation is vacuous: --on-demand did not bring $od1 in either: $s"; exit 1; }
# the field is present and true, or absent; and a gate cannot be both always and on demand
awk '/^  - id: core-check$/{print; print "    on-demand: false"; next} {print}' "$MODEL" > od-false.yaml
expect_exit 10 "$PLAN" --model od-false.yaml --check
expect_grep 'on-demand is true or absent'
awk '/^  - id: core-check$/{print; print "    on-demand: true"; next} {print}' "$MODEL" > od-always.yaml
expect_exit 10 "$PLAN" --model od-always.yaml --check
expect_grep 'both always and on-demand'

# --- the union: two classes select the union of their gates; implied and required gates
#     come with their parent and say so
p="$(plan apps/majordomus-cli/src/lib.rs site/templates/base.html)"
s="$(printf '%s' "$p" | jq -r '.selected | join(" ")')"
for g in rust-check site-probe; do has "$s" "$g" || { echo "    the union lost $g: $s"; exit 1; }; done
[ "$(printf '%s' "$p" | jq -r '.gates[] | select(.id == "rust-integration") | .reason')" = "implied by rust-check" ] || { echo "    rust-integration is not reported as implied"; exit 1; }
[ "$(plan scripts/site-probe | jq -r '.gates[] | select(.id == "site-build") | .reason')" = "class site" ] || { echo "    site-build is not selected by the site class"; exit 1; }
# every gate the model declares carries its job, so the verdict can find it
[ "$(printf '%s' "$p" | jq -r '[.gates[] | select(.job == "")] | length')" = 0 ] || { echo "    a gate has no job in the plan"; exit 1; }
# the github format is one name=value per gate, underscored, plus mode and the selection
plan apps/majordomus-cli/src/lib.rs >/dev/null
printf 'apps/majordomus-cli/src/lib.rs\n' | "$PLAN" --files - --format github > gh.txt
grep -qx 'mode=affected' gh.txt && grep -qx 'rust_check=true' gh.txt && grep -qx 'site_build=false' gh.txt || { cat gh.txt; echo "    the github format is wrong"; exit 1; }
# the plan is deterministic
[ "$(plan docs/DESIGN.md | jq -S .)" = "$(plan docs/DESIGN.md | jq -S .)" ] || { echo "    two plans of one change differ"; exit 1; }

# --- the verdict: green only when every selected gate's job succeeded; red on a failure, a
#     cancellation, a selected gate whose job was skipped, a failed plan, or an empty selection
plan docs/DESIGN.md > plan.json
# A needs context: the jobs the caller names, plus every other job the model declares, as
# skipped. Naming them all here would go stale the day a job is added, and the verdict would
# then report the new job as absent rather than the case reporting what it is testing.
JOBS="$(awk '/^    job: /{print $2}' "$MODEL" | LC_ALL=C sort -u | tr '\n' ' ')"
needs() {
  jq -n --arg s "$1" --arg jobs "plan $JOBS" '
    ($s | split(",") | map(split("=") | {key: .[0], value: {result: .[1]}}) | from_entries) as $named
    | ($jobs | split(" ") | map(select(length > 0)) | map({key: ., value: {result: "skipped"}}) | from_entries) as $rest
    | $rest + $named'
}
needs "plan=success,structure=success,suite=success,rust=success,coverage=skipped,bench=skipped,site=success,macos=skipped" > n.json
expect_exit 0 "$VERDICT" --plan plan.json --needs n.json --summary summary.md
grep -q '^## ci: green' summary.md || { cat summary.md; echo "    a green verdict did not say so"; exit 1; }
grep -q '| rust-coverage | coverage | skipped' summary.md || { echo "    the summary does not show the skipped gate"; exit 1; }
needs "plan=success,structure=success,suite=failure,rust=success,coverage=skipped,bench=skipped,site=success,macos=skipped" > n.json
expect_exit 1 "$VERDICT" --plan plan.json --needs n.json --summary summary.md
expect_grep 'gate shell-suite was selected and its job suite reported failure'
needs "plan=success,structure=success,suite=success,rust=cancelled,coverage=skipped,bench=skipped,site=success,macos=skipped" > n.json
expect_exit 1 "$VERDICT" --plan plan.json --needs n.json --summary summary.md
needs "plan=success,structure=success,suite=success,rust=success,coverage=skipped,bench=skipped,site=skipped,macos=skipped" > n.json
expect_exit 1 "$VERDICT" --plan plan.json --needs n.json --summary summary.md
expect_grep 'gate site-build was selected and its job site reported skipped'
needs "plan=failure,structure=skipped,suite=skipped,rust=skipped,coverage=skipped,bench=skipped,site=skipped,macos=skipped" > n.json
expect_exit 1 "$VERDICT" --plan plan.json --needs n.json --summary summary.md
expect_grep 'planning did not succeed'
# a job that ran for another gate and failed is a failure even for the gate that did not plan it
needs "plan=success,structure=success,suite=success,rust=success,coverage=failure,bench=skipped,site=success,macos=skipped" > n.json
expect_exit 1 "$VERDICT" --plan plan.json --needs n.json --summary summary.md
# an empty selection cannot pass
jq '.gates |= map(.selected = false) | .selected = []' plan.json > empty.json
needs "plan=success,structure=skipped,suite=skipped,rust=skipped,coverage=skipped,bench=skipped,site=skipped,macos=skipped" > n.json
expect_exit 1 "$VERDICT" --plan empty.json --needs n.json --summary summary.md
expect_grep 'selected no gate'

# --- the parallel runner keeps the serial runner's semantics: a failing case turns the run
#     red with its log rendered, the exclusive cases run after the pool, one at a time, the
#     report carries every case, and a case that writes into the checkout is caught
H="$T/harness"; mkdir -p "$H/test/cases" "$H/bin"
cp "$ROOT/test/run.sh" "$H/test/run.sh"; cp "$ROOT/test/lib.sh" "$H/test/lib.sh"
git -C "$H" init -q . 2>/dev/null; git -C "$H" add -A >/dev/null; git -C "$H" -c user.email=t@e.com -c user.name=t commit -qm harness
for i in 1 2 3 4 5; do printf 'sleep 1; echo "case %s ran"\n' "$i" > "$H/test/cases/p$i.sh"; done
printf 'echo "this one explains itself"; exit 1\n' > "$H/test/cases/p_fails.sh"
printf '# majordomus-exclusive: it must see no other case running\necho exclusive ran\n' > "$H/test/cases/x1.sh"
out="$(MJ_TEST_JOBS=3 MJ_TEST_REPORT="$T/report.tsv" bash "$H/test/run.sh" 2>&1)" && { echo "    a failing case did not turn the parallel run red"; exit 1; }
printf '%s\n' "$out" | grep -q 'this one explains itself' || { printf '%s\n' "$out"; echo "    the failing case's log was not rendered"; exit 1; }
printf '%s\n' "$out" | grep -q '^FAIL p_fails$' || { echo "    the failing case has no FAIL line"; exit 1; }
printf '%s\n' "$out" | grep -q '^tests: 6 passed, 1 failed$' || { printf '%s\n' "$out"; echo "    the summary is not deterministic (6 passed, 1 failed)"; exit 1; }
printf '%s\n' "$out" | grep -q 'exclusive cases, one at a time' || { echo "    the exclusive phase did not run"; exit 1; }
[ "$(wc -l < "$T/report.tsv" | tr -d ' ')" = 7 ] || { cat "$T/report.tsv"; echo "    the report does not carry every case"; exit 1; }
grep -q "^x1	ok	[0-9]*	exclusive$" "$T/report.tsv" || { cat "$T/report.tsv"; echo "    the report does not mark the exclusive case"; exit 1; }
grep -q "^p1	ok	[0-9]*	parallel$" "$T/report.tsv" || { echo "    the report does not mark a parallel case"; exit 1; }
# the pool is a pool: five one-second cases run faster with five workers than one after
# the other (the failing case is removed first, so both runs are green)
rm -f "$H/test/cases/p_fails.sh"
t0="$(date +%s)"; MJ_TEST_JOBS=5 bash "$H/test/run.sh" >/dev/null 2>&1 || { echo "    the green harness failed with five workers"; exit 1; }; par=$(( $(date +%s) - t0 ))
t0="$(date +%s)"; bash "$H/test/run.sh" >/dev/null 2>&1 || { echo "    the green harness failed serially"; exit 1; }; ser=$(( $(date +%s) - t0 ))
[ "$par" -lt "$ser" ] || { echo "    five one-second cases took ${par}s with five workers and ${ser}s serially"; exit 1; }
# a case that writes into the checkout during the parallel phase is caught and named
printf 'echo dirty > "$ROOT/test/cases/dirt.txt"\n' > "$H/test/cases/p_writes.sh"
out="$(MJ_TEST_JOBS=2 bash "$H/test/run.sh" 2>&1)" && { echo "    a case that wrote into the checkout did not turn the run red"; exit 1; }
printf '%s\n' "$out" | grep -q 'the checkout changed during the parallel phase: test/cases/dirt.txt' || { printf '%s\n' "$out"; echo "    the dirtied path is not named"; exit 1; }
rm -f "$H/test/cases/dirt.txt" "$H/test/cases/p_writes.sh"
# the filter and the empty directory are usage errors in parallel mode too
MJ_TEST_JOBS=2 bash "$H/test/run.sh" no_such_case >/dev/null 2>&1 && { echo "    a filter matching nothing passed"; exit 1; }
rm -f "$H"/test/cases/*.sh
MJ_TEST_JOBS=2 bash "$H/test/run.sh" >/dev/null 2>&1 && { echo "    an empty case directory passed"; exit 1; }
MJ_TEST_JOBS=0 bash "$H/test/run.sh" >/dev/null 2>&1 && { echo "    MJ_TEST_JOBS=0 was accepted"; exit 1; }
MJ_TEST_JOBS=two bash "$H/test/run.sh" >/dev/null 2>&1 && { echo "    MJ_TEST_JOBS=two was accepted"; exit 1; }

# --- the prebuilt executable: rust_bin hands a case MAJORDOMUS_BIN when one is given and
#     never builds; refuses one that is not executable; and asks to skip when there is
#     neither an executable nor cargo
printf '#!/bin/sh\necho fake\n' > "$T/fake-majordomus"; chmod +x "$T/fake-majordomus"
[ "$(MAJORDOMUS_BIN="$T/fake-majordomus" rust_bin)" = "$T/fake-majordomus" ] || { echo "    rust_bin did not hand back MAJORDOMUS_BIN"; exit 1; }
rc=0; MAJORDOMUS_BIN="$T/no-such-file" rust_bin >/dev/null 2>&1 || rc=$?
[ "$rc" = 1 ] || { echo "    rust_bin accepted a MAJORDOMUS_BIN that is not executable (rc $rc)"; exit 1; }
rc=0; MAJORDOMUS_BIN='' PATH="/usr/bin:/bin" rust_bin >/dev/null 2>&1 || rc=$?
[ "$rc" = 3 ] || { echo "    rust_bin with neither cargo nor MAJORDOMUS_BIN returned $rc, not the skip code 3"; exit 1; }
# --- a job gated on a plan output the plan job does not expose is a gate that never runs.
#     The plan step emits one output per gate, but a workflow job reads them through the
#     `outputs:` block, and GitHub does not complain about a property that is not there — the
#     condition is simply false for ever. `cockpit-assets` was skipped that way, silently, by
#     every run. Nothing derives that block, so this is what keeps it honest.
#     $ROOT and not $H: the harness above is a synthetic checkout with no workflow in it,
#     and a loop over a file that does not exist finds nothing missing and passes.
W="$ROOT/.github/workflows/validate.yml"
[ -f "$W" ] || { echo "    the workflow this check is about is not at $W"; exit 1; }
grep -q 'needs\.plan\.outputs\.' "$W" || { echo "    no job reads a plan output; this check has stopped checking anything"; exit 1; }
missing=""
for ref in $(grep -oE 'needs\.plan\.outputs\.[a-z_]+' "$W" | sed 's/.*\.//' | LC_ALL=C sort -u); do
  grep -qE "^      $ref: \\\$\{\{ steps\.plan\.outputs\.$ref \}\}" "$W" || missing="$missing $ref"
done
[ -z "$missing" ] || { echo "    job(s) gated on plan output(s) the plan job never exposes:$missing"; exit 1; }

echo "    the plan follows the model, the verdict follows the plan, the runner keeps its semantics"

# majordomus-covers: none
# claims: ci-planned-gates, ci-verdict, suite-parallel
# The CI planner, the verdict and the parallel runner: the pieces validate.yml is an adapter
# over. The planner is driven with listed paths, never with this checkout's diff, so the
# case is the same on every machine; the verdict with fixture plans and needs contexts; the
# runner with a private harness of throwaway cases.
. "$ROOT/test/lib.sh"
PLAN="$ROOT/scripts/ci-plan"; VERDICT="$ROOT/scripts/ci/verdict"; MODEL="$ROOT/.ai/repo/ci/gates.yaml"
command -v jq >/dev/null 2>&1 || skip "jq absent"
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
# every gate is in the full plan but the ones another planned gate subsumes, each of which says so
[ "$(printf '%s' "$p" | jq -r '[.gates[] | select(.selected | not) | select(.reason | startswith("subsumed by ") | not)] | length')" = 0 ] || { echo "    the full plan left a gate out"; exit 1; }
p="$(plan some/new/thing.txt)"
[ "$(printf '%s' "$p" | jq -r .mode)" = full ] || { echo "    an unclassified path did not escalate"; exit 1; }
printf '%s' "$p" | jq -r .unclassified[0] | grep -qx 'some/new/thing.txt' || { echo "    the plan does not name the unclassified path"; exit 1; }

# --- the pipeline's data files are excepted from the pipeline class by name: each one plans
#     the gates of the classes it still falls in, never the full plan. The list is read from
#     the model, so a file added to `except` is held to this the day it is added.
excepted="$(awk '/^  - id: ci$/{c=1} c && /^    except: /{sub(/^    except: \[/, ""); sub(/\]$/, ""); gsub(/, /, " "); print; exit}' "$MODEL")"
[ -n "$excepted" ] || { echo "    the ci class excepts nothing; the data-file planning is unasserted"; exit 1; }
for f in $excepted; do
  p="$(plan "$f")"
  [ "$(printf '%s' "$p" | jq -r .mode)" = affected ] || { echo "    the pipeline data file $f escalated: $(printf '%s' "$p" | jq -r .reason)"; exit 1; }
  s="$(printf '%s' "$p" | jq -r '.selected | join(" ")')"
  has "$s" shell-suite || { echo "    the pipeline data file $f did not select the suite: $s"; exit 1; }
done
s="$(selected .ai/repo/ci/suite-durations.tsv)"
has "$s" shell-suite || { echo "    the shard deal's durations did not select the suite: $s"; exit 1; }
s="$(selected .ai/repo/ci/case-numbers-baseline.txt)"
has "$s" case-numbers || { echo "    the case-number ratchet did not select its gate: $s"; exit 1; }
s="$(selected .ai/repo/ci/github-drift-baseline.txt)"
has "$s" github-projection || { echo "    the GitHub drift ratchet did not select its gate: $s"; exit 1; }
# the pipeline's definitions still escalate, and so does a file nobody has named yet: an
# exception is one path, and the directory stays escalating around it
for f in .ai/repo/ci/gates.yaml .ai/repo/ci/suite.yaml .ai/repo/ci/a-file-added-tomorrow.yaml; do
  [ "$(plan "$f" | jq -r .mode)" = full ] || { echo "    $f under the pipeline class did not escalate"; exit 1; }
done
# and an exception that is a pattern is refused: it would carve an open-ended hole
awk '/^  - id: ci$/{c=1} c && /^    except: /{print "    except: [.ai/repo/ci/*.tsv]"; c=0; next} {print}' "$MODEL" > broad.yaml
grep -q 'except: \[.ai/repo/ci/\*.tsv\]' broad.yaml || { echo "    the except mutation did not take"; exit 1; }
expect_exit 10 "$PLAN" --model broad.yaml --check
expect_grep 'an exception names one path'

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

# --- the release-record shape: a release record and its projections plan the always gates
#     and the shape's own, never the union. The attributes come from a fixture checkout whose
#     .gitattributes is this one's with the line the record's own derive adds for its release
#     file, so the case is about the tree a record pull request proposes, whatever release this
#     checkout last recorded. The change is #976's (v0.21.0), path for path.
A="$T/attr"; mkdir -p "$A"; git -C "$A" init -q . 2>/dev/null
END_MARK='# <<< generated by scripts/gitattributes <<<'
grep -qxF "$END_MARK" "$ROOT/.gitattributes" || { echo "    .gitattributes has no generated block; the shape has nothing to read"; exit 1; }
attrs() { # attrs [extra line before the block's end] [extra line after the file]
  awk -v e="$END_MARK" -v add="${1:-}" '
    $0 == e { if (add != "") print add; if (!seen) print "site/static/releases/v0.21.0.json   merge=derived" }
    $1 == "site/static/releases/v0.21.0.json" { seen = 1 }
    { print }' "$ROOT/.gitattributes" > "$A/.gitattributes"
  [ -z "${2:-}" ] || printf '%s\n' "$2" >> "$A/.gitattributes"
}
attrs
cat > rec976.txt <<'L'
.ai/repo/releases/v0.21.0.yaml
.gitattributes
docs/INSTALL.md
docs/PAGES_STATUS.md
docs/PLAN_STATUS.md
docs/generated/artifacts.json
docs/generated/artifacts.md
docs/generated/artifacts.yaml
docs/generated/changelog.json
docs/generated/changelog.md
docs/generated/changelog.yaml
docs/generated/graph.json
site/content/docs/install.md
site/content/docs/pages-status.md
site/data/generated/artifacts.json
site/data/generated/changelog.json
site/data/generated/entities.json
site/data/generated/project.json
site/data/generated/source.json
site/data/registry/distribution.json
site/data/registry/product-graph.json
site/data/registry/product.json
site/data/registry/registry.json
site/static/releases/latest.json
site/static/releases/v0.21.0.json
L
[ "$(wc -l < rec976.txt | tr -d ' ')" = 25 ] || { echo "    the #976 list is not its 25 paths"; exit 1; }
rplan() { "$PLAN" --attr-root "$A" --files - "$@"; }
# "planned as today": the same model with its shapes taken out, over the same paths
awk '/^shapes:/{s=1} /^classes:/{s=0} !s' "$MODEL" > noshape.yaml
grep -q '^shapes:' noshape.yaml && { echo "    the shapes did not come out of the copy"; exit 1; }
as_today() { "$PLAN" --model noshape.yaml --files - | jq -r '.selected | join(" ")'; }
shape_gates="$(awk '/^shapes:/{s=1} s && /^    gates: /{sub(/^    gates: \[/, ""); sub(/\]$/, ""); gsub(/, /, " "); print; exit}' "$MODEL")"
[ -n "$shape_gates" ] || { echo "    the model's release-record shape names no gate"; exit 1; }

p="$(rplan < rec976.txt)"
[ "$(printf '%s' "$p" | jq -r '.shape.id // ""')" = release-record ] || { printf '%s' "$p" | jq -r .reason; echo "    #976's change is not of the release-record shape"; exit 1; }
[ "$(printf '%s' "$p" | jq -r .mode)" = affected ] || { echo "    the shaped plan is not an affected one"; exit 1; }
printf '%s' "$p" | jq -r .reason | grep -q 'shape release-record' || { echo "    the plan's reason does not name the shape"; exit 1; }
[ "$(printf '%s' "$p" | jq -r '.shape.derived')" = 24 ] || { echo "    the shape did not count #976's 24 derived files: $(printf '%s' "$p" | jq -c .shape)"; exit 1; }
s="$(printf '%s' "$p" | jq -r '.selected | join(" ")')"
# exactly the always gates and the shape's, nothing of the union
for g in $always $shape_gates; do has "$s" "$g" || { echo "    the shaped plan lacks $g: $s"; exit 1; }; done
always_w="$(printf '%s ' $always)"
for g in $s; do has "$always_w $shape_gates" "$g" || { echo "    the shaped plan selected $g, which neither runs always nor belongs to the shape"; exit 1; }; done
for g in shell-suite site-probe ui-audit interaction-probe rust-check; do lacks "$s" "$g" || { echo "    the shaped plan selected $g: $s"; exit 1; }; done
# ... which is narrower than the union the same paths plan without the shape
u="$(as_today < rec976.txt)"; has "$u" shell-suite || { echo "    the mutation is vacuous: without the shape #976 did not plan the suite: $u"; exit 1; }
[ "$(printf '%s' "$p" | jq -r '.gates[] | select(.id == "release-record-cases") | .reason')" = "shape release-record" ] || { echo "    a shape gate does not say the shape planned it"; exit 1; }
printf '%s' "$p" | jq -r '.gates[] | select(.id == "shell-suite") | .reason' | grep -q '^left out by the shape release-record' || { echo "    a gate the shape left out does not say so"; exit 1; }
rplan --format text < rec976.txt | grep -q '^shape: release-record (records: .ai/repo/releases/v0.21.0.yaml; derived: 24)$' || { echo "    the text plan does not name the shape"; exit 1; }
rplan --format github < rec976.txt | grep -qx 'shape=release-record' || { echo "    the github plan does not name the shape"; exit 1; }

# the same change and one authored file is planned exactly as today, and says why it is not shaped
for extra in docs/RELEASE.md apps/majordomus-cli/src/main.rs; do
  { cat rec976.txt; echo "$extra"; } > plus.txt
  p="$(rplan < plus.txt)"
  [ "$(printf '%s' "$p" | jq -r '.shape')" = null ] || { echo "    #976 plus $extra was narrowed by a shape"; exit 1; }
  [ "$(printf '%s' "$p" | jq -r '.selected | join(" ")')" = "$(as_today < plus.txt)" ] || { echo "    #976 plus $extra is not planned as without the shape"; exit 1; }
  printf '%s' "$p" | jq -r .reason | grep -qF "not the shape release-record: $extra" || { echo "    the plan does not say why #976 plus $extra is not shaped"; exit 1; }
done
# derived files without a record are not the shape
grep -v '^\.ai/repo/releases/' rec976.txt > derived-only.txt
p="$(rplan < derived-only.txt)"
[ "$(printf '%s' "$p" | jq -r '.shape')" = null ] && [ "$(printf '%s' "$p" | jq -r '.selected | join(" ")')" = "$(as_today < derived-only.txt)" ] \
  || { echo "    derived files without a record were narrowed"; exit 1; }
# a record and a path under a derived directory that no attribute marks — a release file the
# merge policy does not name — is not the shape
{ cat rec976.txt; echo site/static/releases/v9.9.9.json; } > undeclared.txt
[ "$(rplan < undeclared.txt | jq -r '.shape')" = null ] || { echo "    a release file no attribute marks passed as derived"; exit 1; }
# git's answer alone does not make a file derived: a line outside the generated block, and a
# block pattern with no slash (git matches it at any depth), would each launder an authored file
attrs "" "docs/RELEASE.md merge=derived"
[ "$(git -C "$A" check-attr merge -- docs/RELEASE.md)" = 'docs/RELEASE.md: merge: derived' ] || { echo "    the laundering fixture did not take"; exit 1; }
{ cat rec976.txt; echo docs/RELEASE.md; } > laundered.txt
[ "$(rplan < laundered.txt | jq -r '.shape')" = null ] || { echo "    an attribute outside the generated block made an authored file derived"; exit 1; }
attrs
deep="$(git -C "$ROOT" ls-files -- 'campaigns/*/AGENTS.md' 'campaigns/*/CLAUDE.md' | head -n 1)"
if [ -n "$deep" ] && git -C "$A" check-attr merge -- "$deep" | grep -q ': merge: derived$'; then
  { cat rec976.txt; echo "$deep"; } > deep.txt
  [ "$(rplan < deep.txt | jq -r '.shape')" = null ] || { echo "    $deep, matched by a slash-less pattern at depth, passed as derived"; exit 1; }
fi
# --full is never narrowed, and a full plan does not name a shape
p="$("$PLAN" --attr-root "$A" --files rec976.txt --full "asked")"
[ "$(printf '%s' "$p" | jq -r .mode)" = full ] && [ "$(printf '%s' "$p" | jq -r .shape)" = null ] || { echo "    --full was narrowed by a shape"; exit 1; }
# a shape that does not resolve is refused by name
awk '/^shapes:/{s=1} s && /^    gates: /{print "    gates: [release-check, no-such-gate]"; s=0; next} {print}' "$MODEL" > shape-gate.yaml
grep -q 'no-such-gate' shape-gate.yaml || { echo "    the shape mutation did not take"; exit 1; }
expect_exit 10 "$PLAN" --model shape-gate.yaml --check
expect_grep 'shape release-record names a gate that does not exist: no-such-gate'
awk '/^shapes:/{s=1} /^classes:/{s=0} s && /^    records: /{next} {print}' "$MODEL" > shape-norec.yaml
grep -q '^    records: ' shape-norec.yaml && { echo "    the records mutation did not take"; exit 1; }
expect_exit 10 "$PLAN" --model shape-norec.yaml --check
expect_grep 'shape release-record names no record pattern'
awk '/^shapes:/{s=1} s && /^    others: /{print "    others: anything"; s=0; next} {print}' "$MODEL" > shape-others.yaml
expect_exit 10 "$PLAN" --model shape-others.yaml --check
expect_grep 'others must be derived'

# the cases the shape keeps: every case that names the release record or reads what is offered
# from it is in release-record-cases, and every case that gate names exists. Which cases
# those are is defined once, in scripts/ci/release-record-cases-check, the structure gate
# that asks it on every change to a case (I2716); this case asks the same script about this
# tree, so the definition has one home. This case names those paths too, as fixtures of the
# planner it is about, and the script exempts it by name.
expect_exit 0 "$ROOT/scripts/ci/release-record-cases-check"
expect_grep 'case\(s\) name the release record, each run by release-record-cases'

# --- the plan-record shape (I2662): a plan record and its projections plan the always gates
#     and the shape's own, never the union. The change is #1151's (I0806 recorded done), path
#     for path; with one Rust file, or one authored document, it is planned exactly as today.
cat > rec1151.txt <<'L'
.ai/repo/project/issues/I0806.yaml
docs/PAGES_STATUS.md
docs/PLAN_STATUS.md
docs/generated/artifacts.json
docs/generated/artifacts.md
docs/generated/artifacts.yaml
docs/generated/graph.json
site/content/docs/pages-status.md
site/data/generated/plan.json
site/data/generated/roadmap.json
site/data/generated/source.json
site/data/registry/registry.json
L
p="$(rplan < rec1151.txt)"
[ "$(printf '%s' "$p" | jq -r '.shape.id // ""')" = plan-record ] || { printf '%s' "$p" | jq -r .reason; echo "    #1151's change is not of the plan-record shape"; exit 1; }
[ "$(printf '%s' "$p" | jq -r '.shape.derived')" = 11 ] || { echo "    the shape did not count #1151's 11 derived files: $(printf '%s' "$p" | jq -c .shape)"; exit 1; }
s="$(printf '%s' "$p" | jq -r '.selected | join(" ")')"
pr_gates="$(awk '/^  - id: plan-record$/{f=1} f && /^    gates: /{sub(/^    gates: \[/, ""); sub(/\]$/, ""); gsub(/, /, " "); print; exit}' "$MODEL")"
[ -n "$pr_gates" ] || { echo "    the model's plan-record shape names no gate"; exit 1; }
for g in $always $pr_gates; do has "$s" "$g" || { echo "    the plan-record plan lacks $g: $s"; exit 1; }; done
for g in $s; do has "$always_w $pr_gates" "$g" || { echo "    the plan-record plan selected $g, which neither runs always nor belongs to the shape"; exit 1; }; done
for g in shell-suite site-probe ui-audit interaction-probe rust-check; do lacks "$s" "$g" || { echo "    the plan-record plan selected $g: $s"; exit 1; }; done
u="$(as_today < rec1151.txt)"; has "$u" shell-suite || { echo "    the mutation is vacuous: without the shape #1151 did not plan the suite: $u"; exit 1; }
for extra in apps/majordomus-cli/src/plan.rs docs/DESIGN.md; do
  { cat rec1151.txt; echo "$extra"; } > plus.txt
  p="$(rplan < plus.txt)"
  [ "$(printf '%s' "$p" | jq -r '.shape')" = null ] || { echo "    #1151 plus $extra was narrowed by a shape"; exit 1; }
  s="$(printf '%s' "$p" | jq -r '.selected | join(" ")')"
  [ "$s" = "$(as_today < plus.txt)" ] || { echo "    #1151 plus $extra is not planned as without the shape"; exit 1; }
  has "$s" shell-suite || { echo "    #1151 plus $extra did not plan the suite: $s"; exit 1; }
done
{ cat rec1151.txt; echo apps/majordomus-cli/src/plan.rs; } > plus.txt
has "$(rplan < plus.txt | jq -r '.selected | join(" ")')" rust-check || { echo "    a record with a Rust file did not plan rust-check"; exit 1; }

# the cases the plan-record shape keeps: every case that names a plan record path or
# PLAN_STATUS, or runs a `plan` subcommand, is in plan-record-cases, and every case that gate
# names exists. This case names those paths as fixtures and is left out of the grep.
runs="$(awk '/^  - id: plan-record-cases$/{f=1} f && /^    runs: /{sub(/^    runs: /, ""); print; exit}' "$MODEL")"
case "$runs" in *'test/run.sh --no-skips '*) ;; *) echo "    plan-record-cases does not run named cases with --no-skips: $runs"; exit 1 ;; esac
named=" ${runs#*--no-skips } "
want="$(grep -l -E '\.ai/repo/project/(issues|milestones)|PLAN_STATUS|majordomus"? plan |mj_plan|"\$MJ" plan |\bplan (validate|status|show|done|evidence|list|next|waves|ready|blocked|roadmap|verify|start|body|graph|rgraph)\b' "$ROOT"/test/cases/*.sh \
  | sed 's#.*/##; s#\.sh$##' | grep -vx 94_ci_plan)"
[ -n "$want" ] || { echo "    no case names a plan record; the assertion is vacuous"; exit 1; }
for c in $want; do has "$named" "$c" || { echo "    $c reads the plan and plan-record-cases does not run it"; exit 1; }; done
for c in $named; do [ -f "$ROOT/test/cases/$c.sh" ] || { echo "    plan-record-cases names $c, which is not a case"; exit 1; }; done

# --- a gate whose cases the planned suite already runs is not planned beside it. The two case
#     gates exist for the record shapes, which do not run the suite; a plan that does (the
#     full one) ran the same cases twice, ~34 minutes of the rust job's third lane (I3937).
#     The relation is declared in the model (`subsumed-by`) and applied by the planner after
#     the whole selection, so the plan says why the gate is absent.
for g in release-record-cases plan-record-cases; do
  awk -v g="$g" '$0 == "  - id: " g { f = 1; next } f && /^  - id: / { exit } f && /^    subsumed-by: shell-suite$/ { found = 1 } END { exit !found }' "$MODEL" \
    || { echo "    the model does not declare $g subsumed by shell-suite"; exit 1; }
done
full="$("$PLAN" --full)"
fs="$(printf '%s' "$full" | jq -r '.selected | join(" ")')"
has "$fs" shell-suite || { echo "    the full plan does not plan the suite: $fs"; exit 1; }
for g in release-record-cases plan-record-cases; do
  lacks "$fs" "$g" || { echo "    the full plan plans $g beside the suite that runs its cases"; exit 1; }
  [ "$(printf '%s' "$full" | jq -r --arg g "$g" '.gates[] | select(.id == $g) | .selected')" = false ] || { echo "    the full plan's entry for $g says it is selected"; exit 1; }
  printf '%s' "$full" | jq -r --arg g "$g" '.gates[] | select(.id == $g) | .reason' | grep -q '^subsumed by shell-suite: ' \
    || { echo "    the full plan does not say the suite subsumed $g"; exit 1; }
  "$PLAN" --full --format text | grep -q "^  skip  $g .*subsumed by shell-suite" || { echo "    the text plan does not show $g skipped as subsumed"; exit 1; }
  "$PLAN" --full --format github | grep -qx "$(printf '%s' "$g" | tr '-' '_')=false" || { echo "    the github output does not say $g is not planned"; exit 1; }
done
# nothing else leaves the full plan: it is the model's gates but the on-demand and subsumed ones
for g in $(printf '%s' "$full" | jq -r '.gates[] | select(.selected | not) | .id'); do
  has "$ondemand release-record-cases plan-record-cases" "$g" || { echo "    the full plan left out $g, which is neither on demand nor subsumed"; exit 1; }
done
# vacuity: without the declaration the same full plan plans both
grep -v '^    subsumed-by: ' "$MODEL" > unsubsumed.yaml
u="$("$PLAN" --model unsubsumed.yaml --full | jq -r '.selected | join(" ")')"
has "$u" release-record-cases && has "$u" plan-record-cases || { echo "    the mutation is vacuous: without subsumed-by the full plan lacks a case gate: $u"; exit 1; }
# the record shapes do not run the suite, so nothing subsumes the gates and they are planned
rr="$(rplan < rec976.txt)"; pr="$(rplan < rec1151.txt)"
has "$(printf '%s' "$rr" | jq -r '.selected | join(" ")')" release-record-cases || { echo "    the release-record shape no longer plans release-record-cases"; exit 1; }
has "$(printf '%s' "$pr" | jq -r '.selected | join(" ")')" plan-record-cases || { echo "    the plan-record shape no longer plans plan-record-cases"; exit 1; }
lacks "$(printf '%s' "$rr" | jq -r '.selected | join(" ")')" shell-suite && lacks "$(printf '%s' "$pr" | jq -r '.selected | join(" ")')" shell-suite \
  || { echo "    a record shape plans the suite"; exit 1; }
# the relation is validated: it names a gate that exists, not the gate itself, and no chain
sed 's/^    subsumed-by: shell-suite$/    subsumed-by: no-such-gate/' "$MODEL" > sub-missing.yaml
expect_exit 10 "$PLAN" --model sub-missing.yaml --check
expect_grep 'subsumed by a gate that does not exist: no-such-gate'
awk '/^  - id: release-record-cases$/{f=1} f && /^    subsumed-by: /{print "    subsumed-by: release-record-cases"; f=0; next} {print}' "$MODEL" > sub-self.yaml
expect_exit 10 "$PLAN" --model sub-self.yaml --check
expect_grep 'release-record-cases is subsumed by itself'
sed 's/^    subsumed-by: shell-suite$/    subsumed-by: plan-record-cases/' "$MODEL" > sub-chain.yaml
expect_exit 10 "$PLAN" --model sub-chain.yaml --check
expect_grep 'which is itself subsumed'

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
# Every selected gate's steps left a receipt saying they succeeded (scripts/ci/receipt):
# what the jobs' uploads carry, one file per job.
mkdir -p receipts/structure
jq -r '.gates[] | select(.selected) | "\(.id)\tsuccess\t\(.job)/fixture"' plan.json > receipts/structure/receipts.tsv
needs "plan=success,structure=success,suite=success,rust=success,coverage=skipped,bench=skipped,site=success,site-probe=success,ui=success,macos=skipped" > n.json
expect_exit 0 "$VERDICT" --plan plan.json --needs n.json --receipts receipts --summary summary.md
grep -q '^## ci: green' summary.md || { cat summary.md; echo "    a green verdict did not say so"; exit 1; }
grep -q '| rust-coverage | coverage | skipped' summary.md || { echo "    the summary does not show the skipped gate"; exit 1; }
needs "plan=success,structure=success,suite=failure,rust=success,coverage=skipped,bench=skipped,site=success,site-probe=success,ui=success,macos=skipped" > n.json
expect_exit 1 "$VERDICT" --plan plan.json --needs n.json --receipts receipts --summary summary.md
expect_grep 'gate shell-suite was selected and its job suite reported failure'
needs "plan=success,structure=success,suite=success,rust=cancelled,coverage=skipped,bench=skipped,site=success,site-probe=success,ui=success,macos=skipped" > n.json
expect_exit 1 "$VERDICT" --plan plan.json --needs n.json --receipts receipts --summary summary.md
needs "plan=success,structure=success,suite=success,rust=success,coverage=skipped,bench=skipped,site=skipped,site-probe=skipped,ui=skipped,macos=skipped" > n.json
expect_exit 1 "$VERDICT" --plan plan.json --needs n.json --receipts receipts --summary summary.md
expect_grep 'gate site-build was selected and its job site reported skipped'
needs "plan=failure,structure=skipped,suite=skipped,rust=skipped,coverage=skipped,bench=skipped,site=skipped,site-probe=skipped,ui=skipped,macos=skipped" > n.json
expect_exit 1 "$VERDICT" --plan plan.json --needs n.json --receipts receipts --summary summary.md
expect_grep 'planning did not succeed'
# a job that ran for another gate and failed is a failure even for the gate that did not plan it
needs "plan=success,structure=success,suite=success,rust=success,coverage=failure,bench=skipped,site=success,site-probe=success,ui=success,macos=skipped" > n.json
expect_exit 1 "$VERDICT" --plan plan.json --needs n.json --receipts receipts --summary summary.md
# --- receipts: a job's `success` says no step failed, not that the gate's step ran. A
#     selected gate whose job succeeded is red without a receipt, and red when any of its
#     receipts is not `success` — a step its condition skipped, a step cancelled, a failure
#     a `continue-on-error` swallowed. A gate the plan left out needs none.
needs "plan=success,structure=success,suite=success,rust=success,coverage=skipped,bench=skipped,site=success,site-probe=success,ui=success,macos=skipped" > n.json
cp receipts/structure/receipts.tsv all.tsv
grep -v '^site-probe	' all.tsv > receipts/structure/receipts.tsv
expect_exit 1 "$VERDICT" --plan plan.json --needs n.json --receipts receipts --summary summary.md
expect_grep 'gate site-probe was selected and its job site-probe succeeded, but no step left a receipt for it'
grep -q '| site-probe | site-probe | .* | success, \*\*no receipt\*\* |' summary.md || { cat summary.md; echo "    the summary does not show the gate without a receipt"; exit 1; }
for outcome in skipped cancelled failure ''; do
  cp all.tsv receipts/structure/receipts.tsv
  # a second job's receipts beside the first: every *.tsv beneath the directory is read
  mkdir -p receipts/site; printf 'site-build\t%s\tsite/build the site\n' "$outcome" > receipts/site/receipts.tsv
  expect_exit 1 "$VERDICT" --plan plan.json --needs n.json --receipts receipts --summary summary.md
  # an empty outcome is a step id that named no step: read as absent, never as success
  expect_grep "gate site-build was selected and its job site succeeded, but a step of it reported ${outcome:-absent}"
done
rm -rf receipts/site; cp all.tsv receipts/structure/receipts.tsv
# a receipt of a gate the plan left out is read and changes nothing
printf 'rust-bench\tskipped\tbench/fixture\n' >> receipts/structure/receipts.tsv
expect_exit 0 "$VERDICT" --plan plan.json --needs n.json --receipts receipts --summary summary.md
# no receipts at all — the artifacts never arrived — is red for every selected gate, not a pass
expect_exit 1 "$VERDICT" --plan plan.json --needs n.json --receipts "$T/no-such-directory" --summary summary.md
expect_grep 'gate shell-lint was selected and its job structure succeeded, but no step left a receipt for it'
# and a verdict asked without the receipts is a usage error, never the old job-only verdict
expect_exit 2 "$VERDICT" --plan plan.json --needs n.json --summary summary.md
expect_grep 'are required'
cp all.tsv receipts/structure/receipts.tsv

# --- the writers: timed and run-plan write a receipt per step and per gate when
#     MJ_CI_RECEIPTS is set; the workflow's closing steps write the rest from step outcomes
MJ_CI_RECEIPTS="$T/r.tsv" "$ROOT/scripts/ci/timed" --bound 30 site-build "a step" "$T/rows.tsv" true
MJ_CI_RECEIPTS="$T/r.tsv" "$ROOT/scripts/ci/timed" --bound 30 site-probe "a step" "$T/rows.tsv" false && { echo "    timed swallowed a failure"; exit 1; }
[ "$(cut -f1,2 "$T/r.tsv")" = "$(printf 'site-build\tsuccess\nsite-probe\tfailure')" ] \
  || { cat "$T/r.tsv"; echo "    timed did not write each step's receipt"; exit 1; }
# run-plan writes one per gate it ran, and none for a gate a step of its own runs (--skip)
jq -n '{schema: 1, mode: "affected", reason: "fixture", selected: ["gate-a", "gate-b", "gate-c"],
        gates: [{id: "gate-a", job: "j", runs: "true", selected: true, reason: "x"},
                {id: "gate-b", job: "j", runs: "false", selected: true, reason: "x"},
                {id: "gate-c", job: "j", runs: "true", selected: true, reason: "x"}]}' > "$T/rp.json"
MJ_CI_RECEIPTS="$T/rp.tsv" "$ROOT/scripts/ci/run-plan" --plan "$T/rp.json" --job j --skip gate-c >/dev/null 2>&1 \
  && { echo "    run-plan passed a plan with a failing gate"; exit 1; }
[ "$(cut -f1,2 "$T/rp.tsv")" = "$(printf 'gate-a\tsuccess\ngate-b\tfailure')" ] \
  || { cat "$T/rp.tsv"; echo "    run-plan did not write a receipt per gate it ran, and none for the skipped one"; exit 1; }

# an empty selection cannot pass
jq '.gates |= map(.selected = false) | .selected = []' plan.json > empty.json
needs "plan=success,structure=skipped,suite=skipped,rust=skipped,coverage=skipped,bench=skipped,site=skipped,site-probe=skipped,ui=skipped,macos=skipped" > n.json
expect_exit 1 "$VERDICT" --plan empty.json --needs n.json --receipts receipts --summary summary.md
expect_grep 'selected no gate'

# --- the parallel runner keeps the serial runner's semantics: a failing case turns the run
#     red with its log rendered, the exclusive cases run after the pool, one at a time, the
#     report carries every case, and a case that writes into the checkout is caught
H="$T/harness"; mkdir -p "$H/test/cases" "$H/bin" "$H/lib"
cp "$ROOT/test/run.sh" "$H/test/run.sh"; cp "$ROOT/test/lib.sh" "$H/test/lib.sh"
# test/lib.sh sources the tool's own lib/sha256.sh
cp "$ROOT/lib/sha256.sh" "$H/lib/sha256.sh"
git -C "$H" init -q . 2>/dev/null; git -C "$H" add -A >/dev/null; git -C "$H" -c user.email=t@e.com -c user.name=t commit -qm harness
for i in 1 2 3 4 5; do printf 'sleep 1; echo "case %s ran"\n' "$i" > "$H/test/cases/p$i.sh"; done
printf 'echo "this one explains itself"; exit 1\n' > "$H/test/cases/p_fails.sh"
printf '# majordomus-exclusive: it must see no other case running\necho exclusive ran\n' > "$H/test/cases/x1.sh"
out="$(MJ_TEST_JOBS=3 MJ_TEST_REPORT="$T/report.tsv" bash "$H/test/run.sh" 2>&1)" && { echo "    a failing case did not turn the parallel run red"; exit 1; }
printf '%s\n' "$out" | grep -q 'this one explains itself' || { printf '%s\n' "$out"; echo "    the failing case's log was not rendered"; exit 1; }
printf '%s\n' "$out" | grep -q '^FAIL p_fails$' || { echo "    the failing case has no FAIL line"; exit 1; }
printf '%s\n' "$out" | grep -q '^tests: 6 passed, 1 failed, 0 skipped$' || { printf '%s\n' "$out"; echo "    the summary is not deterministic (6 passed, 1 failed, 0 skipped)"; exit 1; }
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
# a case runs only in the fixture the runner made for it: started by hand, it writes its
# fixture into whatever directory it is in, a checkout included, so test/lib.sh refuses
# before the case's first line runs. This case writes and stages a file where it stands.
printf '. "$ROOT/test/lib.sh"\nprintf stub > stub.txt\ngit add -A\n' > "$H/test/cases/p_by_hand.sh"
mkdir -p "$T/away" "$T/other/test"; : > "$T/other/test/run.sh"
harness_state() { git -C "$H" status --porcelain --untracked-files=all; git -C "$H" diff --cached --name-only; }
before="$(harness_state)"
by_hand() {   # by_hand <label> <dir> [VAR=value...]: the case run by hand in <dir> is refused
  local label="$1" dir="$2" rc=0; shift 2
  out="$(cd "$dir" && env -u T "$@" ROOT="$H" bash -eu "$H/test/cases/p_by_hand.sh" 2>&1)" || rc=$?
  [ "$rc" = 1 ] || { printf '%s\n' "$out"; echo "    a case run by hand $label exited $rc, not 1"; exit 1; }
  printf '%s\n' "$out" | grep -q 'run this case through test/run.sh' \
    || { printf '%s\n' "$out"; echo "    a case run by hand $label does not say to run it through test/run.sh"; exit 1; }
  [ ! -e "$dir/stub.txt" ] || { echo "    a case run by hand $label wrote its fixture into $dir"; exit 1; }
}
by_hand "from the checkout, with no fixture" "$H"
by_hand "from the checkout, naming a fixture it does not stand in" "$H" T="$T/away"
by_hand "with the checkout named as its fixture" "$H" T="$H"
by_hand "in a directory inside the checkout, named as its fixture" "$H/test/cases" T="$H/test/cases"
by_hand "in another checkout of the repository, named as its fixture" "$T/other" T="$T/other"
[ "$(harness_state)" = "$before" ] || { harness_state; echo "    a refused case changed the checkout it was started from"; exit 1; }
# and the same case through the runner passes, writing into its own fixture only
out="$(bash "$H/test/run.sh" p_by_hand 2>&1)" || { printf '%s\n' "$out"; echo "    the case failed through the runner"; exit 1; }
printf '%s\n' "$out" | grep -q '^ok   p_by_hand$' || { printf '%s\n' "$out"; echo "    the runner did not report the case ok"; exit 1; }
[ "$(harness_state)" = "$before" ] || { harness_state; echo "    the case wrote into the checkout through the runner"; exit 1; }
rm -f "$H/test/cases/p_by_hand.sh"
# the filter and the empty directory are usage errors in parallel mode too
MJ_TEST_JOBS=2 bash "$H/test/run.sh" no_such_case >/dev/null 2>&1 && { echo "    a filter matching nothing passed"; exit 1; }
rm -f "$H"/test/cases/*.sh
MJ_TEST_JOBS=2 bash "$H/test/run.sh" >/dev/null 2>&1 && { echo "    an empty case directory passed"; exit 1; }
MJ_TEST_JOBS=0 bash "$H/test/run.sh" >/dev/null 2>&1 && { echo "    MJ_TEST_JOBS=0 was accepted"; exit 1; }
MJ_TEST_JOBS=two bash "$H/test/run.sh" >/dev/null 2>&1 && { echo "    MJ_TEST_JOBS=two was accepted"; exit 1; }

# --- the prebuilt executable: rust_bin hands a case MAJORDOMUS_BIN when one is given and
#     never builds; refuses one that is not executable; and asks to skip when there is
#     neither an executable of this tree (none in an empty target, no release of its
#     generation) nor cargo
printf '#!/bin/sh\necho fake\n' > "$T/fake-majordomus"; chmod +x "$T/fake-majordomus"
[ "$(MAJORDOMUS_BIN="$T/fake-majordomus" rust_bin)" = "$T/fake-majordomus" ] || { echo "    rust_bin did not hand back MAJORDOMUS_BIN"; exit 1; }
rc=0; MAJORDOMUS_BIN="$T/no-such-file" rust_bin >/dev/null 2>&1 || rc=$?
[ "$rc" = 1 ] || { echo "    rust_bin accepted a MAJORDOMUS_BIN that is not executable (rc $rc)"; exit 1; }
rc=0; MAJORDOMUS_BIN='' PATH="/usr/bin:/bin" CARGO_TARGET_DIR="$T/no-target" MAJORDOMUS_INSTALLED_BIN="$T/no-installed-release" rust_bin >/dev/null 2>&1 || rc=$?
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

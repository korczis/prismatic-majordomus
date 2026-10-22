# majordomus-covers: none
# Evidence answers by subject (I1953, ADR 0087 decision 3). `evidence subject <key>` judges any
# subject of the index through the index's own judge, and `evidence coverage` lists the
# coverage the run records carry, each judged against the checked-out commit. This case drives
# both through the Rust executable in a fixture repository of its own.
#
# What it proves, in order:
#   1  every subject kind of the index — claim, rule, feature, command, capability, MCP tool
#      and use case — answers with its key, its kind, a verdict and the totals beside it, in
#      JSON and in text; a member reads the verdict its own subject answers with
#   2  an unknown subject, of a kind that exists or of none, exits 13 with a message naming
#      the key and where the subjects are listed, and writes nothing on standard output
#   3  coverage recorded by the real recorder at the checked-out commit on a clean tree is
#      current; a dirty checkout, a tree the caller knows was dirty, another commit, and a
#      coverage run that measured its own tree dirty are each stale, naming why
#   4  a named source is read inside the checkout, and one outside it exits 13
#
# The reports and the summaries live outside the repository under test: a file written inside
# it would be an untracked file, which is a dirty tree, and coverage is current only on a
# clean one. The run records live under .ai/local/, which init ignores.
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"
command -v jq >/dev/null || { echo "    jq is required"; exit 1; }

W="$(mktemp -d "${TMPDIR:-/tmp}/mj508.XXXXXX")"; trap 'rm -rf "$W"' EXIT

ev() {           # ev <name> <args...> — the JSON answer of `evidence` into $W/<name>.json
  local name="$1"; shift
  run_quiet "$W/$name.err" "$MJB" evidence --repo "$T" --format json "$@" > "$W/$name.json"
}
jqe() {          # jqe <name> <filter> <what broke> [jq args...]
  local name="$1" filter="$2" what="$3"; shift 3
  jq -e "$@" "$filter" "$W/$name.json" >/dev/null 2>&1 || {
    printf '    %s\n' "$what"; jq -c . "$W/$name.json" | head -c 2000; echo; return 1; }
}
clean() {        # clean <what> — the fixture's tree is clean, or the case says what dirtied it
  [ -z "$(git status --porcelain)" ] || { echo "    $1 left the tree dirty"; git status --porcelain; exit 1; }
}
RUNS=.ai/local/evidence/runs

# ---------------------------------------------------------------- the fixture
# A layer with a claim, a rule, a feature and a use case of its own; the command, the
# capability and the MCP tool come from the executable itself.
"$MJ" init >/dev/null
git check-ignore -q "$RUNS/x.json" \
  || { echo "    init does not ignore .ai/local/, which the local run records rely on"; exit 1; }
mkdir -p docs lib test/cases .ai/repo/features .ai/repo/use-cases .ai/repo/rules/project
printf 'version: 1\nclaims:\n  - id: alpha-holds\n    claim: Alpha holds\n' > docs/CLAIMS.yaml
printf '    source: docs/ALPHA.md\n    implementation: lib/alpha.sh\n' >> docs/CLAIMS.yaml
printf '    test: test/cases/901_alpha.sh\n    status: advisory\n' >> docs/CLAIMS.yaml
printf '# Alpha\n' > docs/ALPHA.md
printf '#!/usr/bin/env bash\n# alpha\n' > lib/alpha.sh
printf '# majordomus-%s: %s\ntrue\n' covers none > test/cases/901_alpha.sh
{ printf -- '---\nid: project.alpha-rule\nversion: 1\nkind: rule\ntitle: Rule project.alpha-rule\n'
  printf 'description: What project.alpha-rule requires, in one sentence.\n'
  printf 'statement: The normative sentence project.alpha-rule asks a worker to follow.\n'
  printf 'status: active\nclass: advisory\ndepends_on: []\ntags: [fixture]\n\n'
  printf 'x-majordomus:\n  tests: [test/cases/901_alpha.sh]\n---\n\n# Rationale\n\nA fixture.\n\n'
  printf '# Required behaviour\n\nThe fixture holds.\n\n# Failure behaviour\n\nReported.\n\n'
  printf '# Verification\n\nThis case.\n'
} > .ai/repo/rules/project/alpha-rule.v1.md
{ printf -- '---\nschema: feature/v1\nid: alpha\nkind: feature\n'
  printf "title: 'The feature alpha'\nshort_title: 'alpha'\n"
  printf "headline: 'A feature this case declares.'\nsummary: 'A fixture feature.'\n"
  printf 'status: stable\nweight: 10\nfeatured: false\n'
  printf 'claims: [alpha-holds]\nrules: [project.alpha-rule]\nuse_cases: [see-alpha]\n'
  printf -- '---\n\n## What it does\n\nWhat the case says.\n\n## What it does not do\n\nAnything else.\n'
} > .ai/repo/features/alpha.md
{ printf -- "---\nid: see-alpha\nkind: use-case\ntitle: 'See alpha hold'\n"
  printf "summary: 'Ask what proves alpha.'\ncategory: adoption\nstatus: active\n"
  printf 'target: guaranteed\nactors: [operator]\ndifficulty: basic\ncommands: [version]\n'
  printf 'doctrines: [project.alpha-rule]\nclaims: [alpha-holds]\n'
  printf 'mcp_tools: [majordomus_evidence_subject]\nresponsibilities: []\napplications: []\n---\n\n'
  printf '# Situation\n\nIs alpha proven?\n\n# Scenario\n\n```yaml\nsetup: bare\ngiven:\n'
  printf "  - 'nothing installed'\nsteps:\n  - id: print\n    run: ['version']\n"
  printf "    note: 'the version string'\n    expect:\n      exit: 0\nthen:\n"
  printf "  - 'the version needs no repository'\n\`\`\`\n\n# Outcome\n\nAlpha, from its subject.\n"
} > .ai/repo/use-cases/see-alpha.md
git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm fixture
clean "the fixture"

# ---------------------------------------------------------------- 1. every kind answers
for pair in claim:alpha-holds rule:project.alpha-rule feature:alpha "command:evidence subject" \
  capability:evidence.subject mcp:majordomus_evidence_subject use_case:see-alpha; do
  kind="${pair%%:*}"
  ev one subject "$pair"
  jqe one '.subject == $k and .kind == $kind and (.verdict | type == "string") and (.totals | type == "object")
    and .presented.revision == "working_tree"' \
    "subject $pair did not answer with its key, its kind, a verdict and its totals" \
    --arg k "$pair" --arg kind "$kind"
  expect_exit 0 "$MJB" evidence --repo "$T" subject "$pair"
  expect_grep "^subject +$pair \\($kind\\)"
  expect_grep '^verdict +[a-z_]+'
done
ev claim subject claim:alpha-holds
jqe claim '.routes[0].test == "suite:901_alpha"' "the claim's route is not the test it names"
ev feature subject feature:alpha
jqe feature '[.members[] | select(.subject == "claim:alpha-holds") | .state] == [$c[0].verdict]' \
  "the feature's claim member does not read the claim's own verdict" --slurpfile c "$W/claim.json"
ev tool subject mcp:majordomus_evidence_subject
jqe tool '[.members[].subject] == ["capability:evidence.subject"]' \
  "the MCP tool is not an alias of the capability that declares it"

# ---------------------------------------------------------------- 2. an unknown subject
for key in feature:no-such gate:anything; do
  rc=0; "$MJB" evidence --repo "$T" subject "$key" > "$W/unknown.out" 2> "$W/unknown.err" || rc=$?
  [ "$rc" = 13 ] || { echo "    the unknown subject $key exited $rc, not 13"; cat "$W/unknown.err"; exit 1; }
  [ ! -s "$W/unknown.out" ] || { echo "    the unknown subject $key printed an answer"; cat "$W/unknown.out"; exit 1; }
  grep -qF "\`$key\` is not a subject" "$W/unknown.err" \
    || { echo "    the refusal of $key does not name the key"; cat "$W/unknown.err"; exit 1; }
  grep -qF 'site/data/registry/evidence-subjects.json' "$W/unknown.err" \
    || { echo "    the refusal of $key does not say where the subjects are listed"; cat "$W/unknown.err"; exit 1; }
done

# ---------------------------------------------------------------- 3. coverage, current only at its own commit
# A coverage summary as scripts/rust-coverage --summary-json writes it, recorded by the real
# recorder with the measurement its own stamp took.
one='{"covered":9,"total":10,"percent":90.0}'
printf '{"schema":1,"measurement":"scripts/rust-coverage","test_code":"excluded","crate":{"lines":%s,"functions":%s,"regions":%s},"domain":{"lines":%s,"functions":%s,"regions":%s,"files":[],"missing":[]},"files":{}}\n' \
  "$one" "$one" "$one" "$one" "$one" "$one" > "$W/coverage.json"
cover() {        # cover <run record name> — stamp and record the summary into $RUNS/<name>.json
  run_quiet "$W/stamp.err" "$MJB" evidence --repo "$T" stamp --producer coverage \
    --report "$W/coverage.json" --out "$W/pcov.json" > /dev/null
  run_quiet "$W/record.err" "$MJB" evidence --repo "$T" record --coverage "$W/coverage.json" \
    --provenance "coverage=$W/pcov.json" --run-record "$RUNS/$1.json" > /dev/null
}
HEAD1="$(git rev-parse HEAD)"
cover first
clean "a coverage recording"
ev cov coverage
jqe cov '.presented.commit == $h and .presented.tree == "clean" and .source == ".ai/local/evidence/runs" and .runs == 1' \
  "coverage was not judged against the clean checked-out commit, from the local runs" --arg h "$HEAD1"
jqe cov '[.records[] | {state, commit: .coverage.commit}] == [{state: "current", commit: $h}]' \
  "coverage recorded at the checked-out commit on a clean tree is not current" --arg h "$HEAD1"
expect_exit 0 "$MJB" evidence --repo "$T" coverage
expect_grep '^current +'

# a dirty checkout: an untracked file
printf 'x\n' > untracked.txt
ev cov-dirty coverage
jqe cov-dirty '.presented.tree == "dirty" and .records[0].state == "stale" and (.records[0].reason | test("presented tree is dirty"))' \
  "coverage is current over a dirty checkout"
rm -f untracked.txt
clean "the dirty probe"

# a tree the caller knows was dirty, on a clean checkout
ev cov-told coverage --presented-tree dirty
jqe cov-told '.records[0].state == "stale" and (.records[0].reason | test("presented tree is dirty"))' \
  "coverage is current although the caller said the tree was dirty"

# another commit: the record is shown with its own commit, and stale
printf 'more\n' >> docs/ALPHA.md
git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm another
HEAD2="$(git rev-parse HEAD)"
ev cov-moved coverage
jqe cov-moved '.records[0].state == "stale" and .records[0].coverage.commit == $h1
    and (.records[0].reason | test("not at the presented commit"))' \
  "coverage of another commit is not stale with its own commit" --arg h1 "$HEAD1"

# a coverage run that measured its own tree dirty is stale at its own commit, clean or not
printf 'x\n' > untracked.txt
cover second
rm -f untracked.txt
clean "the dirty coverage run"
ev cov-two coverage
jqe cov-two '[.records[] | {state, commit: .coverage.commit, tree: .coverage.working_tree}]
    == [{state: "stale", commit: $h1, tree: "clean"}, {state: "stale", commit: $h2, tree: "dirty"}]' \
  "a coverage run that measured its tree dirty is current" --arg h1 "$HEAD1" --arg h2 "$HEAD2"
jqe cov-two '.records[1].reason | test("measured its tree as dirty")' \
  "the stale record does not say its own run measured a dirty tree"

# ---------------------------------------------------------------- 4. where run records are read from
ev from coverage --from "$RUNS/first.json"
jqe from '.source == ".ai/local/evidence/runs/first.json" and .runs == 1' \
  "a named run record inside the checkout is not read alone"
expect_exit 13 "$MJB" evidence --repo "$T" coverage --from "$W"
expect_grep 'outside the repository'

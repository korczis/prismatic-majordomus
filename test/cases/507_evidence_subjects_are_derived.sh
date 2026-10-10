# majordomus-covers: none
# claims: evidence-subjects-are-derived, evidence-subject-commands-agree-with-the-command-pages
# Every declaration a verdict can be asked about is an evidence subject, and the subject
# index — what each subject is made of and which tests it reaches — is derived by the
# executable from declarations that already exist. This case proves it in two halves.
#
# The fixture half derives the index of a repository of its own, through `generate site`:
#   F1-F4   a claim has one route with the join's inputs, a test-less claim is still a
#           subject, and a rule's path no runner drives is a mechanism
#   F5      a command's routes come from the FIRST covers and negative header of each case,
#           and a documented example path reaches the examples binary
#   F6-F9   features, use cases, MCP aliases and capabilities are made of what they name, a
#           capability of the claims implemented in its module's file
#   F10-F13 the tests nothing reaches are listed, the pages are unique and ordered, the file
#           carries no ledger content, and two derivations are byte-identical
#   F14     derived, not authored: removing a declaration removes what it gave
#   F15     refused when hand-edited: `generate site --check` exits 10
#
# The real-repository half reads this repository's committed projections and holds each one
# equal to the index, so that no shell derivation of subject to test goes unnamed: the
# claims matrix, the product model, the catalogue, the registry, the command pages (R6), the
# capability pages (R10), the doctrine pages (R11) and the command graph (R14). R7 holds
# every case to one header of each kind, and its first covers line to naming the same public
# commands whether it is read word for word or between word boundaries, which is what keeps
# the any-line readers equal to the first-line rule. R13 holds this file to its own single
# header: the fixture writes its header lines with printf, never in a heredoc, so none of
# them can become a header of this case.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"
command -v jq >/dev/null || { echo "    jq is required"; exit 1; }

# The generated trees and the JSON answers live outside the repository under test.
S="$(mktemp -d "${TMPDIR:-/tmp}/mj507.XXXXXX")"; trap 'rm -rf "$S"' EXIT

jqe() {          # jqe <file> <filter> <what broke> [jq args...]
  local file="$1" filter="$2" what="$3"; shift 3
  jq -e "$@" "$filter" "$file" >/dev/null 2>&1 || {
    printf '    %s\n' "$what"; jq -c . "$file" | head -c 2000; echo; return 1; }
}
sub() {          # sub <file> <key> — one subject of an index, as JSON, into $S/sub.json
  jq --arg k "$2" '.subjects[$k]' "$1" > "$S/sub.json"
}

# ---------------------------------------------------------------- the fixture
"$MJ" init >/dev/null
REG="$ROOT/docs/generated/registry.json"
# the capability the fixture's use case calls through MCP, and the file its module is
# composed in: builtin capabilities come from the executable, so the fixture has the same one
CAP="$(jq -r '.capabilities[] | select(.exposure.mcp.tool == "majordomus_capabilities") | .id' "$REG")"
CAPSRC="$(jq -r --arg c "$CAP" '.capabilities[] | select(.id == $c) | .source_path' "$REG")"
CAPCLI="$(jq -r --arg c "$CAP" '.capabilities[] | select(.id == $c) | .exposure.cli.path | join(" ")' "$REG")"
[ -n "$CAP" ] && [ -n "$CAPSRC" ] && [ -n "$CAPCLI" ] \
  || { echo "    registry.json names no capability behind majordomus_capabilities"; exit 1; }

mkdir -p docs lib scripts test/cases apps/majordomus-cli/tests "$(dirname "$CAPSRC")" \
  .ai/repo/features .ai/repo/use-cases .ai/repo/rules/project
cat > docs/CLAIMS.yaml <<YAML
version: 1
claims:
  - id: alpha-holds
    claim: Alpha holds
    source: docs/ALPHA.md
    implementation: lib/alpha.sh
    test: test/cases/901_alpha.sh
    status: guaranteed
  - id: beta-planned
    claim: Beta is not built yet
    source: docs/ALPHA.md
    implementation: '-'
    test: '-'
    status: planned
  - id: gamma-elsewhere
    claim: Gamma is proved by something no runner drives
    source: docs/ALPHA.md
    implementation: lib/gamma.sh
    test: lib/gamma.sh
    status: advisory
  - id: delta-in-a-module
    claim: Delta is implemented where a capability is composed
    source: docs/ALPHA.md
    implementation: $CAPSRC
    test: test/cases/901_alpha.sh
    status: advisory
YAML
printf '# Alpha\n' > docs/ALPHA.md
printf '#!/usr/bin/env bash\n# alpha\n' > lib/alpha.sh
printf '#!/usr/bin/env bash\n# gamma\n' > lib/gamma.sh
printf '#!/usr/bin/env bash\n# the gate\n' > scripts/alpha-gate
: > "$CAPSRC"
# every header line through printf: the format is reused, so the first call prints two lines
printf '# majordomus-%s: %s\n' covers generate negative generate > test/cases/901_alpha.sh
printf '# claims: %s\n' alpha-holds >> test/cases/901_alpha.sh
printf '# majordomus-%s: %s\n' covers none > test/cases/902_orphan.sh
printf '# majordomus-%s: %s\n' covers none > test/cases/903_second.sh
printf 'true\n' >> test/cases/903_second.sh
printf '# majordomus-%s: %s\n' covers generate >> test/cases/903_second.sh
: > apps/majordomus-cli/tests/cli_examples.rs
: > apps/majordomus-cli/tests/lonely_binary.rs

cat > .ai/repo/rules/project/alpha-rule.v1.md <<'MD'
---
id: project.alpha-rule
version: 1
kind: rule
title: Rule project.alpha-rule
description: What project.alpha-rule requires, in one sentence.
statement: The normative sentence project.alpha-rule asks a worker to follow.
status: active
class: blocking
depends_on: []
tags: [fixture]

x-majordomus:
  tests: [test/cases/901_alpha.sh, scripts/alpha-gate]
---

# Rationale

A fixture.

# Required behaviour

The fixture holds.

# Failure behaviour

The fixture is reported.

# Verification

This case.
MD

feature() {      # feature <id> <status> <front-matter line...>
  local id="$1" status="$2"; shift 2
  { printf -- '---\nschema: feature/v1\nid: %s\nkind: feature\n' "$id"
    printf "title: 'The feature %s'\nshort_title: '%s'\n" "$id" "$id"
    printf "headline: 'A feature this case declares.'\nsummary: 'A fixture feature.'\n"
    printf 'status: %s\nweight: 10\n' "$status"
    printf '%s\n' "$@"
    printf -- '---\n\n## What it does\n\nWhat the case says.\n\n## What it does not do\n\nAnything else.\n'
  } > ".ai/repo/features/$id.md"
}
feature alpha stable 'claims: [alpha-holds, beta-planned]' 'rules: [project.alpha-rule]' \
  'use_cases: [see-alpha]'
feature lonely stable 'claims: [beta-planned]'
feature drafty draft 'claims: [beta-planned]'

cat > .ai/repo/use-cases/see-alpha.md <<'MD'
---
id: see-alpha
kind: use-case
title: 'See alpha hold'
summary: 'Generate the site data and see the alpha claim.'
category: adoption
status: active
target: guaranteed
actors: [operator]
difficulty: basic
commands: [generate]
doctrines: [project.alpha-rule]
claims: [alpha-holds]
mcp_tools: [majordomus_capabilities]
responsibilities: []
applications: []
---

# Situation

Is alpha proven?

# Scenario

```yaml
setup: bare
given:
  - 'nothing installed'
steps:
  - id: print
    run: ['version']
    note: 'the version string'
    expect:
      exit: 0
then:
  - 'the version needs no repository'
```

# Outcome

Alpha, from its subject.
MD
git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm fixture

expect_exit 0 "$RB" generate site --out "$S/gen"
E="$S/gen/site/data/registry/evidence-subjects.json"

# ---------------------------------------------------------------- F1: where, and what
expect_file "$E"
jqe "$E" '.schema == "majordomus-site-evidence-subjects/v1"' "the index does not declare its schema"
jqe "$E" '.generated | startswith("GENERATED FILE")' "the index carries no generated banner"
[ ! -e "$S/gen/site/data/generated/evidence-subjects.json" ] \
  || { echo "    the index was written where the shell site generator deletes it"; exit 1; }

# ---------------------------------------------------------------- F2-F4: leaves
sub "$E" claim:alpha-holds
jqe "$S/sub.json" '.routes | length == 1' "a claim has more or fewer than one route"
jqe "$S/sub.json" '.routes[0] | .via == "claim" and .test == "suite:901_alpha" and .path == "test/cases/901_alpha.sh" and .category == "e2e"' \
  "a claim's route is not the test it names, categorised off its identity"
jqe "$S/sub.json" '.routes[0].inputs as $i | ["docs/ALPHA.md", "lib/alpha.sh", "test/cases/901_alpha.sh"] | all(. as $p | $i | index($p))' \
  "a claim's route does not carry the join's inputs"
jqe "$S/sub.json" '.page == "/evidence/claims/alpha-holds/"' "a claim's page is not under /evidence/claims/"
sub "$E" claim:gamma-elsewhere
jqe "$S/sub.json" '.routes[0].path == "lib/gamma.sh" and (.routes[0] | has("test") | not)' \
  "a claim naming a path no runner drives lost the path or gained a test"
sub "$E" claim:beta-planned
jqe "$S/sub.json" '(.routes | length == 1) and (.routes[0] | has("test") or has("path") | not)' \
  "a claim naming no test is not a subject with an empty route"
sub "$E" rule:project.alpha-rule
jqe "$S/sub.json" '[.routes[] | {via, test}] == [{"via": "rule", "test": "suite:901_alpha"}]' \
  "a rule's routes are not the named paths a runner drives"
jqe "$S/sub.json" '.mechanisms == ["scripts/alpha-gate"] and (has("page") | not)' \
  "a rule's path no runner drives is not a mechanism, or a rule has a page"

# ---------------------------------------------------------------- F5: the first header
sub "$E" "command:generate"
jqe "$S/sub.json" '[.routes[] | select(.test == "suite:901_alpha") | .via] | sort == ["behaviour", "negative"]' \
  "a covers or a negative header did not route to its command"
jqe "$S/sub.json" '[.routes[] | select(.via == "example") | .test] == ["crate:cli_examples"]' \
  "a documented example path does not reach the examples binary"
jqe "$S/sub.json" '[.routes[] | select(.test == "suite:903_second" or .test == "suite:902_orphan")] == []' \
  "a header that is not the first of its kind, or names none, routed somewhere"

# ---------------------------------------------------------------- F6-F9: composites
sub "$E" feature:alpha
jqe "$S/sub.json" '.members as $m | ["claim:alpha-holds", "claim:beta-planned", "rule:project.alpha-rule", "use_case:see-alpha"] | all(. as $k | $m | index($k))' \
  "a feature is not made of the claims, rules and use cases it names"
jqe "$S/sub.json" '.routes == [] and .findings == [] and .page == "/evidence/features/alpha/"' \
  "a feature that reaches a test has routes, findings or the wrong page"
jqe "$E" '.subjects["feature:lonely"].findings[0] | .code == "feature_without_evidence" and .severity == "error"' \
  "a stable feature that reaches no test is not an error finding"
jqe "$E" '.subjects["feature:drafty"].findings[0] | .code == "feature_without_evidence" and .severity == "warning"' \
  "a draft feature that reaches no test is not a warning finding"
sub "$E" use_case:see-alpha
jqe "$S/sub.json" '.members == ["claim:alpha-holds", "command:generate", "mcp:majordomus_capabilities", "rule:project.alpha-rule"]' \
  "a use case is not made of exactly what the graph's edges name"
jqe "$S/sub.json" '[.routes[] | {via, category, path, t: has("test")}] == [{"via": "scenario", "category": "scenario", "path": ".ai/repo/use-cases/see-alpha.md", "t": false}]' \
  "a use case's own route is not its scenario"
jqe "$E" '.subjects["mcp:majordomus_capabilities"].alias_of == ("capability:" + $c)' \
  "an MCP tool is not an alias of the capability that declares it" --arg c "$CAP"
jqe "$E" '.subjects["capability:" + $c].members == ["claim:delta-in-a-module", "command:" + $cli]' \
  "a capability is not made of its command and the claims of its module's file" --arg c "$CAP" --arg cli "$CAPCLI"
jqe "$E" '[.subjects[] | select(.kind == "capability") | select(.members | index("claim:delta-in-a-module")) | .id] | sort == ([$reg[0].capabilities[] | select(.source_path == $src) | .id] | sort)' \
  "a claim belongs to a capability of another file, or not to every capability of its own" \
  --slurpfile reg "$REG" --arg src "$CAPSRC"
jqe "$E" '.subjects["capability:repository.scope_classify"].page == "/evidence/capabilities/repository-scope-classify/"' \
  "a capability's page is not slugged as the site slugs it"

# ---------------------------------------------------------------- F10-F13
jqe "$E" '.excluded as $x | (["suite:902_orphan", "suite:903_second", "crate:lonely_binary"] | all(. as $t | $x | index($t))) and ($x | index("suite:901_alpha") | not) and ($x | index("crate:cli_examples") | not)' \
  "the tests nothing reaches are not exactly the ones listed"
jqe "$E" '[.pages[].path] as $p | ($p == ($p | unique)) and ($p | all(startswith("/evidence/")))' \
  "the pages are not unique and ordered under /evidence/"
jqe "$E" '[.pages[].subject | select(startswith("rule:") or startswith("mcp:"))] == []' \
  "a rule or an MCP alias has a page of its own"
jqe "$E" '[paths | .[-1] | strings] | unique | map(select(. == "commit" or . == "head" or . == "outcome" or . == "state" or . == "execution" or . == "at" or . == "digest")) == []' \
  "the index carries ledger content"
expect_exit 0 "$RB" generate site --out "$S/gen2"
cmp -s "$E" "$S/gen2/site/data/registry/evidence-subjects.json" \
  || { echo "    two derivations of one tree differ"; exit 1; }

# ---------------------------------------------------------------- F14: derived, not authored
git rm -q .ai/repo/features/lonely.md apps/majordomus-cli/tests/cli_examples.rs
printf '# majordomus-%s: %s\n' covers generate > test/cases/902_orphan.sh
git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm "a feature, a binary and a header change"
expect_exit 0 "$RB" generate site --out "$S/gen3"
E3="$S/gen3/site/data/registry/evidence-subjects.json"
jqe "$E3" '.subjects | has("feature:lonely") | not' "a removed feature is still a subject"
sub "$E3" "command:generate"
jqe "$S/sub.json" '[.routes[] | select(.via == "example")] == []' \
  "a documented example reaches an examples binary the tree no longer carries"
jqe "$S/sub.json" '[.routes[] | select(.via == "behaviour") | .test] | index("suite:902_orphan")' \
  "a new covers header did not route to its command"
jqe "$E3" '.excluded | index("suite:902_orphan") | not' "a case a header now reaches is still listed as reached by nothing"

# ---------------------------------------------------------------- F15: refused when hand-edited
expect_exit 0 "$RB" generate site
expect_exit 0 "$RB" generate site --check
IN=site/data/registry/evidence-subjects.json
jq '.subjects["claim:authored"] = .subjects["claim:alpha-holds"]' "$IN" > "$S/edited.json"
cp "$S/edited.json" "$IN"
expect_exit 10 "$RB" generate site --check
expect_grep 'site/data/registry/evidence-subjects\.json \(differs\)'

# ---------------------------------------------------------------- the real repository
# Read only, over the committed projections. Each assertion is an equality between two
# independent projections, so it holds whatever the data is; no count is asserted.
R="$ROOT/site/data/registry/evidence-subjects.json"
expect_file "$R"
CAPS="$ROOT/site/data/generated/capabilities.json"
CMDS="$ROOT/site/data/generated/commands.json"
EXE="$ROOT/site/data/generated/executable.json"
DOC="$ROOT/site/data/generated/doctrines.json"
CLI="$ROOT/docs/generated/cli.json"
ids() {          # ids <kind> — the ids of every subject of a kind, sorted, as JSON
  jq -c --arg k "$1" '[.subjects[] | select(.kind == $k) | .id] | sort' "$R"
}
same() {         # same <what broke> <json> <json>
  [ "$2" = "$3" ] || { printf '    %s\n    index:      %s\n    projection: %s\n' "$1" "$(printf '%s' "$2" | head -c 600)" "$(printf '%s' "$3" | head -c 600)"; return 1; }
}

# R1-R5: every subject kind is exactly what its owner declares
same "R1 the claim subjects are not the claims matrix" "$(ids claim)" "$(jq -c '[.claims[].id] | sort' "$CAPS")"
jqe "$R" '[$caps[0].claims[] | select(.test != "-" and .test != null) | . as $c | $idx[0].subjects["claim:" + $c.id].routes[0].path == $c.test] | all' \
  "R1 a claim's route does not name the test the matrix names" --slurpfile caps "$CAPS" --slurpfile idx "$R"
same "R2 the feature subjects are not the product model's features" "$(ids feature)" \
  "$(jq -c '[.features[].id] | sort' "$ROOT/site/data/registry/product.json")"
same "R3 the use-case subjects are not the catalogue's use cases" "$(ids use_case)" \
  "$(jq -c '[.use_cases[].id] | sort' "$ROOT/site/data/generated/catalogue.json")"
same "R4 the capability subjects are not the registry's capabilities" "$(ids capability)" \
  "$(jq -c '[.capabilities[].id] | sort' "$REG")"
jqe "$R" '[$exe[0].capabilities[] | . as $c | $idx[0].subjects["capability:" + $c.id].page == ("/evidence/capabilities/" + $c.slug + "/")] | all' \
  "R4 a capability's evidence page is not slugged as its site page is" --slurpfile exe "$EXE" --slurpfile idx "$R"
same "R5 the MCP subjects are not the registry's tools" "$(ids mcp)" \
  "$(jq -c '[.capabilities[] | .exposure.mcp.tool // empty] | sort' "$REG")"
jqe "$R" '[$reg[0].capabilities[] | select(.exposure.mcp.tool != null) | . as $c | $idx[0].subjects["mcp:" + $c.exposure.mcp.tool].alias_of == ("capability:" + $c.id)] | all' \
  "R5 an MCP alias is not the capability that declares the tool" --slurpfile reg "$REG" --slurpfile idx "$R"

# R6: the command pages and the index name the same cases, in byte order
jqe "$R" '[$cmds[0].commands[] | . as $c | $idx[0].subjects["command:" + $c.name] as $s
    | ($s != null)
      and ($c.tests.behaviour == [$s.routes[]? | select(.via == "behaviour") | .test | ltrimstr("suite:")])
      and ($c.tests.negative == [$s.routes[]? | select(.via == "negative") | .test | ltrimstr("suite:")])] | all' \
  "R6 a command page and the index name different cases for one command" --slurpfile cmds "$CMDS" --slurpfile idx "$R"

# R7: every header reader reads the same public commands out of every case. The index and
# the command pages take the exact words of the first line of a kind; `command-furnished`
# and the use-case impact trace take a command as a whole whitespace-separated word of any
# covers line (`grep -E "^# majordomus-covers:(.*[[:space:]])?<command>([[:space:]]|$)"`).
# They read word boundaries once (`\b<command>\b`), and then `script:scripts/ci/link-check`
# held `check` for them and for no exact reader; a prefixed name is never a command. The
# two readings agree only while each case has one header of each kind AND its first covers
# line names exactly the public commands its covers lines hold as words. The awk below
# applies both readings to every case and names each case where they differ.
jq -r '.commands[].name' "$CMDS" > "$S/public.txt"
r7="$(awk '
  function flush(   i, c) {
    if (file == "") return
    if (covers > 1 || negs > 1) print file " has more than one covers or negative header"
    for (i = 1; i <= n; i++) {
      c = cmd[i]
      if ((c in first) != (c in any))
        print file ": the exact and the any-line readings disagree about `" c "`"
    }
  }
  NR == FNR { cmd[++n] = $0; next }
  FNR == 1 { flush(); file = FILENAME; covers = 0; negs = 0; split("", first); split("", any) }
  /^# majordomus-covers:/ {
    covers++
    words = substr($0, length("# majordomus-covers:") + 1)
    if (covers == 1) {
      k = split(words, w, /[ \t]+/)
      for (j = 1; j <= k; j++) if (w[j] != "") first[w[j]] = 1
    }
    for (j = 1; j <= n; j++)
      if (match(" " words " ", "[ \t]" cmd[j] "[ \t]")) any[cmd[j]] = 1
  }
  /^# majordomus-negative:/ { negs++ }
  END { flush() }
' "$S/public.txt" "$ROOT"/test/cases/*.sh)"
[ -z "$r7" ] || { printf '%s\n' "$r7" | sed "s|$ROOT/||; s|^|    R7 |"; exit 1; }

# R8: the rule subjects are the rules the rules report judges
run_quiet "$S/rules.err" "$RB" rules report --repo "$ROOT" --format json > "$S/rules.json"
same "R8 the rule subjects are not the rules the rules report judges" "$(ids rule)" \
  "$(jq -c '[.rules[].rule.id] | sort' "$S/rules.json")"

# R9: every test nothing reaches exists, and truly nothing reaches it
for t in $(jq -r '.excluded[]' "$R"); do
  case "$t" in
    suite:*) p="test/cases/${t#suite:}.sh" ;;
    crate:*) p="apps/majordomus-cli/tests/${t#crate:}.rs" ;;
    *) echo "    R9 an excluded test $t is no runner's"; exit 1 ;;
  esac
  [ -f "$ROOT/$p" ] || { echo "    R9 the excluded test $t names no file ($p)"; exit 1; }
done
jqe "$R" '[.subjects[].routes[].test // empty] as $reached | [.excluded[] | select(. as $t | $reached | index($t))] == []' \
  "R9 a test listed as reached by nothing is reached by a route"

# R10: the capability pages and the index give a capability the same claims and tests
jqe "$R" '[$exe[0].capabilities[] | . as $c | $idx[0].subjects["capability:" + $c.id] as $s
    | ([$s.members[] | select(startswith("claim:")) | ltrimstr("claim:")] | sort) as $mine
    | (($c.claims | map(.id) | sort) == $mine)
      and (([$c.tests[] | select(. != "-" and . != "")] | unique)
           == ([$mine[] | $idx[0].subjects["claim:" + .].routes[].path // empty] | unique))] | all' \
  "R10 a capability page and the index give a capability different claims or tests" --slurpfile exe "$EXE" --slurpfile idx "$R"

# R11: every doctrine's test is a route or a mechanism of its rule, over at least one
jqe "$R" '[$doc[0].doctrines[] | select((.test // "") != "") | . as $d | $idx[0].subjects["rule:" + $d.id] | select(. != null)
    | ([.routes[].path] + .mechanisms | index($d.test)) != null] | (length > 0) and all' \
  "R11 a doctrine names a test its rule's subject does not reach, or no doctrine was compared" \
  --slurpfile doc "$DOC" --slurpfile idx "$R"

# R12: a command names the programs that answer to it, and both only with the finding
jqe "$R" '([$cmds[0].commands[].name]) as $shell
    | ([$cli[0] | .. | objects | select(has("examples") and has("path")) | select((.examples | length) > 0) | .path[1:] | join(" ") | select(. != "")]
       + [$reg[0].capabilities[] | .exposure.cli.path // empty | join(" ")]) as $native
    | [.subjects[] | select(.kind == "command") | . as $s
       | ((.programs | index("shell") != null) == ($shell | index($s.id) != null))
         and ((.programs | index("native") != null) == ($native | index($s.id) != null))
         and (([.findings[].code] | index("command_in_two_programs") != null) == ((.programs | length) == 2))] | all' \
  "R12 a command does not name exactly the programs that answer to it, or the finding disagrees" \
  --slurpfile cmds "$CMDS" --slurpfile cli "$CLI" --slurpfile reg "$REG"

# R14: the command subjects, and the programs each names, are the command graph's. The
# executable's runnable commands are its native words (a group that only holds commands,
# such as `evidence`, is not one), and the shell tool's commands are its shell words; the
# index reads the same two declarations through the documented examples and the registry,
# and this holds the two readings equal, so neither is a second inventory of the other.
run_quiet "$S/graph.err" "$RB" commands graph --format json --repo "$ROOT" > "$S/graph.json"
jqe "$R" '([$g[0].commands[] | select(.origin == "executable" and .runnable) | .path | join(" ")] | unique) as $native
    | ([$g[0].commands[] | select(.origin == "tool") | .path | join(" ")] | unique) as $shell
    | (($native | length) > 0) and (($shell | length) > 0)
      and ([.subjects[] | select(.kind == "command") | .id] | sort) == (($native + $shell) | unique)
      and ([.subjects[] | select(.kind == "command") | . as $s
            | ((.programs | index("native") != null) == ($native | index($s.id) != null))
              and ((.programs | index("shell") != null) == ($shell | index($s.id) != null))] | all)' \
  "R14 the command subjects, or the programs they name, are not the command graph's" \
  --slurpfile g "$S/graph.json"

# R13: this case's own headers, which the fixture's printf lines must never add to
SELF="$ROOT/test/cases/507_evidence_subjects_are_derived.sh"
nc=$(grep -c '^# majordomus-covers:' "$SELF" || true)
nn=$(grep -c '^# majordomus-negative:' "$SELF" || true)
ncl=$(grep -c '^# claims:' "$SELF" || true)
words="$(sed -n 's/^# majordomus-covers: *//p' "$SELF")"
if [ "$nc" != 1 ] || [ "$words" != none ] || [ "$nn" != 0 ] || [ "$ncl" != 1 ]; then
  printf '    R13 507 carries covers=%s (%s) negative=%s claims=%s header lines\n' "$nc" "$words" "$nn" "$ncl"
  exit 1
fi

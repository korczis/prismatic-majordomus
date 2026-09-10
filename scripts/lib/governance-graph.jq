# governance-graph.jq — this repository's enforcement chain as one graph, projected from
# site/data/generated/doctrines.json (itself the standard rule package plus the wiring the
# validators declare).
#
# Five node classes and five edge kinds, and no sixth of either is written here: a principle
# exists because a rule names it, a validator because a rule is decided by it, a command
# because a validator is dispatched from it, a test because a rule cites it. A rule that
# reaches no principle, or a principle no rule reaches, is therefore visible as a node with
# nothing attached rather than as an absence nobody can see.
#
#   jq -S -f scripts/lib/governance-graph.jq site/data/generated/doctrines.json
def slug: ascii_downcase | gsub("[^a-z0-9]+"; "-") | sub("^-"; "") | sub("-$"; "");
def has_test: (.test // "") | (. != "" and . != "-");

.doctrines as $rules
| ([$rules[] | select(.principle != "") | .principle] | unique) as $principles
| ([$rules[] | .id]) as $ids
| {
    schema: 1,
    source: "share/standard/majordomus/",
    generator: "scripts/lib/governance-graph.jq",
    statuses: {
      blocking: "the command that finds it stops and exits non-zero",
      advisory: "the command reports it and carries on"
    },
    edge_kinds: {
      derives: "the rule an invariant is made decidable by",
      depends_on: "the rule this one is only meaningful after",
      decided_by: "the validator that answers it",
      run_by: "the command that dispatches the validator",
      proved_by: "the behavioural test that proves it"
    },
    nodes: (
      [ $principles[]
        | { id: ("principle:" + (. | slug)), kind: "principle", role: "accent",
            label: ., external: false,
            summary: "An invariant of the standard package. It is not checked; the rules under it are." } ]
      + [ $rules[]
        | { id: ("rule:" + .id), kind: "rule",
            role: (if .class == "blocking" then "text" else "warn" end),
            label: .slug, status: .class, route: ("/doctrines/" + .slug + "/"),
            external: false, summary: .summary,
            note: ("exit " + (.exit_code | tostring) + " · " + .category) } ]
      + ( [ $rules[]
        | { id: ("validator:" + .validator), kind: "validator", role: "muted",
            label: .validator_function, external: false,
            summary: ("Decides the rule. Declared in " + .defined_in + ".") } ] | unique_by(.id) )
      + ( [ $rules[] | .enforced_by[]
        | { id: ("command:" + .), kind: "enforcer", role: "accent",
            label: ("majordomus " + .), external: false,
            summary: "A command a worker or a git hook runs. It dispatches the validators wired to it." } ]
          | unique_by(.id) )
      + ( [ $rules[] | select(has_test)
        | { id: ("test:" + .test), kind: "test", role: "good",
            label: .test, external: false,
            summary: "A behavioural case. It proves the validator answers, not that the rule is written down." } ]
          | unique_by(.id) )
    ),
    edges: (
      [ $rules[] | select(.principle != "")
        | { source: ("principle:" + (.principle | slug)), target: ("rule:" + .id), kind: "derives" } ]
      + [ $rules[] | .id as $from | (.depends_on // [])[]
        | (split("@") | .[0]) as $to
        | select($ids | index($to))
        | { source: ("rule:" + $from), target: ("rule:" + $to), kind: "depends_on" } ]
      + [ $rules[] | { source: ("rule:" + .id), target: ("validator:" + .validator), kind: "decided_by" } ]
      + ( [ $rules[] | .validator as $v | .enforced_by[]
        | { source: ("validator:" + $v), target: ("command:" + .), kind: "run_by" } ] | unique )
      + [ $rules[] | select(has_test)
        | { source: ("rule:" + .id), target: ("test:" + .test), kind: "proved_by" } ]
    )
  }
